// 集群 HTTP 服务: raft RPC 入站 + 管理接口(加入/提升/提议/状态)
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
    InstallSnapshotResponse, VoteRequest, VoteResponse,
};
use openraft::{BasicNode, ChangeMembers};
use serde::{Deserialize, Serialize};

use super::{NodeId, RaftHandle, TypeConfig};

#[derive(Clone)]
pub struct AppState {
    pub handle: std::sync::Arc<RaftHandle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    pub token: String,
    pub raft_id: NodeId,
    pub addr: String, // 新节点通告地址 ip:port
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub ok: bool,
    pub msg: String,
    pub members: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromoteRequest {
    pub raft_id: NodeId,
    pub token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRequest {
    pub raft_id: NodeId,
    pub token: Option<String>,
}

async fn cluster_remove(
    State(app): State<AppState>,
    Json(req): Json<RemoveRequest>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "集群令牌错误".into() });
    }
    if crate::raft::metrics_snapshot(&app.handle.raft).state != openraft::ServerState::Leader {
        return Json(GenericResponse { ok: false, msg: "本节点不是 Leader".into() });
    }
    let voters: std::collections::BTreeSet<NodeId> = app
        .handle
        .raft
        .metrics()
        .borrow()
        .membership_config
        .nodes()
        .map(|(id, _)| *id)
        .collect();
    let remaining: std::collections::BTreeSet<NodeId> =
        voters.into_iter().filter(|id| *id != req.raft_id).collect();
    match app
        .handle
        .raft
        .change_membership(openraft::ChangeMembers::ReplaceAllVoters(remaining), true)
        .await
    {
        Ok(_) => Json(GenericResponse { ok: true, msg: format!("已移除节点 {}", req.raft_id) }),
        Err(e) => Json(GenericResponse { ok: false, msg: format!("{e}") }),
    }
}

