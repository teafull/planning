// SQLite 存储层
// 数据目录: ~/Documents/planning/
//   planning.db        主库(WAL 模式)
//   backups/           启动快照备份(保留 20 份)
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

use crate::raft::RaftHandle;
use crate::raft_cmd::{
    apply_command, diff_events, kv_get, kv_upsert, route_diff, RaftCommand,
};

pub struct Db(pub Mutex<Connection>);

// ---------- 数据目录配置(固定位置存指针, 目录本身可换) ----------

#[derive(Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(rename = "dataDir", default)]
    data_dir: Option<String>,
}

fn config_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

fn read_config(app: &AppHandle) -> ConfigFile {
    config_file_path(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_config(app: &AppHandle, cfg: &ConfigFile) -> Result<(), String> {
    let p = config_file_path(app)?;
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(p, s).map_err(|e| e.to_string())
}

fn default_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .document_dir()
        .map_err(|_| "无法定位文档目录".to_string())?;
    Ok(base.join("planning"))
}

/// 解析启动参数: --data-dir <path> 与 --port <n>
/// 用于单机多节点测试: planning --data-dir /tmp/node1 --port 7878
pub fn parse_launch_args() -> (Option<String>, Option<u16>) {
    let args: Vec<String> = std::env::args().collect();
    let mut data_dir = None;
    let mut port = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data-dir" | "-d" => {
                if i + 1 < args.len() {
                    data_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse::<u16>().ok();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    (data_dir, port)
}

/// 解析当前生效目录: 命令行参数 > 配置 > 默认
fn resolve_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // 命令行参数优先级最高
    if let Some((Some(dir), _)) = std::thread::spawn(parse_launch_args).join().ok() {
        let p = PathBuf::from(&dir);
        if fs::create_dir_all(&p).is_ok() {
            return Ok(p);
        }
        tracing::warn!("命令行指定的数据目录不可用: {dir}");
    }

    let cfg = read_config(app);
    let cfg = read_config(app);
    if let Some(dir) = cfg.data_dir {
        let p = PathBuf::from(&dir);
        if fs::create_dir_all(&p).is_ok() {
            return Ok(p);
        }
        tracing::warn!("配置的数据目录不可用, 回退默认: {dir}");
    }
    default_data_dir(app)
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    resolve_data_dir(app)
}

/// 初始化数据库: 建目录、开库、WAL、建表、启动备份
pub fn init(app: &AppHandle) -> Result<(), String> {
    let dir = data_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let conn = Connection::open(dir.join("planning.db")).map_err(|e| e.to_string())?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
            id          TEXT PRIMARY KEY,
            date        TEXT NOT NULL,
            end_date    TEXT,
            start_time  REAL,
            end_time    REAL,
            title       TEXT NOT NULL DEFAULT '',
            event_type  TEXT NOT NULL DEFAULT 'task',
            description TEXT NOT NULL DEFAULT '',
            is_all_day  INTEGER NOT NULL DEFAULT 0,
            updated_at  INTEGER NOT NULL,
            created_at  INTEGER NOT NULL,
            deleted     INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_events_date ON events(date);
        CREATE TABLE IF NOT EXISTS kv (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;

    // 节点身份(集群 groundwork): 首次启动生成 uuid 并持久化
    if kv_get(&conn, "node.id")?.is_none() {
        kv_upsert(&conn, "node.id", &uuid::Uuid::new_v4().to_string())?;
    }

    // 一次性迁移: 旧 cluster.config.syncSince → 独立复制键 cluster.sync_since
    if kv_get(&conn, "cluster.sync_since")?.is_none() {
        let legacy = kv_get(&conn, "cluster.config")?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|cfg| {
                cfg.get("syncSince")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            });
        if let Some(since) = legacy {
            kv_upsert(&conn, "cluster.sync_since", &since)?;
        }
    }

    // schema 版本管理: 新库写入当前版本, 旧库按阶梯迁移
    const SCHEMA_VERSION: u32 = 1;
    match kv_get(&conn, "meta.schema_version")?.and_then(|v| v.parse::<u32>().ok()) {
        None => {
            kv_upsert(&conn, "meta.schema_version", &SCHEMA_VERSION.to_string())?;
        }
        Some(v) if v < SCHEMA_VERSION => {
            // 阶梯迁移(版本2时启用): if v < 2 { migrate_v1_to_v2(&conn)?; }
            let mut cur = v;
            while cur < SCHEMA_VERSION {
                cur += 1;
            }
            kv_upsert(&conn, "meta.schema_version", &SCHEMA_VERSION.to_string())?;
        }
        Some(v) if v > SCHEMA_VERSION => {
            tracing::warn!("[数据库] 警告: 库 schema 版本 {v} 高于程序支持的 {SCHEMA_VERSION}, 请升级应用");
        }
        _ => {}
    }

    // 一次性迁移: 旧 cluster.config.syncSince → 独立复制键 cluster.sync_since
    if kv_get(&conn, "cluster.sync_since")?.is_none() {
        let legacy = kv_get(&conn, "cluster.config")?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|cfg| {
                cfg.get("syncSince")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            });
        if let Some(since) = legacy {
            kv_upsert(&conn, "cluster.sync_since", &since)?;
        }
    }

    // 启动时快照备份(失败不阻塞启动)
    let _ = backup_snapshot(&conn);

    app.manage(Db(Mutex::new(conn)));
    Ok(())
}

