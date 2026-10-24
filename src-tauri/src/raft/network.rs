// HTTP 网络层: openraft RaftNetwork 实现(出站, reqwest)
// 入站路由见 server.rs, 与此处 URL 约定一致: /raft/{vote|append-entries|snapshot}
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::error::{RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::{BasicNode};
use std::time::Duration;

use super::{NodeId, TypeConfig};

#[derive(Clone, Default)]
pub struct HttpNetworkFactory;

pub struct HttpNetwork {
    base_url: String,
    client: reqwest::Client,
}

impl RaftNetworkFactory<TypeConfig> for HttpNetworkFactory {
    type Network = HttpNetwork;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        HttpNetwork {
            base_url: node.addr.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl HttpNetwork {
    fn unreachable<E: std::fmt::Display>(e: E) -> RPCError<NodeId, BasicNode, RaftError<NodeId>> {
        RPCError::Unreachable(Unreachable::new(&SErrNet(e.to_string())))
    }

    async fn post<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        body: &Req,
        timeout: Duration,
    ) -> Result<Res, String> {
        let url = format!("http://{}/raft/{path}", self.base_url);
        self.client
            .post(&url)
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct SErrNet(String);

impl RaftNetwork<TypeConfig> for HttpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post("append-entries", &rpc, option.hard_ttl())
            .await
            .map_err(Self::unreachable)
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId, openraft::error::InstallSnapshotError>>>
    {
        self.post("snapshot", &rpc, Duration::from_secs(60))
            .await
            .map_err(|e| {
                RPCError::Unreachable(Unreachable::new(&SErrNet(e.to_string())))
            })
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post("vote", &rpc, option.hard_ttl())
            .await
            .map_err(Self::unreachable)
    }
}