#[derive(Deserialize)]
pub struct ProposeRequest {
    pub command: super::RaftCommand,
    pub token: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GenericResponse {
    pub ok: bool,
    pub msg: String,
}

// ---------- raft RPC(错误以 5xx/401 返回, 调用方视作 Unreachable) ----------

/// Raft RPC 令牌校验: 请求头 x-cluster-token 与本集群令牌一致才放行
fn rpc_authorized(handle: &std::sync::Arc<RaftHandle>, headers: &axum::http::HeaderMap) -> bool {
    match &handle.cluster_token {
        Some(expect) => headers
            .get("x-cluster-token")
            .and_then(|v| v.to_str().ok())
            .map(|got| got == expect)
            .unwrap_or(false),
        None => false, // 无令牌的节点不接受任何 RPC
    }
}

fn unauthorized() -> (axum::http::StatusCode, String) {
    (axum::http::StatusCode::UNAUTHORIZED, "集群令牌错误".to_string())
}

async fn append_entries(
    State(app): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(rpc): Json<AppendEntriesRequest<TypeConfig>>,
) -> Result<Json<AppendEntriesResponse<NodeId>>, (axum::http::StatusCode, String)> {
    if !rpc_authorized(&app.handle, &headers) {
        return Err(unauthorized());
    }
    app.handle
        .raft
        .append_entries(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn vote(
    State(app): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(rpc): Json<VoteRequest<NodeId>>,
) -> Result<Json<VoteResponse<NodeId>>, (axum::http::StatusCode, String)> {
    if !rpc_authorized(&app.handle, &headers) {
        return Err(unauthorized());
    }
    app.handle
        .raft
        .vote(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn install_snapshot(
    State(app): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(rpc): Json<InstallSnapshotRequest<TypeConfig>>,
) -> Result<Json<InstallSnapshotResponse<NodeId>>, (axum::http::StatusCode, String)> {
    if !rpc_authorized(&app.handle, &headers) {
        return Err(unauthorized());
    }
    app.handle
        .raft
        .install_snapshot(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ---------- 管理接口 ----------

/// 管理接口令牌校验(集群未设令牌时拒绝, 防止空令牌绕过)
fn token_ok(handle: &std::sync::Arc<RaftHandle>, provided: &Option<String>) -> bool {
    match (&handle.cluster_token, provided) {
        (Some(expect), Some(got)) => expect == got,
        _ => false,
    }
}

async fn cluster_join(
    State(app): State<AppState>,
    Json(req): Json<JoinRequest>,
) -> Json<JoinResponse> {
    let token_ok = app
        .handle
        .cluster_token
        .as_ref()
        .map(|t| t == &req.token)
        .unwrap_or(false);
    if !token_ok {
        return Json(JoinResponse { ok: false, msg: "集群令牌错误".into(), members: vec![] });
    }

    if crate::raft::metrics_snapshot(&app.handle.raft).state != openraft::ServerState::Leader {
        return Json(JoinResponse { ok: false, msg: "本节点不是 Leader, 请向 Leader 节点发起加入".into(), members: vec![] });
    }

    let node = BasicNode { addr: req.addr.clone() };
    match app.handle.raft.add_learner(req.raft_id, node, true).await {
        Ok(_) => {
            let members = app.handle.members_snapshot().await;
            Json(JoinResponse { ok: true, msg: "已添加为学习者".into(), members })
        }
        Err(e) => Json(JoinResponse { ok: false, msg: format!("{e}"), members: vec![] }),
    }
}

async fn cluster_promote(
    State(app): State<AppState>,
    Json(req): Json<PromoteRequest>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "集群令牌错误".into() });
    }
    let members = app.handle.members_snapshot().await;
    let mut ids: Vec<NodeId> = members
        .iter()
        .filter_map(|m| m.get("raftId").and_then(|v| v.as_u64()))
        .collect();
    if !ids.contains(&req.raft_id) {
        ids.push(req.raft_id);
    }
    let set: std::collections::BTreeSet<NodeId> = ids.into_iter().collect();
    match app
        .handle
        .raft
        .change_membership(ChangeMembers::ReplaceAllVoters(set), true)
        .await
    {
        Ok(_) => Json(GenericResponse { ok: true, msg: "已提升为投票成员".into() }),
        Err(e) => Json(GenericResponse { ok: false, msg: format!("{e}") }),
    }
}

async fn cluster_propose(
    State(app): State<AppState>,
    Json(req): Json<ProposeRequest>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "集群令牌错误".into() });
    }
    match app.handle.raft.client_write(req.command).await {
        Ok(_) => Json(GenericResponse { ok: true, msg: String::new() }),
        Err(e) => Json(GenericResponse { ok: false, msg: format!("{e}") }),
    }
}

async fn cluster_status(
    State(app): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if !rpc_authorized(&app.handle, &headers) {
        return Err(unauthorized());
    }
    let metrics = crate::raft::metrics_snapshot(&app.handle.raft);
    let members: Vec<serde_json::Value> = metrics
        .membership_config
        .nodes()
        .map(|(id, n)| serde_json::json!({ "raftId": id, "addr": n.addr }))
        .collect();
    Ok(Json(serde_json::json!({
        "nodeId": app.handle.node_id,
        "state": format!("{:?}", metrics.state),
        "currentTerm": metrics.current_term,
        "currentLeader": metrics.current_leader,
        "running": metrics.running_state.is_ok(),
        "membership": members,
        "lastApplied": metrics.last_applied.map(|l| l.index),
        "lastLogIndex": metrics.last_log_index,
    })))
}

// ---------- 测试端点(令牌保护, 用于自动化验证) ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCreateEvent {
    pub token: Option<String>,
    pub date: String,
    pub start_time: f64,
    pub end_time: f64,
    pub title: String,
    #[serde(default = "default_event_type")]
    pub event_type: String,
}
fn default_event_type() -> String { "task".into() }

#[derive(Serialize)]
pub struct TestEventsResponse {
    pub ok: bool,
    pub count: usize,
    pub events: Vec<serde_json::Value>,
}