/// VACUUM INTO 快照备份, 保留最近 20 份
fn backup_snapshot(conn: &Connection) -> Result<String, String> {
    let db_path = conn
        .path()
        .map(|p| PathBuf::from(p))
        .ok_or("无法获取数据库路径")?;
    let dir = db_path.parent().ok_or("无父目录")?.join("backups");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let target = dir.join(format!("planning-{stamp}.db"));
    conn.execute("VACUUM INTO ?1", params![target.to_string_lossy()])
        .map_err(|e| e.to_string())?;

    let mut backups: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "db").unwrap_or(false))
        .collect();
    backups.sort();
    while backups.len() > 20 {
        let oldest = backups.remove(0);
        let _ = fs::remove_file(oldest);
    }
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub fn db_backup(db: tauri::State<Db>) -> Result<String, String> {
    let conn = db.0.lock().unwrap();
    backup_snapshot(&conn)
}

// ---------- 数据目录管理 ----------

#[tauri::command]
pub fn db_get_data_dir(app: AppHandle) -> Result<String, String> {
    Ok(resolve_data_dir(&app)?.to_string_lossy().to_string())
}

fn is_cloud_path(p: &str) -> bool {
    let s = p.to_lowercase();
    s.contains("onedrive")
        || s.contains("dropbox")
        || s.contains("google drive")
        || s.contains("google\\drive")
        || s.contains("nutstore")
        || s.contains("坚果云")
        || s.contains("mobile documents")
}

/// 切换数据目录: checkpoint→快照→复制→完整性校验→写配置→热切换连接
/// 原目录文件保留作备份, 失败时无副作用
#[tauri::command]
pub fn db_set_data_dir(
    app: AppHandle,
    db: tauri::State<Db>,
    new_dir: String,
) -> Result<String, String> {
    let new_path = PathBuf::from(&new_dir);
    if !new_path.is_absolute() {
        return Err("请提供绝对路径".into());
    }
    fs::create_dir_all(&new_path).map_err(|e| format!("无法创建目录: {e}"))?;
    let new_db = new_path.join("planning.db");
    if new_db.exists() {
        return Err("目标目录已存在 planning.db，为避免覆盖请选择空目录".into());
    }

    // 1. 合并 WAL 到主库 + 旧目录落一份快照(安全网)
    let old_db_path = {
        let conn = db.0.lock().unwrap();
        let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        let _ = backup_snapshot(&conn);
        conn.path()
            .map(|s| PathBuf::from(s))
            .ok_or("无法获取当前库路径")?
    };

    // 2. 复制到新目录
    fs::copy(&old_db_path, &new_db).map_err(|e| format!("复制数据库失败: {e}"))?;

    // 3. 打开新库并做完整性校验, 失败则清理退出
    let conn = Connection::open(&new_db).map_err(|e| e.to_string())?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let check: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        let _ = fs::remove_file(&new_db);
        return Err("新目录数据库完整性校验失败，已取消切换".into());
    }

    // 4. 写入配置指针(此后解析指向新目录)
    write_config(&app, &ConfigFile { data_dir: Some(new_dir.clone()) })?;

    // 5. 热切换连接, 无需重启
    *db.0.lock().unwrap() = conn;

    if is_cloud_path(&new_dir) {
        Ok("目录已切换。⚠️ 该目录位于云盘同步范围内，网盘同步 SQLite 文件可能导致数据损坏，建议在网盘设置中排除 planning 子目录".to_string())
    } else {
        Ok(format!("目录已切换至 {new_dir}（原目录数据保留作备份）"))
    }
}

// ---------- 事件表 ----------

