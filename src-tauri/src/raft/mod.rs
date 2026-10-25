// Raft 集群管理: 节点生命周期(创建/加入) + 提议路由
// 第 2 期范围: 单节点启动 + HTTP 网络 + 命令提议; 成员管理完善与前端 UI 在第 3/4 期
pub mod network;
pub mod server;
pub mod storage;

use openraft::{BasicNode, Config, Raft};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

pub use crate::raft_cmd::RaftCommand;

pub type NodeId = u64;
pub type ApplyResult = ();

openraft::declare_raft_types!(
    pub TypeConfig:
        D = RaftCommand,
        R = ApplyResult,
        Node = BasicNode,
);

/// 运行中的 raft 节点句柄(Tauri State 管理)
pub struct RaftHandle {
    pub raft: Raft<TypeConfig>,
    pub node_id: NodeId,
    pub listen_port: u16,
    /// 集群令牌(校验加入请求)
    pub cluster_token: Option<String>,
}

impl RaftHandle {
    /// 当前成员表(从 metrics 读取)
    pub async fn members_snapshot(&self) -> Vec<serde_json::Value> {
        let m = metrics_snapshot(&self.raft).membership_config;
        m.nodes()
            .map(|(id, n)| serde_json::json!({ "raftId": id, "addr": n.addr }))
            .collect()
    }
}

/// 集群配置(持久化于 kv 'cluster.config' 的结构)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterConfig {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub sync_since: Option<String>,
    #[serde(default)]
    pub listen_port: Option<u16>,
    #[serde(default)]
    pub advertise_addr: Option<String>, // 本机通告地址 ip:port
    #[serde(default)]
    pub node_id: Option<NodeId>,
}