async fn test_create_event(
    State(app): State<AppState>,
    Json(req): Json<TestCreateEvent>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "令牌错误".into() });
    }
    let event = serde_json::json!({
        "id": format!("test-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()),
        "date": req.date,
        "startTime": req.start_time,
        "endTime": req.end_time,
        "title": req.title,
        "type": req.event_type,
        "description": "auto-test",
        "isAllDay": false,
    });
    let cmd = crate::raft_cmd::RaftCommand::UpsertEvents { events: vec![event] };
    match app.handle.raft.client_write(cmd).await {
        Ok(_) => Json(GenericResponse { ok: true, msg: "created".into() }),
        Err(e) => {
            // 非 Leader → 转发到 Leader 的同名端点
            if let Some(fl) = e.forward_to_leader() {
                if let Some(node) = &fl.leader_node {
                    return Json(forward_to_leader_test(
                        &format!("http://{}/test/create-event", node.addr),
                        &serde_json::json!({
                            "token": req.token,
                            "date": req.date,
                            "startTime": req.start_time,
                            "endTime": req.end_time,
                            "title": req.title,
                            "eventType": req.event_type,
                        }),
                    ).await);
                }
            }
            Json(GenericResponse { ok: false, msg: format!("{e}") })
        }
    }
}

/// 转发测试请求到 Leader(异步辅助)
async fn forward_to_leader_test(url: &str, body: &serde_json::Value) -> GenericResponse {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().unwrap_or_default();
    match client.post(url).json(body).send().await {
        Ok(r) if r.status().is_success() => {
            r.json::<GenericResponse>().await.unwrap_or(
                GenericResponse { ok: false, msg: "Leader 响应解析失败".into() }
            )
        }
        Ok(r) => GenericResponse { ok: false, msg: format!("Leader 拒绝({})", r.status()) },
        Err(e) => GenericResponse { ok: false, msg: format!("转发失败: {e}") },
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestUpdateEvent {
    pub token: Option<String>,
    pub id: String,
    pub title: Option<String>,
    pub event_type: Option<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub date: Option<String>,
    pub description: Option<String>,
}

async fn test_update_event(
    State(app): State<AppState>,
    Json(req): Json<TestUpdateEvent>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "令牌错误".into() });
    }
    // 构造更新后的事件(先从 SQLite 读原事件, 合并变更)
    let db_path = find_db_path(&app.handle);
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => return Json(GenericResponse { ok: false, msg: format!("db: {e}") }),
    };
    let orig: Option<(String, Option<String>, Option<f64>, Option<f64>, String, String, String)> = conn.query_row(
        "SELECT date, end_date, start_time, end_time, title, event_type, description FROM events WHERE id = ?1",
        rusqlite::params![req.id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
    ).ok();
    let Some((date, end_date, st, et, title, ty, desc)) = orig else {
        return Json(GenericResponse { ok: false, msg: format!("事件 {} 不存在", req.id) });
    };
    drop(conn);

    let event = serde_json::json!({
        "id": req.id.clone(),
        "date": req.date.clone().unwrap_or(date),
        "endDate": end_date,
        "startTime": req.start_time.or(st),
        "endTime": req.end_time.or(et),
        "title": req.title.clone().unwrap_or(title),
        "type": req.event_type.clone().unwrap_or(ty),
        "description": req.description.clone().unwrap_or(desc),
        "isAllDay": false,
    });
    let cmd = crate::raft_cmd::RaftCommand::UpsertEvents { events: vec![event] };
    match app.handle.raft.client_write(cmd).await {
        Ok(_) => Json(GenericResponse { ok: true, msg: "updated".into() }),
        Err(e) => {
            if let Some(fl) = e.forward_to_leader() {
                if let Some(node) = &fl.leader_node {
                    let mut body = serde_json::json!({"token": req.token, "id": req.id});
                    if let Some(v) = req.title.as_ref() { body["title"] = serde_json::json!(v); }
                    if let Some(v) = req.event_type.as_ref() { body["eventType"] = serde_json::json!(v); }
                    if let Some(v) = req.start_time { body["startTime"] = serde_json::json!(v); }
                    if let Some(v) = req.end_time { body["endTime"] = serde_json::json!(v); }
                    if let Some(v) = req.date.as_ref() { body["date"] = serde_json::json!(v); }
                    if let Some(v) = req.description.as_ref() { body["description"] = serde_json::json!(v); }
                    return Json(forward_to_leader_test(
                        &format!("http://{}/test/update-event", node.addr), &body,
                    ).await);
                }
            }
            Json(GenericResponse { ok: false, msg: format!("{e}") })
        }
    }
}

