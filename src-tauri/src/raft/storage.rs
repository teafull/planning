// Raft 存储适配器（openraft 0.9.x）：日志/状态机/快照 全部落 SQLite
// - raft_log 表: 日志(index, term, entry JSON)
// - raft_state 表: vote / committed / last_purged / sm 状态
// - 状态机 apply → 复用 raft_cmd::apply_command 写 events/kv 表
// - 快照: VACUUM INTO 全量导出; 安装时解析重建(仅复制同步范围内的数据)
// - rusqlite Connection 非 Sync, 统一用 Mutex 包裹(操作均为毫秒级, 可接受)
use std::collections::Bound;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Mutex;

use openraft::storage::{LogFlushed, RaftLogStorage, RaftStateMachine};
use openraft::{
    AnyError, BasicNode, Entry, EntryPayload, ErrorSubject, ErrorVerb, LogId, LogState,
    RaftLogReader, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError, StorageIOError,
    StoredMembership, Vote,
};
use rusqlite::{params, Connection, OptionalExtension};

use crate::raft_cmd::{apply_command, is_replicated_key};

use super::{ApplyResult, NodeId, TypeConfig};

// 简单字符串错误, 供 AnyError 包装
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct SErr(String);

fn err(e: impl std::fmt::Display) -> StorageError<NodeId> {
    StorageError::IO {
        source: StorageIOError::new(
            ErrorSubject::Store,
            ErrorVerb::Write,
            AnyError::new(&SErr(e.to_string())),
        ),
    }
}

