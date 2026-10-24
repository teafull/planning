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

/// 解析当前生效目录: 配置优先, 配置不可用时回退默认目录
fn resolve_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = read_config(app);
    if let Some(dir) = cfg.data_dir {
        let p = PathBuf::from(&dir);
        if fs::create_dir_all(&p).is_ok() {
            return Ok(p);
        }
        eprintln!("配置的数据目录不可用, 回退默认: {dir}");
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
/// diff(精确变更) → 按同步时间范围路由 → 命令 → 单事务 apply
/// 无实际变更时不写库(不碰 updated_at, 为未来 LWW 语义做准备)
#[tauri::command]
pub fn db_save_events(db: tauri::State<Db>, events: Vec<Value>) -> Result<(), String> {
    let mut conn = db.0.lock().unwrap();

    let diff = diff_events(&conn, &events)?;
    if diff.is_empty() {
        return Ok(()); // 无变更, 零写入
    }

    // 第 1 期(单机): cluster/local 两路都本地应用, 同一事务保证原子性
    // 第 2 期(集群): cluster 路径改为 openraft propose
    let routed = route_diff(&conn, &diff)?;
    if routed.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for cmd in routed.iter_all() {
        apply_command(&tx, cmd)?;
    }
    tx.commit().map_err(|e| e.to_string())
}

// ---------- KV 表(设置/任务/笔记/绩效) ----------

#[tauri::command]
pub fn db_get_kv(db: tauri::State<Db>, key: String) -> Result<Option<String>, String> {
    let conn = db.0.lock().unwrap();
    kv_get(&conn, &key)
}

/// kv 写入路由: 需复制的键(任务/绩效/集群配置)走命令路径, 其余(settings/meta)本地直写
#[tauri::command]
pub fn db_set_kv(db: tauri::State<Db>, key: String, value: String) -> Result<(), String> {
    let conn = db.0.lock().unwrap();
    if crate::raft_cmd::is_replicated_key(&key) {
        apply_command(&conn, &RaftCommand::SetKv { key, value })
    } else {
        kv_upsert(&conn, &key, &value)
    }
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

#[tauri::command]
pub fn db_get_cluster_config(db: tauri::State<Db>) -> Result<Option<String>, String> {
    let conn = db.0.lock().unwrap();
    kv_get(&conn, "cluster.config")
}

/// 设置集群配置(经命令路径, 未来随 Raft 复制)。要求合法 JSON。
#[tauri::command]
pub fn db_set_cluster_config(
    db: tauri::State<Db>,
    value: String,
) -> Result<(), String> {
    let parsed: Value = serde_json::from_str(&value).map_err(|e| format!("非法 JSON: {e}"))?;
    let conn = db.0.lock().unwrap();
    apply_command(&conn, &RaftCommand::SetClusterConfig { value: parsed })
}