#[tauri::command]
pub fn db_get_events(db: tauri::State<Db>) -> Result<Vec<Value>, String> {
    let conn = db.0.lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, date, end_date, start_time, end_time, title, event_type, description,
                    is_all_day, updated_at, created_at
             FROM events WHERE deleted = 0
             ORDER BY date, start_time, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let date: String = row.get(1)?;
            let end_date: Option<String> = row.get(2)?;
            let start_time: Option<f64> = row.get(3)?;
            let end_time: Option<f64> = row.get(4)?;
            let title: String = row.get(5)?;
            let event_type: String = row.get(6)?;
            let description: String = row.get(7)?;
            let is_all_day: bool = row.get::<_, i64>(8)? != 0;
            let updated_at: i64 = row.get(9)?;
            let created_at: i64 = row.get(10)?;
            // 旧数据 id 为数字, 读回时还原为数字保持前端引用一致
            let id_val = match id.parse::<i64>() {
                Ok(n) => json!(n),
                Err(_) => json!(id),
            };
            Ok(json!({
                "id": id_val,
                "date": date,
                "endDate": end_date,
                "startTime": start_time,
                "endTime": end_time,
                "title": title,
                "type": event_type,
                "description": description,
                "isAllDay": is_all_day,
                "updatedAt": updated_at,
                "createdAt": created_at,
            }))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 全量保存(写路径改造后):
/// diff(精确变更) → 按同步时间范围路由 → 命令 → 本地 apply 或 Raft 提议
/// 无实际变更时不写库; 前端 API 不变
#[tauri::command]
pub async fn db_save_events(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    events: Vec<Value>,
) -> Result<(), String> {
    // 集群模式: 离线模式 → 本地应用+入队; 在线 → local 立即应用, cluster 提议
    let cluster_cmds = {
        let mut conn = db.0.lock().unwrap();
        let diff = diff_events(&conn, &events)?;
        if diff.is_empty() {
            return Ok(());
        }
        let routed = route_diff(&conn, &diff)?;
        if routed.is_empty() {
            return Ok(());
        }

        let raft_on = cluster.handle.try_lock().map(|g| g.is_some()).unwrap_or(true);
        if !raft_on {
            // 单机模式: 全部本地应用, 单事务(第 1 期行为)
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for cmd in routed.iter_all() {
                apply_command(&tx, cmd)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }

        if is_offline_mode(&conn) {
            // 离线模式: 本地立即生效 + 命令入队(同一事务, 崩溃时不产生半状态)
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for cmd in routed.iter_all() {
                apply_command(&tx, cmd)?;
            }
            for cmd in &routed.cluster {
                pending_push(&tx, cmd)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }

        if !routed.local.is_empty() {
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for cmd in &routed.local {
                apply_command(&tx, cmd)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        routed.cluster
    };

    // 集群路径: 提议(本机 Leader 直写 / 转发), 成功后尽力重放积压
    if !cluster_cmds.is_empty() {
        let handle = cluster.handle.lock().await.clone();
        let handle = handle.ok_or("集群句柄缺失")?;
        for cmd in &cluster_cmds {
            ensure_proposed(&handle, cmd).await.map_err(|e| {
                format!("{e}; 集群可能不可用, 可在设置中开启离线模式")
            })?;
        }
        let _ = flush_pending(&db, &cluster).await;
    }
    Ok(())
}

// ---------- KV 表(设置/任务/笔记/绩效) ----------

#[tauri::command]
pub fn db_get_kv(db: tauri::State<Db>, key: String) -> Result<Option<String>, String> {
    let conn = db.0.lock().unwrap();
    kv_get(&conn, &key)
}

/// kv 写入路由: 复制键在集群模式经 Raft 提议(离线模式入队), 其余本地直写
#[tauri::command]
pub async fn db_set_kv(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    key: String,
    value: String,
) -> Result<(), String> {
    if !crate::raft_cmd::is_replicated_key(&key) {
        let conn = db.0.lock().unwrap();
        return kv_upsert(&conn, &key, &value);
    }

    let cmd = RaftCommand::SetKv { key, value };
    let raft_on = cluster.handle.try_lock().map(|g| g.is_some()).unwrap_or(true);
    if !raft_on {
        // 单机模式
        let conn = db.0.lock().unwrap();
        return apply_command(&conn, &cmd);
    }

    // 集群模式
    let offline = {
        let mut conn = db.0.lock().unwrap();
        let off = is_offline_mode(&conn);
        if off {
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            apply_command(&tx, &cmd)?;
            pending_push(&tx, &cmd)?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        off
    };
    if offline {
        return Ok(());
    }

    let guard = cluster.handle.lock().await;
    let handle = guard.as_ref().ok_or("集群句柄缺失")?;
    ensure_proposed(handle, &cmd).await
}

#[tauri::command]
pub fn db_set_kv_multi(
    db: tauri::State<Db>,
    map: HashMap<String, String>,
) -> Result<(), String> {
    let mut conn = db.0.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (k, v) in map {
        tx.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![k, v],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

// ---------- 集群 groundwork(第 1 期: 身份与配置读写) ----------

#[tauri::command]
pub fn db_get_node_id(db: tauri::State<Db>) -> Result<String, String> {
    let conn = db.0.lock().unwrap();
    kv_get(&conn, "node.id")?.ok_or_else(|| "节点身份缺失".to_string())
}

// ---------- 集群管理(Raft 第2期) ----------

/// 集群运行状态(Tauri State 管理)
#[derive(Default)]
pub struct ClusterState {
    pub handle: tokio::sync::Mutex<Option<std::sync::Arc<RaftHandle>>>,
    /// axum 服务任务(退出集群时中止, 释放端口)
    pub server_task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// 周期重放任务是否已启动(防重复)
    pub flusher_started: std::sync::atomic::AtomicBool,
}

/// 清空 Raft 持久化状态(退出/重建/换集群加入时调用)。
/// events/kv 业务数据不受影响; 表不存在(从未跑过 raft)时静默忽略。
fn wipe_raft_state(conn: &Connection) {
    let _ = conn.execute_batch("DELETE FROM raft_log; DELETE FROM raft_state;");
}

/// 启动 30s 周期积压重放(全局单例)
pub fn spawn_pending_flusher(app: tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    let db = app.state::<Db>();
    let cluster = app.state::<ClusterState>();
    if cluster.flusher_started.swap(true, Ordering::SeqCst) {
        return;
    }
    drop(db);
    drop(cluster);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let db = app.state::<Db>();
            let cluster = app.state::<ClusterState>();
            let has_pending = {
                let conn = db.0.lock().unwrap();
                !pending_cmds(&conn).is_empty()
            };
            let handle = cluster.handle.lock().await.clone();
            if has_pending {
                if let Some(_) = handle {
                    let _ = flush_pending(&db, &cluster).await;
                }
            }
        }
    });
}

// ---------- 离线队列(kv 本地键) ----------

fn pending_cmds(conn: &Connection) -> Vec<RaftCommand> {
    crate::raft_cmd::kv_get(conn, "cluster.pending")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn pending_push(conn: &Connection, cmd: &RaftCommand) -> Result<(), String> {
    let mut v = pending_cmds(conn);
    v.push(cmd.clone());
    crate::raft_cmd::kv_upsert(
        conn,
        "cluster.pending",
        &serde_json::to_string(&v).map_err(|e| e.to_string())?,
    )
}

fn pending_take_all(conn: &Connection) -> Vec<RaftCommand> {
    let v = pending_cmds(conn);
    let _ = crate::raft_cmd::kv_upsert(conn, "cluster.pending", "[]");
    v
}

fn pending_prepend(conn: &Connection, cmds: Vec<RaftCommand>) -> Result<(), String> {
    let mut all = cmds;
    all.extend(pending_cmds(conn));
    crate::raft_cmd::kv_upsert(
        conn,
        "cluster.pending",
        &serde_json::to_string(&all).map_err(|e| e.to_string())?,
    )
}

fn is_offline_mode(conn: &Connection) -> bool {
    crate::raft_cmd::kv_get(conn, "cluster.offline")
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// 重放积压命令: 逐条提议, 失败时剩余回队首
async fn flush_pending(
    db: &tauri::State<'_, Db>,
    cluster: &tauri::State<'_, ClusterState>,
) -> Result<usize, String> {
    let cmds = {
        let conn = db.0.lock().unwrap();
        pending_take_all(&conn)
    };
    if cmds.is_empty() {
        return Ok(0);
    }
    let handle = cluster.handle.lock().await.clone();
    let Some(handle) = handle else {
        let conn = db.0.lock().unwrap();
        pending_prepend(&conn, cmds)?;
        return Err("集群未启用".into());
    };
    let mut done = 0;
    for (i, cmd) in cmds.iter().enumerate() {
        if let Err(e) = ensure_proposed(&handle, cmd).await {
            let rest: Vec<RaftCommand> = cmds[i..].to_vec();
            let conn = db.0.lock().unwrap();
            pending_prepend(&conn, rest)?;
            return Err(e);
        }
        done += 1;
    }
    Ok(done)
}

/// 枚举本机全部 IPv4 地址(UI 多网卡选择用)
#[tauri::command]
pub fn get_local_ips() -> Result<Vec<serde_json::Value>, String> {
    let ifas = local_ip_address::list_afinet_netifas().map_err(|e| e.to_string())?;
    let mut list: Vec<serde_json::Value> = ifas
        .into_iter()
        .filter_map(|(name, ip)| match ip {
            std::net::IpAddr::V4(v4) => {
                let s = v4.to_string();
                if s.starts_with("127.") {
                    return None;
                }
                // 私网地址标记为"局域网", 其余(198.18 TUN/169.254 链路本地等)视为"虚拟"
                let o = s.split('.').next().and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
                let second = s.split('.').nth(1).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
                let is_lan = o == 192 && second == 168
                    || o == 10
                    || (o == 172 && (16..=31).contains(&second));
                Some(serde_json::json!({ "ip": s, "name": name, "isLan": is_lan }))
            }
            _ => None,
        })
        .collect();
    // 局域网地址排前, 便于默认选中
    list.sort_by(|a, b| {
        let la = a["isLan"].as_bool().unwrap_or(false);
        let lb = b["isLan"].as_bool().unwrap_or(false);
        lb.cmp(&la).then_with(|| a["ip"].as_str().cmp(&b["ip"].as_str()))
    });
    Ok(list)
}

/// 局域网 IP(UDP 探测, 不实际发包)
fn lan_ip() -> String {
    if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(a) = s.local_addr() {
                return a.ip().to_string();
            }
        }
    }
    "127.0.0.1".into()
}

fn gen_token() -> String {
    let n = uuid::Uuid::new_v4().as_simple().to_string();
    n.chars().take(6).collect::<String>().to_uppercase()
}

/// 命令提议: 先尝试本机写入, 非 Leader 时依据 ForwardToLeader 错误转发
async fn ensure_proposed(handle: &std::sync::Arc<RaftHandle>, cmd: &RaftCommand) -> Result<(), String> {
    let forward_addr = match handle.raft.client_write(cmd.clone()).await {
        Ok(_) => return Ok(()),
        Err(e) => {
            let fl = e.forward_to_leader().ok_or_else(|| format!("写入失败: {e}"))?;
            fl.leader_node
                .as_ref()
                .map(|n| n.addr.clone())
                .ok_or("集群暂无 Leader, 稍后重试")?
        }
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: Value = client
        .post(format!("http://{forward_addr}/cluster/propose"))
        .json(&serde_json::json!({ "command": cmd, "token": handle.cluster_token }))
        .send()
        .await
        .map_err(|e| format!("转发失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析失败: {e}"))?;
    if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(resp.get("msg").and_then(|v| v.as_str()).unwrap_or("转发失败").to_string())
    }
}

/// 创建集群: 单节点初始化, 生成令牌, 启动 HTTP 服务
#[tauri::command]
pub async fn cluster_create(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    port: u16,
    advertise: Option<String>,
) -> Result<serde_json::Value, String> {
    {
        let g = cluster.handle.lock().await;
        if g.is_some() {
            return Err("集群已在本机运行".into());
        }
    }

    let (db_path, data_dir) = {
        let conn = db.0.lock().unwrap();
        let p = conn.path().map(|s| s.to_string()).ok_or("无库路径")?;
        let dir = std::path::Path::new(&p)
            .parent()
            .map(|d| d.to_path_buf())
            .ok_or("无数据目录")?;
        (std::path::PathBuf::from(p), dir)
    };

    let token = gen_token();
    let addr = format!("{}:{port}", advertise.unwrap_or_else(lan_ip));

    // 清理旧集群残留的 raft 状态(如未正常退出), 避免 initialize 被拒
    {
        let conn = db.0.lock().unwrap();
        wipe_raft_state(&conn);
    }

    let handle = crate::raft::start_node(Some(app.clone()), 1, &db_path, data_dir.join("snapshots"), port, Some(token.clone())).await?;

    // initialize 失败 → 关停 Raft 核心, 不留孤儿
    let mut members = std::collections::BTreeMap::new();
    members.insert(1u64, openraft::BasicNode { addr: addr.clone() });
    if let Err(e) = handle.raft.initialize(members).await {
        let _ = handle.raft.shutdown().await;
        return Err(format!("初始化失败: {e}"));
    }

    // 持久化配置(保留已有 syncSince) —— 先于服务启动, 失败仅需关停 Raft
    let persist: Result<(), String> = (|| {
        let conn = db.0.lock().unwrap();
        let old: crate::raft::ClusterConfig = kv_get(&conn, "cluster.config")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let cfg = crate::raft::ClusterConfig {
            token: Some(token.clone()),
            sync_since: old.sync_since,
            listen_port: Some(port),
            advertise_addr: Some(addr.clone()),
            node_id: Some(1),
        };
        kv_upsert(&conn, "cluster.config", &serde_json::to_string(&cfg).map_err(|e| e.to_string())?)?;
        kv_upsert(&conn, "node.raft_id", "1")?;
        Ok(())
    })();
    if let Err(e) = persist {
        let _ = handle.raft.shutdown().await;
        return Err(e);
    }

    // 历史数据全量入库: 把本地所有事件写入 Raft 日志,
    // 加入方通过日志复制即可获得全部历史(否则历史永远不同步)
    let hist_cmds = {
        let conn = db.0.lock().unwrap();
        crate::raft_cmd::collect_local_events_for_proposal(&conn)?
    }; // guard 在此释放, 后续 await 安全
    if !hist_cmds.is_empty() {
        let hist_handle = handle.clone();
        for cmd in &hist_cmds {
            if let Err(e) = hist_handle.raft.client_write(cmd.clone()).await {
                let _ = handle.raft.shutdown().await;
                return Err(format!("历史数据入库失败: {e}"));
            }
        }
        tracing::info!("[集群] 历史数据入库完成: {} 条命令", hist_cmds.len());
    }

    // HTTP 服务最后启动; 失败关停 Raft(端口未占用, 无需 abort)
    let handle_with_token = handle;
    let jh = match crate::raft::server::start_server(handle_with_token.clone()).await {
        Ok(jh) => jh,
        Err(e) => {
            let _ = handle_with_token.raft.shutdown().await;
            return Err(e);
        }
    };
    *cluster.server_task.lock().await = Some(jh);
    spawn_pending_flusher(app.clone());
    *cluster.handle.lock().await = Some(handle_with_token.clone());
    Ok(serde_json::json!({ "token": token, "nodeId": 1, "addr": addr }))
}

/// 加入集群: 向目标节点登记, 本地作为 learner 启动并自动请求转正
#[tauri::command]
pub async fn cluster_join(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    target: String,
    token: String,
    port: u16,
    advertise: Option<String>,
) -> Result<serde_json::Value, String> {
    {
        let g = cluster.handle.lock().await;
        if g.is_some() {
            return Err("集群已在本机运行, 请先退出".into());
        }
    }

    // 目标未带端口时补默认端口
    let target = if target.contains(':') {
        target
    } else {
        format!("{target}:7878")
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    // 1. 查询目标状态分配新 raft_id
    let status_resp = client
        .get(format!("http://{target}/cluster/status"))
        .header("x-cluster-token", &token)
        .send()
        .await
        .map_err(|e| format!("无法连接 {target}: {e} (请确认对方已创建集群且防火墙放行)"))?;
    let status_code = status_resp.status();
    let status_text = status_resp.text().await.unwrap_or_default();
    if !status_code.is_success() {
        return Err(format!(
            "查询对方状态失败({status_code}): {status_text} (401=令牌错误; 404=对方版本过旧无此接口)"
        ));
    }
    let status: Value = serde_json::from_str(&status_text)
        .map_err(|_| format!("对方状态响应非 JSON(疑似版本不匹配): {status_text}"))?;
    let max_id = status["membership"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["raftId"].as_u64()).max().unwrap_or(0))
        .unwrap_or(0);
    let my_id = max_id + 1;
    let addr = format!("{}:{port}", advertise.unwrap_or_else(lan_ip));

    // 2. 本地先启动(不 initialize)——本地失败时目标侧零残留
    let (db_path, data_dir) = {
        let conn = db.0.lock().unwrap();
        // 清理旧集群残留 raft 状态(旧 vote 会阻断新集群选主)
        wipe_raft_state(&conn);
        let p = conn.path().map(|s| s.to_string()).ok_or("无库路径")?;
        let dir = std::path::Path::new(&p).parent().map(|d| d.to_path_buf()).ok_or("无数据目录")?;
        (std::path::PathBuf::from(p), dir)
    };
    let handle = crate::raft::start_node(Some(app.clone()), my_id, &db_path, data_dir.join("snapshots"), port, Some(token.clone())).await?;
    let jh = match crate::raft::server::start_server(handle.clone()).await {
        Ok(jh) => jh,
        Err(e) => {
            let _ = handle.raft.shutdown().await;
            return Err(e);
        }
    };

    // 3. 登记为目标 learner; 失败则关停本地, 不留孤儿成员
    let mut join_failed: Option<String> = None;
    let join_resp: Value = match client
        .post(format!("http://{target}/cluster/join"))
        .json(&serde_json::json!({ "token": token, "raftId": my_id, "addr": addr }))
        .send()
        .await
    {
        Ok(r) => {
            let code = r.status();
            let text = r.text().await.unwrap_or_default();
            if !code.is_success() {
                join_failed = Some(format!("加入被拒绝({code}): {text}"));
                Value::Null
            } else {
                match serde_json::from_str::<Value>(&text) {
                    Ok(v) => v,
                    Err(_) => {
                        join_failed = Some(format!(
                            "对方响应非 JSON(疑似版本不匹配): {text}"
                        ));
                        Value::Null
                    }
                }
            }
        }
        Err(e) => {
            join_failed = Some(format!("加入请求失败: {e}"));
            Value::Null
        }
    };
    if join_failed.is_some() || !join_resp["ok"].as_bool().unwrap_or(false) {
        jh.abort();
        let _ = handle.raft.shutdown().await;
        return Err(join_failed.unwrap_or_else(|| {
            join_resp["msg"].as_str().unwrap_or("加入失败").to_string()
        }));
    }
    *cluster.server_task.lock().await = Some(jh);
    spawn_pending_flusher(app.clone());

    // 持久化
    {
        let conn = db.0.lock().unwrap();
        let old: crate::raft::ClusterConfig = kv_get(&conn, "cluster.config")
            .ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let cfg = crate::raft::ClusterConfig {
            token: Some(token.clone()),
            sync_since: old.sync_since,
            listen_port: Some(port),
            advertise_addr: Some(addr.clone()),
            node_id: Some(my_id),
        };
        kv_upsert(&conn, "cluster.config", &serde_json::to_string(&cfg).map_err(|e| e.to_string())?)?;
        kv_upsert(&conn, "node.raft_id", &my_id.to_string())?;
    }
    *cluster.handle.lock().await = Some(handle.clone());

    // 4. 追平后: ①请求转正 ②本地历史数据重扫入库(本机独有的历史事件同步给集群)
    let t = target.clone();
    let promote_handle = handle.clone();
    let join_app = app.clone();
    let join_db_path = db_path.clone();
    let rescan_raft = handle.raft.clone();
    let rescan_node_id = handle.node_id;
    tokio::spawn(async move {
        let c = reqwest::Client::new();
        for _ in 0..12 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let is_voter = promote_handle
                .raft
                .metrics()
                .borrow()
                .membership_config
                .nodes()
                .any(|(id, _)| *id == promote_handle.node_id);
            if is_voter {
                return;
            }
            let ok = c
                .post(format!("http://{t}/cluster/promote"))
                .json(&serde_json::json!({ "raftId": promote_handle.node_id, "token": token }))
                .send()
                .await
                .and_then(|r| Ok(r.status().is_success()))
                .unwrap_or(false);
            if ok {
                return;
            }
        }
        tracing::warn!("[集群] 自动转正未完成, 可稍后手动重试加入");
    });

    // 5. 本地独有历史数据入库(等待转正/追平后执行, 确保写入有仲裁)
    let rescan_app = join_app;
    let rescan_db = join_db_path;
    tokio::spawn(async move {
        // 等待成为投票者或超时(60s)
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let is_voter = crate::raft::metrics_snapshot(&rescan_raft)
                .membership_config
                .nodes()
                .any(|(id, _)| *id == rescan_node_id);
            if is_voter { break; }
        }
        // 重扫: 把本地独有事件提议进集群
        let conn = match rusqlite::Connection::open(&rescan_db) {
            Ok(c) => c,
            Err(_) => return,
        };
        let cmds = match crate::raft_cmd::collect_local_events_for_proposal(&conn) {
            Ok(c) => c,
            Err(_) => return,
        };
        if cmds.is_empty() { return; }
        for cmd in &cmds {
            let _ = rescan_raft.client_write(cmd.clone()).await;
        }
        tracing::info!("[集群] 加入方历史数据重扫入库: {} 条命令", cmds.len());
        let _ = rescan_app;
    });

    Ok(serde_json::json!({ "nodeId": my_id, "addr": addr }))
}

/// 集群状态(未启用时 enabled=false)
#[tauri::command]
pub async fn cluster_status(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
) -> Result<serde_json::Value, String> {
    let g = cluster.handle.lock().await;
    let (offline, pending) = {
        let conn = db.0.lock().unwrap();
        (is_offline_mode(&conn), pending_cmds(&conn).len())
    };
    match g.as_ref() {
        None => Ok(serde_json::json!({ "enabled": false, "offline": offline, "pending": pending })),
        Some(h) => {
            let metrics = crate::raft::metrics_snapshot(&h.raft);
            let members: Vec<Value> = metrics
                .membership_config
                .nodes()
                .map(|(id, n)| serde_json::json!({ "raftId": id, "addr": n.addr }))
                .collect();
            Ok(serde_json::json!({
                "enabled": true,
                "offline": offline,
                "pending": pending,
                "nodeId": h.node_id,
                "state": format!("{:?}", metrics.state),
                "currentLeader": metrics.current_leader,
                "running": metrics.running_state.is_ok(),
                "membership": members,
                "lastApplied": metrics.last_applied.map(|l| l.index),
                "lastLogIndex": metrics.last_log_index,
            }))
        }
    }
}

/// 离线模式开关: 开启时写入本地+入队; 关闭时立即尝试重放积压
#[tauri::command]
pub async fn cluster_set_offline(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    {
        let conn = db.0.lock().unwrap();
        crate::raft_cmd::kv_upsert(&conn, "cluster.offline", if enabled { "1" } else { "0" })?;
    }
    if !enabled {
        let n = flush_pending(&db, &cluster).await?;
        return Ok(serde_json::json!({ "ok": true, "replayed": n }));
    }
    Ok(serde_json::json!({ "ok": true, "replayed": 0 }))
}

/// 手动全量同步: 把本地全部(时间范围内)事件推入 Raft 日志。
/// 用于历史数据补救 / 范围变更后回灌 / 手动对账。
#[tauri::command]
pub async fn cluster_sync_all(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
) -> Result<serde_json::Value, String> {
    let handle = cluster.handle.lock().await.clone().ok_or("集群未启用")?;

    // 离线模式下提示先恢复在线
    let offline = {
        let conn = db.0.lock().unwrap();
        is_offline_mode(&conn)
    };
    if offline {
        return Err("当前处于离线模式, 请先关闭离线模式再同步".into());
    }

    // 收集本地全部事件(时间范围内), guard 在块内释放
    let cmds = {
        let conn = db.0.lock().unwrap();
        crate::raft_cmd::collect_local_events_for_proposal(&conn)?
    };
    if cmds.is_empty() {
        return Ok(serde_json::json!({ "ok": true, "proposed": 0, "msg": "本地无待同步事件" }));
    }

    let total: usize = cmds.iter().map(|c| match c {
        RaftCommand::UpsertEvents { events } => events.len(),
        _ => 0,
    }).sum();

    // 逐批提议(非 Leader 自动转发)
    for cmd in &cmds {
        ensure_proposed(&handle, cmd).await?;
    }

    Ok(serde_json::json!({
        "ok": true,
        "proposed": total,
        "msg": format!("已将 {total} 条事件推入集群同步")
    }))
}

/// 手动重放积压命令
#[tauri::command]
pub async fn cluster_flush_pending(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
) -> Result<serde_json::Value, String> {
    let n = flush_pending(&db, &cluster).await?;
    Ok(serde_json::json!({ "ok": true, "replayed": n }))
}

async fn forward_remove_to_leader(handle: &std::sync::Arc<RaftHandle>, raft_id: u64) -> Result<(), String> {
    let metrics = crate::raft::metrics_snapshot(&handle.raft);
    let leader_id = metrics.current_leader.ok_or("集群无 Leader")?;
    let addr = metrics
        .membership_config
        .nodes()
        .find(|(id, _)| **id == leader_id)
        .map(|(_, n)| n.addr.clone())
        .ok_or("找不到 Leader 地址")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: Value = client
        .post(format!("http://{addr}/cluster/remove"))
        .json(&serde_json::json!({ "raftId": raft_id, "token": handle.cluster_token }))
        .send()
        .await
        .map_err(|e| format!("转发失败: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(resp.get("msg").and_then(|v| v.as_str()).unwrap_or("移除失败").to_string())
    }
}

async fn shutdown_cluster_local(
    db: &tauri::State<'_, Db>,
    cluster: &tauri::State<'_, ClusterState>,
    handle: &std::sync::Arc<RaftHandle>,
) -> Result<(), String> {
    let _ = handle.raft.shutdown().await;
    if let Some(jh) = cluster.server_task.lock().await.take() {
        jh.abort();
    }
    *cluster.handle.lock().await = None;
    // 配置回单机(保留 syncSince 供下次使用); 清空离线队列防跨集群污染;
    // 清空 raft 持久化状态, 否则下次 initialize 会被残留日志/投票拒绝
    let conn = db.0.lock().unwrap();
    wipe_raft_state(&conn);
    let _ = crate::raft_cmd::kv_upsert(&conn, "cluster.pending", "[]");
    let _ = crate::raft_cmd::kv_upsert(&conn, "cluster.offline", "0");
    let old: crate::raft::ClusterConfig = crate::raft_cmd::kv_get(&conn, "cluster.config")
        .ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let cfg = crate::raft::ClusterConfig {
        token: None,
        sync_since: old.sync_since,
        listen_port: None,
        advertise_addr: None,
        node_id: None,
    };
    crate::raft_cmd::kv_upsert(
        &conn,
        "cluster.config",
        &serde_json::to_string(&cfg).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}

/// 退出集群: 从成员表移除自己(经 Leader), 关停本地 raft/服务, 配置回单机
#[tauri::command]
pub async fn cluster_leave(
    db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
) -> Result<(), String> {
    let handle = cluster.handle.lock().await.clone().ok_or("集群未启用")?;

    let voters: std::collections::BTreeSet<u64> = handle
        .raft
        .metrics()
        .borrow()
        .membership_config
        .nodes()
        .map(|(id, _)| *id)
        .collect();
    let remaining: std::collections::BTreeSet<u64> =
        voters.into_iter().filter(|id| *id != handle.node_id).collect();

    if !remaining.is_empty() {
        if crate::raft::metrics_snapshot(&handle.raft).state == openraft::ServerState::Leader {
            handle
                .raft
                .change_membership(openraft::ChangeMembers::ReplaceAllVoters(remaining), true)
                .await
                .map_err(|e| format!("成员变更失败: {e}"))?;
        } else {
            forward_remove_to_leader(&handle, handle.node_id).await?;
        }
    }

    shutdown_cluster_local(&db, &cluster, &handle).await
}

/// 移除其他节点(需 Leader 执行, 非 Leader 自动转发)
#[tauri::command]
pub async fn cluster_remove(
    _db: tauri::State<'_, Db>,
    cluster: tauri::State<'_, ClusterState>,
    raft_id: u64,
) -> Result<(), String> {
    let handle = cluster.handle.lock().await.clone().ok_or("集群未启用")?;
    if crate::raft::metrics_snapshot(&handle.raft).state == openraft::ServerState::Leader {
        let voters: std::collections::BTreeSet<u64> = handle
            .raft
            .metrics()
            .borrow()
            .membership_config
            .nodes()
            .map(|(id, _)| *id)
            .collect();
        let remaining: std::collections::BTreeSet<u64> =
            voters.into_iter().filter(|id| *id != raft_id).collect();
        handle
            .raft
            .change_membership(openraft::ChangeMembers::ReplaceAllVoters(remaining), true)
            .await
            .map_err(|e| format!("成员变更失败: {e}"))?;
        Ok(())
    } else {
        forward_remove_to_leader(&handle, raft_id).await
    }
}

/// 应用启动时按持久化配置自动重启集群(重连)
pub async fn auto_start_cluster(app: tauri::AppHandle) {
    let db = app.state::<Db>();
    let cluster = app.state::<ClusterState>();

    let cfg: Option<crate::raft::ClusterConfig> = {
        let conn = db.0.lock().unwrap();
        crate::raft_cmd::kv_get(&conn, "cluster.config")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    };
    let Some(cfg) = cfg else { return };
    let (Some(node_id), Some(cfg_port), Some(token)) = (cfg.node_id, cfg.listen_port, cfg.token) else {
        return;
    };
    // 命令行 --port 覆盖
    let port = if let Some((_, Some(p))) = std::thread::spawn(parse_launch_args).join().ok() {
        p
    } else {
        cfg_port
    };

    let (db_path, snap_dir) = {
        let conn = db.0.lock().unwrap();
        let p = conn
            .path()
            .map(|s| s.to_string())
            .ok_or_else(|| "无库路径".to_string())
            .unwrap_or_default();
        if p.is_empty() {
            return;
        }
        (
            std::path::PathBuf::from(&p),
            std::path::Path::new(&p)
                .parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_default(),
        )
    };

    match crate::raft::start_node(Some(app.clone()), node_id, &db_path, snap_dir.join("snapshots"), port, Some(token)).await {
        Ok(handle) => match crate::raft::server::start_server(handle.clone()).await {
            Ok(jh) => {
                *cluster.server_task.lock().await = Some(jh);
                *cluster.handle.lock().await = Some(handle);
                tracing::info!("[集群] 已按配置自动重启: node={node_id} port={port}");
            }
            Err(e) => tracing::error!("[集群] 服务启动失败: {e}"),
        },
        Err(e) => tracing::error!("[集群] 自动重启失败: {e}"),
    }

    // 周期性重放积压(全局单例)
    spawn_pending_flusher(app.clone());
}
