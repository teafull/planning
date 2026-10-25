// 复制命令与状态机（写路径改造 · 第 1 期）
//
// 单机模式: db_save_events / db_set_kv → diff → RaftCommand → apply_command 直接落库
// 集群模式(第 2 期): 命令 → openraft propose → 各节点按日志序 apply_command
//
// 关键约束: apply_command 必须确定且幂等 —— 相同命令序列在任何节点产生相同库状态。
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- 字段提取(容错) ----------

pub fn get_str(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(String::from)
}
pub fn get_f64(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}
pub fn get_bool(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(|x| x.as_bool())
}

/// 统一 id 为字符串存储(兼容旧数字 id 与新 uuid)
pub fn norm_id(v: &Value) -> String {
    match v.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => format!("gen-{}", now_ms()),
    }
}

// ---------- 复制命令定义 ----------

/// 集群同步的基本单元。第 2 期该命令经 Raft 日志复制到各节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RaftCommand {
    UpsertEvents { events: Vec<Value> },
    DeleteEvents { ids: Vec<String> },
    SetKv { key: String, value: String },
    SetClusterConfig { value: Value },
}

/// 需要集群复制的 kv 键前缀(settings/meta/node 等为本地键)
pub fn is_replicated_key(key: &str) -> bool {
    key == "cluster.config"
        || key == "cluster.sync_since"
        || key.starts_with("tasks.")
        || key.starts_with("performance.")
}

// ---------- 变更检测 ----------

/// 事件关键字段签名(固定顺序)。相同签名 = 无实质变更, 不产生命令。
pub fn event_signature(v: &Value) -> String {
    format!(
        "{}|{:?}|{:?}|{:?}|{}|{}|{}|{}",
        get_str(v, "date").unwrap_or_default(),
        get_str(v, "endDate"),
        get_f64(v, "startTime"),
        get_f64(v, "endTime"),
        get_str(v, "title").unwrap_or_default(),
        get_str(v, "type").unwrap_or_default(),
        get_str(v, "description").unwrap_or_default(),
        get_bool(v, "isAllDay").unwrap_or(false)
    )
}

pub struct EventsDiff {
    pub upserts: Vec<Value>,
    pub deletes: Vec<String>,
}

impl EventsDiff {
    pub fn is_empty(&self) -> bool {
        self.upserts.is_empty() && self.deletes.is_empty()
    }
}

/// 对比现库与传入全量数组, 产出精确变更(前端 API 保持"全量传入"不变)
pub fn diff_events(conn: &Connection, incoming: &[Value]) -> Result<EventsDiff, String> {
    // 现存: id → 字段签名
    let mut existing: HashMap<String, String> = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, date, end_date, start_time, end_time, title, event_type, description, is_all_day FROM events",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            // 手工跳过第一列后构造部分 JSON: 复用 row_to_partial 需要列偏移,
            // rusqlite Row 无偏移 API, 因此这里直接内联构造
            let partial = json!({
                "date": row.get::<_, String>(1)?,
                "endDate": row.get::<_, Option<String>>(2)?,
                "startTime": row.get::<_, Option<f64>>(3)?,
                "endTime": row.get::<_, Option<f64>>(4)?,
                "title": row.get::<_, String>(5)?,
                "type": row.get::<_, String>(6)?,
                "description": row.get::<_, String>(7)?,
                "isAllDay": row.get::<_, i64>(8)? != 0,
            });
            Ok((id, event_signature(&partial)))
        })
        .map_err(|e| e.to_string())?;
    for r in rows {
        let (id, sig) = r.map_err(|e| e.to_string())?;
        existing.insert(id, sig);
    }

    let incoming_ids: HashSet<String> = incoming.iter().map(norm_id).collect();

    let deletes: Vec<String> = existing
        .keys()
        .filter(|id| !incoming_ids.contains(*id))
        .cloned()
        .collect();

    let upserts: Vec<Value> = incoming
        .iter()
        .filter(|ev| {
            let id = norm_id(ev);
            match existing.get(&id) {
                None => true,
                Some(old_sig) => *old_sig != event_signature(ev),
            }
        })
        .cloned()
        .collect();

    Ok(EventsDiff { upserts, deletes })
}

// ---------- 时间范围路由(第 2 期接入 propose) ----------