fn rkv_set(conn: &Connection, key: &str, value: &str) -> Result<(), StorageError<NodeId>> {
    conn.execute(
        "INSERT INTO raft_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(err)
}

fn rkv_get(conn: &Connection, key: &str) -> Result<Option<String>, StorageError<NodeId>> {
    conn.query_row("SELECT value FROM raft_state WHERE key = ?1", params![key], |r| {
        r.get::<_, String>(0)
    })
    .optional()
    .map_err(err)
}

fn ensure_tables(conn: &Connection) -> Result<(), StorageError<NodeId>> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS raft_log (
            idx   INTEGER PRIMARY KEY,
            term  INTEGER NOT NULL,
            entry TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS raft_state (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(err)?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
    Ok(())
}

fn range_to_bounds<RB: RangeBounds<u64>>(range: &RB) -> (u64, u64) {
    let start = match range.start_bound() {
        Bound::Included(s) => *s,
        Bound::Excluded(s) => s.saturating_add(1),
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(e) => e.saturating_add(1),
        Bound::Excluded(e) => *e,
        Bound::Unbounded => u64::MAX,
    };
    (start, end)
}

fn open_conn(path: &str) -> Result<Connection, StorageError<NodeId>> {
    let conn = Connection::open(path).map_err(err)?;
    ensure_tables(&conn)?;
    Ok(conn)
}

fn conn_path(conn: &Connection) -> String {
    conn.path().map(|s| s.to_string()).unwrap_or_default()
}

// ---------- 日志存储 ----------

pub struct LogStore {
    conn: Mutex<Connection>,
}

impl LogStore {
    pub fn open(path: &Path) -> Result<Self, StorageError<NodeId>> {
        let conn = open_conn(path.to_str().unwrap_or_default())?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        Ok(Self { conn: Mutex::new(conn) })
    }
}

fn read_log_range(conn: &Connection, start: u64, end: u64) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
    let mut stmt = conn
        .prepare("SELECT entry FROM raft_log WHERE idx >= ?1 AND idx < ?2 ORDER BY idx")
        .map_err(err)?;
    let rows = stmt
        .query_map(params![start, end], |r| r.get::<_, String>(0))
        .map_err(err)?;
    let mut out = Vec::new();
    for r in rows {
        let s = r.map_err(err)?;
        out.push(serde_json::from_str(&s).map_err(err)?);
    }
    Ok(out)
}

impl RaftLogReader<TypeConfig> for LogStore {
    async fn try_get_log_entries<RB>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>>
    where
        RB: RangeBounds<u64> + Clone + std::fmt::Debug + openraft::OptionalSend,
    {
        let conn = self.conn.lock().unwrap();
        let (start, end) = range_to_bounds(&range);
        read_log_range(&conn, start, end)
    }
}

impl RaftLogStorage<TypeConfig> for LogStore {
    type LogReader = LogStoreReader;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        let last: Option<String> = conn
            .query_row(
                "SELECT entry FROM raft_log ORDER BY idx DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(err)?;
        let last_log_id = match last {
            Some(s) => {
                let e: Entry<TypeConfig> = serde_json::from_str(&s).map_err(err)?;
                Some(e.log_id)
            }
            None => None,
        };
        let last_purged = match rkv_get(&conn, "last_purged")? {
            Some(s) => Some(serde_json::from_str(&s).map_err(err)?),
            None => None,
        };
        Ok(LogState { last_purged_log_id: last_purged, last_log_id })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        let path = conn_path(&self.conn.lock().unwrap());
        LogStoreReader {
            conn: Mutex::new(open_conn(&path).expect("打开日志读连接失败")),
        }
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        rkv_set(&conn, "vote", &serde_json::to_string(vote).map_err(err)?)
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        match rkv_get(&conn, "vote")? {
            Some(s) => Ok(Some(serde_json::from_str(&s).map_err(err)?)),
            None => Ok(None),
        }
    }

    async fn save_committed(&mut self, committed: Option<LogId<NodeId>>) -> Result<(), StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        match committed {
            Some(id) => rkv_set(&conn, "committed", &serde_json::to_string(&id).map_err(err)?),
            None => rkv_set(&conn, "committed", ""),
        }
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        match rkv_get(&conn, "committed")? {
            Some(s) if !s.is_empty() => Ok(Some(serde_json::from_str(&s).map_err(err)?)),
            _ => Ok(None),
        }
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfig>,
    ) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + openraft::OptionalSend,
        I::IntoIter: openraft::OptionalSend,
    {
        let res = {
            let conn = self.conn.lock().unwrap();
            (|| -> Result<(), StorageError<NodeId>> {
                for e in entries {
                    let s = serde_json::to_string(&e).map_err(err)?;
                    conn.execute(
                        "INSERT OR REPLACE INTO raft_log (idx, term, entry) VALUES (?1, ?2, ?3)",
                        params![e.log_id.index as i64, e.log_id.leader_id.term as i64, s],
                    )
                    .map_err(err)?;
                }
                Ok(())
            })()
        };
        callback.log_io_completed(res.map_err(|e| std::io::Error::other(e.to_string())));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM raft_log WHERE idx > ?1", params![log_id.index as i64])
            .map_err(err)?;
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM raft_log WHERE idx <= ?1", params![log_id.index as i64])
            .map_err(err)?;
        rkv_set(
            &conn,
            "last_purged",
            &serde_json::to_string(&log_id).map_err(err)?,
        )
    }
}

/// 独立只读日志读取器
pub struct LogStoreReader {
    conn: Mutex<Connection>,
}

impl RaftLogReader<TypeConfig> for LogStoreReader {
    async fn try_get_log_entries<RB>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>>
    where
        RB: RangeBounds<u64> + Clone + std::fmt::Debug + openraft::OptionalSend,
    {
        let conn = self.conn.lock().unwrap();
        let (start, end) = range_to_bounds(&range);
        read_log_range(&conn, start, end)
    }
}

// ---------- 状态机存储 ----------

pub struct StateMachineStore {
    conn: Mutex<Connection>,
    /// 最近一次构建/安装的快照(内存缓存, 重启后由 leader 重发)
    snapshot: Mutex<Option<Snapshot<TypeConfig>>>,
    snap_dir: std::path::PathBuf,
    /// 应用句柄: 集群复制应用后通知前端刷新(单机模式下不用)
    app: Option<tauri::AppHandle>,
}

