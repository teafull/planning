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
pub struct PromoteRequest {
    pub raft_id: NodeId,
}

#[derive(Deserialize)]
pub struct RemoveRequest {
    pub raft_id: NodeId,
}

async fn cluster_remove(
    State(app): State<AppState>,
    Json(req): Json<RemoveRequest>,
) -> Json<GenericResponse> {
    if app.handle.raft.metrics().borrow().state != openraft::ServerState::Leader {
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
}

#[derive(Serialize)]
pub struct GenericResponse {
    pub ok: bool,
    pub msg: String,
}

// ---------- raft RPC(错误以 500 返回, 调用方视作 Unreachable) ----------

async fn append_entries(
    State(app): State<AppState>,
    Json(rpc): Json<AppendEntriesRequest<TypeConfig>>,
) -> Result<Json<AppendEntriesResponse<NodeId>>, (axum::http::StatusCode, String)> {
    app.handle
        .raft
        .append_entries(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn vote(
    State(app): State<AppState>,
    Json(rpc): Json<VoteRequest<NodeId>>,
) -> Result<Json<VoteResponse<NodeId>>, (axum::http::StatusCode, String)> {
    app.handle
        .raft
        .vote(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn install_snapshot(
    State(app): State<AppState>,
    Json(rpc): Json<InstallSnapshotRequest<TypeConfig>>,
) -> Result<Json<InstallSnapshotResponse<NodeId>>, (axum::http::StatusCode, String)> {
    app.handle
        .raft
        .install_snapshot(rpc)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ---------- 管理接口 ----------

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

    if app.handle.raft.metrics().borrow().state != openraft::ServerState::Leader {
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
    match app.handle.raft.client_write(req.command).await {
        Ok(_) => Json(GenericResponse { ok: true, msg: String::new() }),
        Err(e) => Json(GenericResponse { ok: false, msg: format!("{e}") }),
    }
}

async fn cluster_status(State(app): State<AppState>) -> Json<serde_json::Value> {
    let metrics = app.handle.raft.metrics().borrow().clone();
    let members: Vec<serde_json::Value> = metrics
        .membership_config
        .nodes()
        .map(|(id, n)| serde_json::json!({ "raftId": id, "addr": n.addr }))
        .collect();
    Json(serde_json::json!({
        "nodeId": app.handle.node_id,
        "state": format!("{:?}", metrics.state),
        "currentTerm": metrics.current_term,
        "currentLeader": metrics.current_leader,
        "running": metrics.running_state.is_ok(),
        "membership": members,
        "lastApplied": metrics.last_applied.map(|l| l.index),
        "lastLogIndex": metrics.last_log_index,
    }))
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("监听 {addr} 失败: {e}"))?;
    let jh = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(jh)
}