pub fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| {
        r.get::<_, String>(0)
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

pub fn kv_upsert(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// 读取集群级同步起始日期(YYYY-MM-DD, 词法比较即可)。无配置 = 全量同步。
/// 新键 cluster.sync_since(纯日期字符串, 经 Raft 复制);
/// 兼容旧 cluster.config JSON 内的 syncSince 字段(仅回退读取)。
pub fn sync_since(conn: &Connection) -> Option<String> {
    if let Some(v) = kv_get(conn, "cluster.sync_since").ok().flatten() {
        return if v.is_empty() { None } else { Some(v) };
    }
    let raw = kv_get(conn, "cluster.config").ok().flatten()?;
    let cfg: Value = serde_json::from_str(&raw).ok()?;
    cfg.get("syncSince")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

pub struct RoutedCommands {
    /// 进集群路径(第 2 期 → Raft propose; 第 1 期 → 本地 apply)
    pub cluster: Vec<RaftCommand>,
    /// 仅本地路径(范围外事件 / 本地键)
    pub local: Vec<RaftCommand>,
}

impl RoutedCommands {
    pub fn is_empty(&self) -> bool {
        self.cluster.is_empty() && self.local.is_empty()
    }
    pub fn iter_all(&self) -> impl Iterator<Item = &RaftCommand> {
        self.cluster.iter().chain(self.local.iter())
    }
}

/// 按同步时间范围把 diff 拆分为 集群命令 / 本地命令
pub fn route_diff(conn: &Connection, diff: &EventsDiff) -> Result<RoutedCommands, String> {
    let mut cluster = Vec::new();
    let mut local = Vec::new();

    match sync_since(conn) {
        None => {
            // 无范围约束: 全部走集群路径
            if !diff.upserts.is_empty() {
                cluster.push(RaftCommand::UpsertEvents { events: diff.upserts.clone() });
            }
            if !diff.deletes.is_empty() {
                cluster.push(RaftCommand::DeleteEvents { ids: diff.deletes.clone() });
            }
        }
        Some(since) => {
            let mut in_up = Vec::new();
            let mut out_up = Vec::new();
            for ev in &diff.upserts {
                let in_range = get_str(ev, "date")
                    .map(|d| d.as_str() >= since.as_str())
                    .unwrap_or(false);
                if in_range { in_up.push(ev.clone()) } else { out_up.push(ev.clone()) }
            }
            // 删除: 用现库中该事件的日期判断范围
            let mut in_del = Vec::new();
            let mut out_del = Vec::new();
            for id in &diff.deletes {
                let date: Option<String> = conn
                    .query_row("SELECT date FROM events WHERE id = ?1", params![id], |r| r.get(0))
                    .ok();
                let in_range = date.map(|d| d >= since).unwrap_or(true);
                if in_range { in_del.push(id.clone()) } else { out_del.push(id.clone()) }
            }
            if !in_up.is_empty() {
                cluster.push(RaftCommand::UpsertEvents { events: in_up });
            }
            if !in_del.is_empty() {
                cluster.push(RaftCommand::DeleteEvents { ids: in_del });
            }
            if !out_up.is_empty() {
                local.push(RaftCommand::UpsertEvents { events: out_up });
            }
            if !out_del.is_empty() {
                local.push(RaftCommand::DeleteEvents { ids: out_del });
            }
        }
    }

    Ok(RoutedCommands { cluster, local })
}

// ---------- 状态机核心: 命令 → SQL ----------

/// 将命令应用到本地 SQLite。单机直调; 集群模式各节点按日志序调用同一函数。
pub fn apply_command(conn: &Connection, cmd: &RaftCommand) -> Result<(), String> {
    match cmd {
        RaftCommand::UpsertEvents { events } => {
            let now = now_ms();
            for ev in events {
                let id = norm_id(ev);
                let date = get_str(ev, "date").unwrap_or_default();
                if date.is_empty() {
                    continue; // 无日期脏数据跳过
                }
                // 保留首次创建时间
                let created: i64 = conn
                    .query_row("SELECT created_at FROM events WHERE id = ?1", params![id], |r| r.get(0))
                    .unwrap_or(now);
                conn.execute(
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
            Ok(())
        }
        RaftCommand::DeleteEvents { ids } => {
            for id in ids {
                conn.execute("DELETE FROM events WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        RaftCommand::SetKv { key, value } => kv_upsert(conn, key, value),
        RaftCommand::SetClusterConfig { value } => kv_upsert(conn, "cluster.config", &value.to_string()),
    }
}

// ---------- 离线积压对账(快照恢复保护) ----------

/// 快照恢复后对账 pending 队列:
/// 快照重建会覆盖本地离线写入, 逐条对账——丢弃已在本地的命令, 保留仍需重放的,
/// 避免重放全量旧命令重新打 updated_at 或吞掉离线写入。
pub fn prune_pending(conn: &Connection) -> Result<(), String> {
    let raw = match kv_get(conn, "cluster.pending")? {
        Some(r) => r,
        None => return Ok(()),
    };
    let cmds: Vec<RaftCommand> = match serde_json::from_str(&raw) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let mut kept = Vec::new();
    for cmd in cmds {
        match cmd {
            RaftCommand::UpsertEvents { events } => {
                let mut pending_events = Vec::new();
                for ev in events {
                    let id = norm_id(&ev);
                    let sig = event_signature(&ev);
                    let local: Option<String> = conn
                        .query_row(
                            "SELECT date, end_date, start_time, end_time, title, event_type, description, is_all_day FROM events WHERE id = ?1",
                            params![id],
                            |row| {
                                let partial = serde_json::json!({
                                    "date": row.get::<_, String>(0)?,
                                    "endDate": row.get::<_, Option<String>>(1)?,
                                    "startTime": row.get::<_, Option<f64>>(2)?,
                                    "endTime": row.get::<_, Option<f64>>(3)?,
                                    "title": row.get::<_, String>(4)?,
                                    "type": row.get::<_, String>(5)?,
                                    "description": row.get::<_, String>(6)?,
                                    "isAllDay": row.get::<_, i64>(7)? != 0,
                                });
                                Ok(event_signature(&partial))
                            },
                        )
                        .ok();
                    if local.as_deref() != Some(sig.as_str()) {
                        pending_events.push(ev);
                    }
                }
                if !pending_events.is_empty() {
                    kept.push(RaftCommand::UpsertEvents { events: pending_events });
                }
            }
            RaftCommand::DeleteEvents { ids } => {
                let mut rest = Vec::new();
                for id in ids {
                    let exists: bool = conn
                        .query_row("SELECT 1 FROM events WHERE id = ?1", params![id], |_| Ok(true))
                        .unwrap_or(false);
                    if exists {
                        rest.push(id);
                    }
                }
                if !rest.is_empty() {
                    kept.push(RaftCommand::DeleteEvents { ids: rest });
                }
            }
            other => kept.push(other), // SetKv / SetClusterConfig 保留
        }
    }

    kv_upsert(
        conn,
        "cluster.pending",
        &serde_json::to_string(&kept).map_err(|e| e.to_string())?,
    )
}

// ---------- 单元测试 ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE events (
                id TEXT PRIMARY KEY, date TEXT NOT NULL, end_date TEXT,
                start_time REAL, end_time REAL, title TEXT DEFAULT '',
                event_type TEXT DEFAULT 'task', description TEXT DEFAULT '',
                is_all_day INTEGER DEFAULT 0, updated_at INTEGER, created_at INTEGER,
                deleted INTEGER DEFAULT 0);
             CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        conn
    }

    fn ev(id: i64, date: &str, title: &str) -> Value {
        json!({ "id": id, "date": date, "startTime": 9.0, "endTime": 10.0, "title": title, "type": "task", "isAllDay": false })
    }

    #[test]
    fn diff_and_apply_roundtrip() {
        let conn = mem();
        // 初次: 2 条全部新增
        let d = diff_events(&conn, &[ev(1, "2026-10-24", "a"), ev(2, "2026-10-25", "b")]).unwrap();
        assert_eq!(d.upserts.len(), 2);
        assert!(d.deletes.is_empty());
        for c in [RaftCommand::UpsertEvents { events: d.upserts.clone() }] {
            apply_command(&conn, &c).unwrap();
        }
        // 无变化: diff 为空
        let d2 = diff_events(&conn, &[ev(1, "2026-10-24", "a"), ev(2, "2026-10-25", "b")]).unwrap();
        assert!(d2.is_empty());
        // 改标题: 仅 1 条 upsert, 应用它
        let d3 = diff_events(&conn, &[ev(1, "2026-10-24", "a2"), ev(2, "2026-10-25", "b")]).unwrap();
        assert_eq!(d3.upserts.len(), 1);
        assert!(d3.deletes.is_empty());
        apply_command(&conn, &RaftCommand::UpsertEvents { events: d3.upserts.clone() }).unwrap();
        // 删除: 1 条 delete
        let d4 = diff_events(&conn, &[ev(1, "2026-10-24", "a2")]).unwrap();
        assert!(d4.upserts.is_empty());
        assert_eq!(d4.deletes.len(), 1);
    }

    #[test]
    fn created_at_preserved_and_updated_bumped() {
        let conn = mem();
        apply_command(&conn, &RaftCommand::UpsertEvents { events: vec![ev(5, "2026-10-24", "x")] }).unwrap();
        let (c1, u1): (i64, i64) = conn
            .query_row("SELECT created_at, updated_at FROM events WHERE id='5'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        apply_command(&conn, &RaftCommand::UpsertEvents { events: vec![ev(5, "2026-10-24", "x2")] }).unwrap();
        let (c2, u2): (i64, i64) = conn
            .query_row("SELECT created_at, updated_at FROM events WHERE id='5'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(c1, c2, "created_at 必须保留");
        assert!(u2 > u1, "updated_at 必须更新");
    }

    #[test]
    fn prune_pending_keeps_unapplied_only() {
        let conn = mem();
        // 本地有 e1(与队列一致) 和 e3(队列里但本地被快照覆盖丢了); e2 队列要求删除但本地已无
        apply_command(&conn, &RaftCommand::UpsertEvents { events: vec![ev(1, "2026-10-24", "a"), ev(3, "2026-10-26", "c")] }).unwrap();
        // 模拟快照覆盖: e3 被删
        conn.execute("DELETE FROM events WHERE id = '3'", []).unwrap();
        kv_upsert(&conn, "cluster.pending", &serde_json::to_string(&vec![
            RaftCommand::UpsertEvents { events: vec![ev(1, "2026-10-24", "a"), ev(3, "2026-10-26", "c")] },
            RaftCommand::DeleteEvents { ids: vec!["2".to_string()] },
        ]).unwrap()).unwrap();

        prune_pending(&conn).unwrap();
        let raw = kv_get(&conn, "cluster.pending").unwrap().unwrap();
        let kept: Vec<RaftCommand> = serde_json::from_str(&raw).unwrap();
        match &kept[0] {
            RaftCommand::UpsertEvents { events } => {
                assert_eq!(events.len(), 1, "已在本地的 e1 应丢弃, 被覆盖的 e3 应保留");
                assert_eq!(get_str(&events[0], "title").unwrap(), "c");
            }
            _ => panic!(),
        }
        assert_eq!(kept.len(), 1, "目标已不存在的删除命令应丢弃");
    }

    #[test]
    fn range_routing_splits_events() {
        let conn = mem();
        // 建库: 范围内 1 条 + 范围外 1 条
        apply_command(&conn, &RaftCommand::UpsertEvents { events: vec![ev(1, "2026-10-24", "in"), ev(2, "2020-01-01", "out")] }).unwrap();
        // 设置范围
        apply_command(&conn, &RaftCommand::SetClusterConfig { value: json!({ "syncSince": "2025-01-01" }) }).unwrap();
        // 新增一内一外 + 删除一外
        let diff = diff_events(&conn, &[ev(1, "2026-10-24", "in"), ev(3, "2026-11-01", "in3"), ev(4, "2019-01-01", "out4")]).unwrap();
        let routed = route_diff(&conn, &diff).unwrap();
        match &routed.cluster[0] {
            RaftCommand::UpsertEvents { events } => {
                assert_eq!(events.len(), 1, "仅范围内新事件进集群");
                assert_eq!(get_str(&events[0], "title").unwrap(), "in3");
            }
            _ => panic!(),
        }
        assert_eq!(routed.cluster.len(), 1, "范围外删除(out)不进集群");
        match &routed.local[0] {
            RaftCommand::UpsertEvents { events } => assert_eq!(events.len(), 1),
            _ => panic!(),
        }
        // 删除范围外事件(2) → 本地路径
        let diff2 = diff_events(&conn, &[ev(1, "2026-10-24", "in"), ev(3, "2026-11-01", "in3"), ev(4, "2019-01-01", "out4")].iter().filter(|e| get_str(e, "title") != Some("out".into())).cloned().collect::<Vec<_>>()).unwrap();
        assert!(diff2.deletes.contains(&"2".to_string()));
        let routed2 = route_diff(&conn, &diff2).unwrap();
        match routed2.local.iter().find(|c| matches!(c, RaftCommand::DeleteEvents { .. })) {
            Some(RaftCommand::DeleteEvents { ids }) => assert_eq!(ids, &vec!["2".to_string()]),
            _ => panic!("范围外删除应走本地路径"),
        }
    }
}