#[derive(Deserialize)]
pub struct TestDeleteEvent {
    pub token: Option<String>,
    pub id: String,
}

async fn test_delete_event(
    State(app): State<AppState>,
    Json(req): Json<TestDeleteEvent>,
) -> Json<GenericResponse> {
    if !token_ok(&app.handle, &req.token) {
        return Json(GenericResponse { ok: false, msg: "令牌错误".into() });
    }
    let cmd = crate::raft_cmd::RaftCommand::DeleteEvents { ids: vec![req.id.clone()] };
    match app.handle.raft.client_write(cmd).await {
        Ok(_) => Json(GenericResponse { ok: true, msg: "deleted".into() }),
        Err(e) => {
            if let Some(fl) = e.forward_to_leader() {
                if let Some(node) = &fl.leader_node {
                    return Json(forward_to_leader_test(
                        &format!("http://{}/test/delete-event", node.addr),
                        &serde_json::json!({"token": req.token.clone(), "id": req.id.clone()}),
                    ).await);
                }
            }
            Json(GenericResponse { ok: false, msg: format!("{e}") })
        }
    }
}

async fn test_get_events(
    State(app): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<TestEventsResponse> {
    let token = params.get("token").cloned();
    if !token_ok(&app.handle, &token) {
        return Json(TestEventsResponse { ok: false, count: 0, events: vec![] });
    }
    // 直接读 SQLite(与 Raft 状态机共享同一文件)
    // db_path now obtained from handle directly
    // 通过环境/已知路径读取 — 更简单: 打开当前数据目录
    let conn = match rusqlite::Connection::open_with_flags(
        find_db_path(&app.handle),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(_) => return Json(TestEventsResponse { ok: false, count: 0, events: vec![] }),
    };
    let mut stmt = match conn.prepare(
        "SELECT id, date, start_time, end_time, title, event_type, description FROM events WHERE deleted = 0"
    ) {
        Ok(s) => s,
        Err(_) => return Json(TestEventsResponse { ok: false, count: 0, events: vec![] }),
    };
    let rows = match stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, String>(0)?,
            "date": r.get::<_, String>(1)?,
            "startTime": r.get::<_, Option<f64>>(2)?,
            "endTime": r.get::<_, Option<f64>>(3)?,
            "title": r.get::<_, String>(4)?,
            "type": r.get::<_, String>(5)?,
            "description": r.get::<_, String>(6)?,
        }))
    }) {
        Ok(rows) => rows,
        Err(_) => return Json(TestEventsResponse { ok: false, count: 0, events: vec![] }),
    };
    let events: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    let count = events.len();
    Json(TestEventsResponse { ok: true, count, events })
}

/// 从 RaftHandle 获取数据库路径(不再猜测)
fn find_db_path(handle: &std::sync::Arc<RaftHandle>) -> String {
    handle.db_path.to_string_lossy().to_string()
}

pub async fn start_server(
    handle: std::sync::Arc<RaftHandle>,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let addr = format!("0.0.0.0:{}", handle.listen_port);
    let state = AppState { handle: handle.clone() };

    let app = Router::new()
        .route("/raft/append-entries", post(append_entries))
        .route("/raft/vote", post(vote))
        .route("/raft/snapshot", post(install_snapshot))
        .route("/cluster/join", post(cluster_join))
        .route("/cluster/promote", post(cluster_promote))
        .route("/cluster/remove", post(cluster_remove))
        .route("/cluster/propose", post(cluster_propose))
        .route("/cluster/status", get(cluster_status))
        .route("/test/create-event", post(test_create_event))
        .route("/test/events", get(test_get_events))
        .route("/test/update-event", post(test_update_event))
        .route("/test/delete-event", post(test_delete_event))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("监听 {addr} 失败: {e}"))?;
    let jh = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(jh)
}
