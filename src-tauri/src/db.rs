// SQLite 存储层
// 数据目录: ~/Documents/planning/
//   planning.db        主库(WAL 模式)
//   backups/           启动快照备份(保留 20 份)
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

pub struct Db(pub Mutex<Connection>);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .document_dir()
        .map_err(|_| "无法定位文档目录".to_string())?;
    Ok(base.join("planning"))
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

// ---------- 事件表 ----------

fn get_str(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(String::from)
}
fn get_f64(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}
fn get_bool(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(|x| x.as_bool())
}

/// 统一 id 为字符串存储(兼容旧数字 id 与新 uuid)
fn norm_id(v: &Value) -> String {
    match v.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => format!("gen-{}", now_ms()),
    }
}

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

/// 全量保存: 事务内 upsert + 删除缺失(单事务, 失败整体回滚)
#[tauri::command]
pub fn db_save_events(db: tauri::State<Db>, events: Vec<Value>) -> Result<(), String> {
    let mut conn = db.0.lock().unwrap();
    let now = now_ms();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 现存 id → created_at(保留首次创建时间)
    let mut existing: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = tx
            .prepare("SELECT id, created_at FROM events")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (id, c) = r.map_err(|e| e.to_string())?;
            existing.insert(id, c);
        }
    }

    let incoming: HashSet<String> = events.iter().map(norm_id).collect();

    for ev in &events {
        let id = norm_id(ev);
        let date = get_str(ev, "date").unwrap_or_default();
        if date.is_empty() {
            continue; // 无日期的脏数据跳过
        }
        let created = existing.get(&id).copied().unwrap_or(now);
        tx.execute(
            "INSERT OR REPLACE INTO events
             (id, date, end_date, start_time, end_time, title, event_type, description,
              is_all_day, updated_at, created_at, deleted)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,0)",
            params![
                id,
                date,
                get_str(ev, "endDate"),
                get_f64(ev, "startTime"),
                get_f64(ev, "endTime"),
                get_str(ev, "title").unwrap_or_default(),
                get_str(ev, "type").unwrap_or_else(|| "task".into()),
                get_str(ev, "description").unwrap_or_default(),
                get_bool(ev, "isAllDay").unwrap_or(false) as i64,
                now,
                created
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    // 物理删除已不存在的事件(墓碑机制留给同步功能)
    for id in existing.keys() {
        if !incoming.contains(id) {
            tx.execute("DELETE FROM events WHERE id = ?1", params![id])
                .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())
}

// ---------- KV 表(设置/任务/笔记/绩效) ----------

#[tauri::command]
pub fn db_get_kv(db: tauri::State<Db>, key: String) -> Result<Option<String>, String> {
    let conn = db.0.lock().unwrap();
    conn.query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
        row.get::<_, String>(0)
    })
    .map(Some)
    .or_else(|e| {
        if e == rusqlite::Error::QueryReturnedNoRows {
            Ok(None)
        } else {
            Err(e.to_string())
        }
    })
}

#[tauri::command]
pub fn db_set_kv(db: tauri::State<Db>, key: String, value: String) -> Result<(), String> {
    let conn = db.0.lock().unwrap();
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
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