fn now_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn load_last_membership(conn: &Connection) -> StoredMembership<NodeId, BasicNode> {
    rkv_get(conn, "sm.last_membership")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

impl StateMachineStore {
    pub fn open(
        path: &Path,
        snap_dir: std::path::PathBuf,
        app: Option<tauri::AppHandle>,
    ) -> Result<Self, StorageError<NodeId>> {
        let conn = open_conn(path.to_str().unwrap_or_default())?;
        std::fs::create_dir_all(&snap_dir).map_err(err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            snapshot: Mutex::new(None),
            snap_dir,
            app,
        })
    }

    fn sm_from_conn(&self, conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            snapshot: Mutex::new(None),
            snap_dir: self.snap_dir.clone(),
            app: self.app.clone(),
        }
    }
}

impl RaftStateMachine<TypeConfig> for StateMachineStore {
    type SnapshotBuilder = StateMachineStore;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogId<NodeId>>,
            StoredMembership<NodeId, BasicNode>,
        ),
        StorageError<NodeId>,
    > {
        let conn = self.conn.lock().unwrap();
        let applied = match rkv_get(&conn, "sm.last_applied")? {
            Some(s) => Some(serde_json::from_str(&s).map_err(err)?),
            None => None,
        };
        Ok((applied, load_last_membership(&conn)))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<ApplyResult>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + openraft::OptionalSend,
        I::IntoIter: openraft::OptionalSend,
    {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        let mut normal_applied: u64 = 0;
        for e in entries {
            match &e.payload {
                EntryPayload::Blank => {}
                EntryPayload::Normal(cmd) => {
                    apply_command(&conn, cmd).map_err(err)?;
                    normal_applied += 1;
                }
                EntryPayload::Membership(m) => {
                    let sm = StoredMembership::new(Some(e.log_id), m.clone());
                    rkv_set(
                        &conn,
                        "sm.last_membership",
                        &serde_json::to_string(&sm).map_err(err)?,
                    )?;
                }
            }
            rkv_set(
                &conn,
                "sm.last_applied",
                &serde_json::to_string(&e.log_id).map_err(err)?,
            )?;
            results.push(());
        }
        // 集群复制产生了数据变更 → 通知前端刷新
        if normal_applied > 0 {
            if let Some(app) = &self.app {
                use tauri::Emitter;
                let _ = app.emit("raft-applied", normal_applied);
            }
        }
        Ok(results)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        let path = conn_path(&self.conn.lock().unwrap());
        let conn = open_conn(&path).expect("打开快照构建连接失败");
        self.sm_from_conn(conn)
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<NodeId>> {
        let bytes = snapshot.into_inner();
        {
            let conn = self.conn.lock().unwrap();
            restore_from_snapshot_bytes(&conn, &bytes)?;
            // 快照可能覆盖本地离线写入: 对账 pending, 保留仍需重放的命令
            let _ = crate::raft_cmd::prune_pending(&conn);
            rkv_set(
                &conn,
                "sm.last_applied",
                &serde_json::to_string(&meta.last_log_id).map_err(err)?,
            )?;
            rkv_set(
                &conn,
                "sm.last_membership",
                &serde_json::to_string(&meta.last_membership).map_err(err)?,
            )?;
        }
        *self.snapshot.lock().unwrap() = Some(Snapshot {
            meta: meta.clone(),
            snapshot: Box::new(Cursor::new(bytes)),
        });
        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        Ok(self.snapshot.lock().unwrap().clone())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for StateMachineStore {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let conn = self.conn.lock().unwrap();
        let stamp = now_stamp();
        let tmp = self.snap_dir.join(format!("snap-{stamp}.db"));
        let _ = std::fs::remove_file(&tmp);
        conn.execute("VACUUM INTO ?1", params![tmp.to_string_lossy()])
            .map_err(err)?;
        let bytes = std::fs::read(&tmp).map_err(err)?;
        let _ = std::fs::remove_file(&tmp);

        let applied: LogId<NodeId> = match rkv_get(&conn, "sm.last_applied")? {
            Some(s) => serde_json::from_str(&s).map_err(err)?,
            None => return Err(err("build_snapshot: 无 applied 状态")),
        };
        let meta = SnapshotMeta {
            snapshot_id: format!("snap-{stamp}"),
            last_log_id: Some(applied),
            last_membership: load_last_membership(&conn),
        };
        let snap = Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(bytes)),
        };
        *self.snapshot.lock().unwrap() = Some(snap.clone());
        Ok(snap)
    }
}