/// 安全读取 Raft 指标快照。
/// tokio watch 的 borrow() 在发送端持写锁的极小窗口内会 panic,
/// 此处以 catch_unwind + 重试兼容(实际触发概率极低, 触发时重试即成功)。
pub fn metrics_snapshot(raft: &Raft<TypeConfig>) -> openraft::RaftMetrics<NodeId, BasicNode> {
    let rx = raft.metrics();
    let mut spins: u32 = 0;
    loop {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rx.borrow().clone()
        }));
        if let Ok(m) = r {
            return m;
        }
        spins += 1;
        if spins < 100 {
            std::thread::yield_now();
        } else {
            // 持续冲突(理论场景): 退化为短睡眠, 避免热自旋
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

pub fn raft_config() -> Config {
    Config {
        cluster_name: "planning-cluster".into(),
        heartbeat_interval: 1000,
        election_timeout_min: 2000,
        election_timeout_max: 4000,
        snapshot_policy: openraft::SnapshotPolicy::LogsSinceLast(5000),
        max_in_snapshot_log_to_keep: 1000,
        ..Default::default()
    }
}

/// 启动 raft 节点(日志/状态机使用同一 SQLite 文件的独立连接)
pub async fn start_node(
    node_id: NodeId,
    db_path: &std::path::Path,
    snap_dir: std::path::PathBuf,
    listen_port: u16,
    cluster_token: Option<String>,
) -> Result<Arc<RaftHandle>, String> {
    let log_store = storage::LogStore::open(db_path).map_err(|e| e.to_string())?;
    let sm_store = storage::StateMachineStore::open(db_path, snap_dir).map_err(|e| e.to_string())?;

    let raft = Raft::new(
        node_id,
        Arc::new(raft_config()),
        network::HttpNetworkFactory::new(cluster_token.clone()),
        log_store,
        sm_store,
    )
    .await
    .map_err(|e| format!("Raft 启动失败: {e}"))?;

    Ok(Arc::new(RaftHandle {
        raft,
        node_id,
        listen_port,
        cluster_token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft_cmd::RaftCommand;
    use serde_json::json;

    fn temp_db(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("planning-raft-test-{stamp}-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("planning.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
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
        db
    }

    fn count_events(db: &std::path::Path, title: &str) -> i64 {
        let conn = rusqlite::Connection::open_with_flags(
            db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM events WHERE title = ?1",
            rusqlite::params![title],
            |r| r.get(0),
        )
        .unwrap()
    }

    async fn wait_for<F: Fn() -> bool>(cond: F, timeout_ms: u64) -> bool {
        let mut waited = 0u64;
        while waited < timeout_ms {
            if cond() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            waited += 200;
        }
        cond()
    }

    /// 三节点集群: 初始化 → 加入 → 提议 → 全节点验证复制
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn three_node_cluster_replicates() {
        let db1 = temp_db("n1");
        let db2 = temp_db("n2");
        let db3 = temp_db("n3");

        // 节点1: 创建单节点集群
        let n1 = start_node(1, &db1, db1.parent().unwrap().join("snap"), 17801, Some("e2e-token".into()))
            .await
            .unwrap();

        let mut m = std::collections::BTreeMap::new();
        m.insert(1u64, BasicNode { addr: "127.0.0.1:17801".into() });
        n1.raft.initialize(m).await.unwrap();
        server::start_server(n1.clone()).await.unwrap();
        // 负向鉴权: 无令牌/错误令牌访问管理端口应 401
        {
            let no_token = reqwest::Client::new()
                .get("http://127.0.0.1:17801/cluster/status")
                .send()
                .await
                .unwrap();
            assert_eq!(no_token.status(), 401, "无令牌应被拒绝");
            let bad = reqwest::Client::new()
                .post("http://127.0.0.1:17801/raft/vote")
                .header("x-cluster-token", "WRONG")
                .json(&serde_json::json!({ "vote": { "leader_id": { "term": 1, "node_id": 9 }, "committed": false }, "last_log_id": null }))
                .send()
                .await
                .unwrap();
            assert_eq!(bad.status(), 401, "错误令牌应被拒绝");
        }

        // 节点2/3: 启动(不初始化), 由 leader 添加为 learner
        let n2 = start_node(2, &db2, db2.parent().unwrap().join("snap"), 17802, Some("e2e-token".into()))
            .await
            .unwrap();
        server::start_server(n2.clone()).await.unwrap();
        let n3 = start_node(3, &db3, db3.parent().unwrap().join("snap"), 17803, Some("e2e-token".into()))
            .await
            .unwrap();
        server::start_server(n3.clone()).await.unwrap();

        n1.raft
            .add_learner(2, BasicNode { addr: "127.0.0.1:17802".into() }, true)
            .await
            .unwrap();
        n1.raft
            .add_learner(3, BasicNode { addr: "127.0.0.1:17803".into() }, true)
            .await
            .unwrap();

        // 转正为三节点投票集群
        let voters: std::collections::BTreeSet<u64> = [1u64, 2, 3].into_iter().collect();
        n1.raft
            .change_membership(openraft::ChangeMembers::ReplaceAllVoters(voters), true)
            .await
            .unwrap();

        // Leader 提议命令
        let cmd = RaftCommand::UpsertEvents {
            events: vec![json!({
                "id": "test-1", "date": "2026-10-24", "startTime": 9.0, "endTime": 10.0,
                "title": "集群复制测试", "type": "task", "isAllDay": false
            })],
        };
        n1.raft.client_write(cmd).await.unwrap();

        // 三库均应包含该事件
        let ok2 = wait_for(|| count_events(&db2, "集群复制测试") == 1, 8000).await;
        let ok3 = wait_for(|| count_events(&db3, "集群复制测试") == 1, 8000).await;
        assert!(count_events(&db1, "集群复制测试") == 1, "leader 本地未应用");
        assert!(ok2, "节点2未同步");
        assert!(ok3, "节点3未同步");

        // HTTP 管理接口合同测试: GET status + POST join(驼峰字段) —— 锁线格式
        {
            let c = reqwest::Client::new();
            let st = c
                .get("http://127.0.0.1:17801/cluster/status")
                .header("x-cluster-token", "e2e-token")
                .send().await.unwrap();
            assert_eq!(st.status(), 200);
            let body: serde_json::Value = st.json().await.unwrap();
            assert!(body["membership"].as_array().unwrap().len() >= 3, "成员表应含3节点");

            // 第四节点经 HTTP join: 请求体必须用驼峰 raftId
            let join = c
                .post("http://127.0.0.1:17801/cluster/join")
                .json(&serde_json::json!({
                    "token": "e2e-token",
                    "raftId": 4u64,
                    "addr": "127.0.0.1:19999"
                }))
                .send().await.unwrap();
            assert_eq!(join.status(), 200, "join 应 200");
            let jv: serde_json::Value = join.json().await.unwrap();
            assert!(jv["ok"].as_bool().unwrap(), "驼峰字段 join 必须成功: {jv}");
        }

        // 清理测试目录
        for db in [&db1, &db2, &db3] {
            let dir = db.parent().unwrap();
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