/// 从快照字节重建状态机: 仅恢复 同步范围数据(events + 复制键), 本地键(settings等)不动
fn restore_from_snapshot_bytes(conn: &Connection, bytes: &[u8]) -> Result<(), StorageError<NodeId>> {
    let stamp = now_stamp();
    let tmp = std::env::temp_dir().join(format!("planning-snap-restore-{stamp}.db"));
    std::fs::write(&tmp, bytes).map_err(err)?;
    let src = Connection::open_with_flags(&tmp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(err)?;

    let res = (|| -> Result<(), StorageError<NodeId>> {
        conn.execute_batch("BEGIN; DELETE FROM events;").map_err(err)?;

        {
            let mut stmt = src
                .prepare("SELECT id, date, end_date, start_time, end_time, title, event_type, description, is_all_day, updated_at, created_at FROM events WHERE deleted = 0")
                .map_err(err)?;
            let mut rows = stmt.query([]).map_err(err)?;
            while let Some(r) = rows.next().map_err(err)? {
                conn.execute(
                    "INSERT OR REPLACE INTO events (id, date, end_date, start_time, end_time, title, event_type, description, is_all_day, updated_at, created_at, deleted)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,0)",
                    params![
                        r.get::<_, String>(0).map_err(err)?,
                        r.get::<_, String>(1).map_err(err)?,
                        r.get::<_, Option<String>>(2).map_err(err)?,
                        r.get::<_, Option<f64>>(3).map_err(err)?,
                        r.get::<_, Option<f64>>(4).map_err(err)?,
                        r.get::<_, String>(5).map_err(err)?,
                        r.get::<_, String>(6).map_err(err)?,
                        r.get::<_, String>(7).map_err(err)?,
                        r.get::<_, i64>(8).map_err(err)?,
                        r.get::<_, i64>(9).map_err(err)?,
                        r.get::<_, i64>(10).map_err(err)?,
                    ],
                )
                .map_err(err)?;
            }
        }

        // 复制键: 先清本地复制键, 再从快照恢复
        {
            let mut sel = conn.prepare("SELECT key FROM kv").map_err(err)?;
            let mut rows = sel.query([]).map_err(err)?;
            let mut keys = Vec::new();
            while let Some(r) = rows.next().map_err(err)? {
                let k: String = r.get(0).map_err(err)?;
                if is_replicated_key(&k) {
                    keys.push(k);
                }
            }
            drop(rows);
            drop(sel);
            for k in keys {
                conn.execute("DELETE FROM kv WHERE key = ?1", params![k]).map_err(err)?;
            }
        }
        {
            let mut stmt = src.prepare("SELECT key, value FROM kv").map_err(err)?;
            let mut rows = stmt.query([]).map_err(err)?;
            while let Some(r) = rows.next().map_err(err)? {
                let k: String = r.get(0).map_err(err)?;
                let v: String = r.get(1).map_err(err)?;
                if is_replicated_key(&k) {
                    conn.execute(
                        "INSERT INTO kv (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        params![k, v],
                    )
                    .map_err(err)?;
                }
            }
        }

        conn.execute_batch("COMMIT;").map_err(err)?;
        Ok(())
    })();

    let _ = std::fs::remove_file(&tmp);
    if res.is_err() {
        let _ = conn.execute_batch("ROLLBACK;");
    }
    res
}
