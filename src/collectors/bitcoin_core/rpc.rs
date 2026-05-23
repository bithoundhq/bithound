//! `BitcoinCoreRpcCollector` — V0's first concrete `PollingCollector`.
//!
//! Per poll: four RPC calls (`getblockchaininfo`, `getmempoolinfo`,
//! `getnetworkinfo`, `getpeerinfo`) issued in parallel via
//! `tokio::join!`. Worst-case wall time per poll is one
//! `timeout_per_rpc` window rather than four. Bitcoin Core's default
//! `rpcworkers=16` handles four concurrent reads comfortably.
//!
//! Results are processed in spec order (blockchain, mempool, network,
//! peers) so the output is deterministic regardless of which call
//! settles first. Successful calls emit a state observation + a
//! health observation (latency stamped). If any call fails the batch
//! becomes `ProbeResult::Failed`; the first failure by spec order
//! drives the `health` + `error` fields, every successful observation
//! still lands in `partial_observations`.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use thiserror::Error;

use super::rpc_client::{
    BitcoinRpcClient, GetBlockchainInfoResponse, GetMempoolInfoResponse, GetNetworkInfoResponse,
    GetPeerInfoResponse, RpcError,
};
use crate::collectors::registry::BitcoinNodeConnection;
use crate::collectors::traits::PollingCollector;
use crate::collectors::{CollectionContext, CollectionError, CollectorDescriptor, CollectorTarget};
use crate::observations::{
    Attributes, BitcoinBlockchainState, BitcoinMempoolState, BitcoinNetworkState,
    BitcoinPeerSummaryState, HealthCheckObservation, HealthStatus, HealthTargetId, Observation,
    ObservationBatch, ObservationContext, ObservationOrigin, ObservationSource, ProbeResult,
    ProbeWindow, StateObservation,
};
use crate::shared::types::{BitcoinNodeId, EntityRef, ObservationBatchId, SidecarId};

#[derive(Debug, Clone)]
pub struct BitcoinCoreRpcCollectorConfig {
    pub timeout_per_rpc: Duration,
}

impl Default for BitcoinCoreRpcCollectorConfig {
    fn default() -> Self {
        Self {
            timeout_per_rpc: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("invalid RPC URL: {0}")]
    InvalidUrl(String),
    #[error("collector target must be BitcoinNode, got {0:?}")]
    WrongTargetKind(CollectorTarget),
}

#[derive(Debug)]
pub struct BitcoinCoreRpcCollector {
    descriptor: CollectorDescriptor,
    node_id: BitcoinNodeId,
    client: BitcoinRpcClient,
}

impl BitcoinCoreRpcCollector {
    /// Validates URL shape; never hits the network (ADR-C3 §C3.5).
    pub fn new(
        descriptor: CollectorDescriptor,
        connection: BitcoinNodeConnection,
        http: reqwest::Client,
        config: BitcoinCoreRpcCollectorConfig,
    ) -> Result<Self, BuildError> {
        reqwest::Url::parse(&connection.rpc_url)
            .map_err(|e| BuildError::InvalidUrl(format!("{}: {}", connection.rpc_url, e)))?;

        let node_id = match &descriptor.target {
            CollectorTarget::BitcoinNode(id) => id.clone(),
            other => return Err(BuildError::WrongTargetKind(other.clone())),
        };

        let client = BitcoinRpcClient::new(
            connection.rpc_url,
            connection.rpc_auth,
            http,
            config.timeout_per_rpc,
        );

        Ok(Self {
            descriptor,
            node_id,
            client,
        })
    }

    fn obs_context(
        &self,
        sidecar_id: &SidecarId,
        observed_at: chrono::DateTime<Utc>,
    ) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: sidecar_id.clone(),
                collector: self.descriptor.as_ref(),
            },
            subject: EntityRef::BitcoinNode(self.node_id.clone()),
            observed_at,
            origin: ObservationOrigin::Collected,
        }
    }

    fn build_batch(
        &self,
        sidecar_id: SidecarId,
        window: ProbeWindow,
        result: ProbeResult,
    ) -> ObservationBatch {
        ObservationBatch {
            id: ObservationBatchId::new(),
            collector: self.descriptor.as_ref(),
            sidecar_id,
            window,
            result,
        }
    }
}

#[async_trait]
impl PollingCollector for BitcoinCoreRpcCollector {
    fn descriptor(&self) -> &CollectorDescriptor {
        &self.descriptor
    }

    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch {
        let started_at = Utc::now();

        // Fire all four RPCs concurrently. Each closure captures its
        // own start/end timestamps so per-call latency stays accurate
        // even though the futures interleave on the executor.
        let (bc, mp, nw, pi) = tokio::join!(
            timed(self.client.get_blockchain_info()),
            timed(self.client.get_mempool_info()),
            timed(self.client.get_network_info()),
            timed(self.client.get_peer_info()),
        );

        let mut partials: Vec<Observation> = Vec::with_capacity(8);
        let mut first_failure: Option<(
            &'static str,
            chrono::DateTime<Utc>,
            chrono::DateTime<Utc>,
            RpcError,
        )> = None;

        // Process in spec order so observation order and the
        // "first failure" used for the health/error fields stay
        // deterministic regardless of completion order.

        let (s, e, r) = bc;
        match r {
            Ok(info) => {
                partials.push(Observation::state(
                    self.obs_context(&ctx.sidecar_id, s),
                    StateObservation::BitcoinBlockchain(blockchain_state(info)),
                    empty_attrs(),
                ));
                partials.push(Observation::health(
                    self.obs_context(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_BLOCKCHAIN),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => first_failure = Some((HEALTH_BLOCKCHAIN, s, e, err)),
        }

        let (s, e, r) = mp;
        match r {
            Ok(info) => {
                partials.push(Observation::state(
                    self.obs_context(&ctx.sidecar_id, s),
                    StateObservation::BitcoinMempool(mempool_state(info)),
                    empty_attrs(),
                ));
                partials.push(Observation::health(
                    self.obs_context(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_MEMPOOL),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some((HEALTH_MEMPOOL, s, e, err));
                }
            }
        }

        let (s, e, r) = nw;
        match r {
            Ok(info) => {
                partials.push(Observation::state(
                    self.obs_context(&ctx.sidecar_id, s),
                    StateObservation::BitcoinNetwork(network_state(info)),
                    empty_attrs(),
                ));
                partials.push(Observation::health(
                    self.obs_context(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_NETWORK),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some((HEALTH_NETWORK, s, e, err));
                }
            }
        }

        let (s, e, r) = pi;
        match r {
            Ok(peers) => {
                partials.push(Observation::state(
                    self.obs_context(&ctx.sidecar_id, s),
                    StateObservation::BitcoinPeerSummary(peer_summary_state(&peers)),
                    empty_attrs(),
                ));
                partials.push(Observation::health(
                    self.obs_context(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_PEERS),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some((HEALTH_PEERS, s, e, err));
                }
            }
        }

        if let Some((target, observed_at, completed_at, err)) = first_failure {
            return self.failed(
                &ctx,
                started_at,
                target,
                err,
                partials,
                observed_at,
                completed_at,
            );
        }

        let window = safe_probe_window(started_at, Utc::now());
        self.build_batch(
            ctx.sidecar_id.clone(),
            window,
            ProbeResult::Ok {
                observations: partials,
            },
        )
    }
}

/// Wrap a future so its start and end timestamps travel alongside the
/// result. Used by `poll` to stamp per-call latency on health
/// observations under `tokio::join!`.
async fn timed<F: std::future::Future>(
    future: F,
) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>, F::Output) {
    let start = Utc::now();
    let result = future.await;
    let end = Utc::now();
    (start, end, result)
}

impl BitcoinCoreRpcCollector {
    #[allow(clippy::too_many_arguments)]
    fn failed(
        &self,
        ctx: &CollectionContext,
        started_at: chrono::DateTime<Utc>,
        target: &str,
        err: RpcError,
        partials: Vec<Observation>,
        observed_at: chrono::DateTime<Utc>,
        completed_at: chrono::DateTime<Utc>,
    ) -> ObservationBatch {
        let kind = err.collection_error_kind();
        let message = err.to_string();
        let latency_ms = duration_ms(observed_at, completed_at);

        // `target` is one of the HEALTH_* constants from this module
        // (see HEALTH_TARGETS / first_failure tuples above), each of
        // which satisfies the parse rule guarded by the test below.
        let health = HealthCheckObservation {
            target: HealthTargetId::parse(target)
                .expect("HEALTH_* constants satisfy the parse rule"),
            status: HealthStatus::Critical,
            latency_ms,
            message: Some(message.clone()),
            error: None,
        };

        let batch_end = Utc::now();
        let window = safe_probe_window(started_at, batch_end.max(completed_at));
        self.build_batch(
            ctx.sidecar_id.clone(),
            window,
            ProbeResult::Failed {
                health,
                partial_observations: partials,
                error: CollectionError { kind, message },
            },
        )
    }
}

pub const HEALTH_BLOCKCHAIN: &str = "bitcoin.rpc.getblockchaininfo";
pub const HEALTH_MEMPOOL: &str = "bitcoin.rpc.getmempoolinfo";
pub const HEALTH_NETWORK: &str = "bitcoin.rpc.getnetworkinfo";
pub const HEALTH_PEERS: &str = "bitcoin.rpc.getpeerinfo";

/// The four RPC health-target names emitted by [`BitcoinCoreRpcCollector`],
/// in canonical order. Rules and tests reference this slice so a renamed
/// target in the collector can't drift silently from its consumers.
pub const HEALTH_TARGETS: &[&str] = &[
    HEALTH_BLOCKCHAIN,
    HEALTH_MEMPOOL,
    HEALTH_NETWORK,
    HEALTH_PEERS,
];

fn empty_attrs() -> Attributes {
    Attributes(std::collections::BTreeMap::new())
}

/// Construct a `ProbeWindow` defensively. A backwards clock jump
/// between `started_at` and `completed_at` collapses to a zero-width
/// window pinned at the later instant instead of panicking — matches
/// the failure path's old fallback so the success path doesn't crash
/// the poll task on NTP correction.
fn safe_probe_window(
    started_at: chrono::DateTime<Utc>,
    completed_at: chrono::DateTime<Utc>,
) -> ProbeWindow {
    ProbeWindow::new(started_at, completed_at)
        .unwrap_or_else(|_| ProbeWindow::new(completed_at, completed_at).unwrap())
}

fn duration_ms(from: chrono::DateTime<Utc>, to: chrono::DateTime<Utc>) -> Option<u64> {
    let ms = (to - from).num_milliseconds();
    if ms < 0 {
        None
    } else {
        Some(ms as u64)
    }
}

fn blockchain_state(r: GetBlockchainInfoResponse) -> BitcoinBlockchainState {
    BitcoinBlockchainState {
        chain: r.chain,
        blocks: r.blocks,
        headers: r.headers,
        best_block_hash: r.bestblockhash,
        verification_progress: r.verificationprogress,
        initial_block_download: r.initialblockdownload,
        pruned: r.pruned,
        size_on_disk_bytes: r.size_on_disk,
    }
}

fn mempool_state(r: GetMempoolInfoResponse) -> BitcoinMempoolState {
    BitcoinMempoolState {
        loaded: r.loaded,
        tx_count: r.size,
        bytes: r.bytes,
        usage_bytes: r.usage,
        max_mempool_bytes: r.maxmempool,
    }
}

fn network_state(r: GetNetworkInfoResponse) -> BitcoinNetworkState {
    BitcoinNetworkState {
        version: r.version,
        subversion: r.subversion,
        protocol_version: r.protocolversion,
        connections: r.connections,
        connections_in: r.connections_in,
        connections_out: r.connections_out,
        network_active: r.networkactive,
    }
}

fn peer_summary_state(peers: &GetPeerInfoResponse) -> BitcoinPeerSummaryState {
    let peer_count = peers.len() as u64;
    let inbound_count = peers.iter().filter(|p| p.inbound).count() as u64;
    let outbound_count = peer_count - inbound_count;
    // Bitcoin Core ≥ 0.21 exposes `connection_type`; older versions omit
    // it. If no peer in the response carries the field, treat the count
    // as unknown rather than emitting Some(0), which would silently look
    // like "no block-relay-only peers" to downstream rules.
    let connection_type_supported =
        !peers.is_empty() && peers.iter().any(|p| p.connection_type.is_some());
    let block_relay_only_count = if connection_type_supported {
        Some(
            peers
                .iter()
                .filter(|p| p.connection_type.as_deref() == Some("block-relay-only"))
                .count() as u64,
        )
    } else {
        None
    };
    BitcoinPeerSummaryState {
        peer_count,
        inbound_count: Some(inbound_count),
        outbound_count: Some(outbound_count),
        block_relay_only_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::registry::{BitcoinNodeConnection, BitcoinRpcAuth};
    use crate::collectors::{CollectionRunId, CollectorTarget, IntegrationKind};
    use crate::observations::{ObservationPayload, StateObservation};
    use crate::shared::types::{BitcoinNodeId, CollectorId, HostId, ObservationId, SidecarId};
    use chrono::Duration as ChronoDuration;
    use secrecy::SecretString;
    use std::net::SocketAddr;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Notify;
    use uuid::Uuid;

    enum Reply {
        Json(serde_json::Value),
        /// Like `Json` but sleeps before responding. Lets parallelism
        /// tests detect whether four 200ms RPCs take ~200ms (parallel)
        /// or ~800ms (sequential).
        DelayedJson(std::time::Duration, serde_json::Value),
        Status(u16),
        Hang,
    }

    /// Mock server whose handler closes over an iteration counter so it
    /// can give a different reply per request — useful for "third RPC
    /// fails" scenarios.
    async fn spawn_mock(
        handler: impl Fn(&str, usize) -> Reply + Send + Sync + 'static,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handler = Arc::new(handler);
        let count = Arc::new(AtomicUsize::new(0));
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(p) => p,
                    Err(_) => return,
                };
                let handler = handler.clone();
                let count = count.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                    let parsed = serde_json::from_str::<serde_json::Value>(&req[body_start..])
                        .unwrap_or(serde_json::Value::Null);
                    let method = parsed
                        .get("method")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                        .unwrap_or_default();
                    let request_id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    let idx = count.fetch_add(1, Ordering::SeqCst);
                    match handler(&method, idx) {
                        Reply::Json(mut value) => {
                            // Echo the request id into the response envelope so the
                            // client's id-correlation check passes by default.
                            if let Some(obj) = value.as_object_mut() {
                                obj.insert("id".to_string(), request_id);
                            }
                            let body = serde_json::to_vec(&value).unwrap();
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            socket.write_all(response.as_bytes()).await.ok();
                            socket.write_all(&body).await.ok();
                        }
                        Reply::DelayedJson(delay, mut value) => {
                            tokio::time::sleep(delay).await;
                            if let Some(obj) = value.as_object_mut() {
                                obj.insert("id".to_string(), request_id);
                            }
                            let body = serde_json::to_vec(&value).unwrap();
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            socket.write_all(response.as_bytes()).await.ok();
                            socket.write_all(&body).await.ok();
                        }
                        Reply::Status(code) => {
                            let response = format!(
                                "HTTP/1.1 {} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                code
                            );
                            socket.write_all(response.as_bytes()).await.ok();
                        }
                        Reply::Hang => {
                            Notify::new().notified().await;
                        }
                    }
                });
            }
        });
        addr
    }

    fn descriptor() -> CollectorDescriptor {
        CollectorDescriptor {
            id: CollectorId("bitcoin-rpc".into()),
            integration: IntegrationKind::BitcoinCoreRpc {
                interval: ChronoDuration::seconds(10),
            },
            target: CollectorTarget::BitcoinNode(BitcoinNodeId("alice".into())),
            instance_label: "alice".into(),
            description: None,
        }
    }

    fn connection(addr: SocketAddr) -> BitcoinNodeConnection {
        BitcoinNodeConnection {
            rpc_url: format!("http://{}/", addr),
            rpc_auth: BitcoinRpcAuth::UserPass {
                user: "rpcuser".into(),
                pass: SecretString::from("rpcpass".to_string()),
            },
            zmq_endpoint: None,
        }
    }

    fn ctx() -> CollectionContext {
        CollectionContext {
            sidecar_id: SidecarId(Uuid::now_v7()),
            collector_id: CollectorId("bitcoin-rpc".into()),
            target: CollectorTarget::BitcoinNode(BitcoinNodeId("alice".into())),
            now: Utc::now(),
            run_id: CollectionRunId(Uuid::now_v7()),
        }
    }

    fn ok_envelope(result: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "result": result, "error": null, "id": "bithound" })
    }

    fn ok_blockchain() -> serde_json::Value {
        ok_envelope(serde_json::json!({
            "chain": "main",
            "blocks": 800_000,
            "headers": 800_000,
            "bestblockhash": "deadbeef",
            "verificationprogress": 1.0,
            "initialblockdownload": false,
            "pruned": false,
            "size_on_disk": 500_000_000_000u64,
        }))
    }

    fn ok_mempool() -> serde_json::Value {
        ok_envelope(serde_json::json!({
            "loaded": true,
            "size": 100,
            "bytes": 1000,
            "usage": 2000,
            "maxmempool": 300_000_000,
        }))
    }

    fn ok_network() -> serde_json::Value {
        ok_envelope(serde_json::json!({
            "version": 250_000,
            "subversion": "/Satoshi:25.0.0/",
            "protocolversion": 70016,
            "connections": 8,
            "connections_in": 1,
            "connections_out": 7,
            "networkactive": true,
        }))
    }

    fn ok_peers() -> serde_json::Value {
        ok_envelope(serde_json::json!([
            { "inbound": false, "addr": "1.2.3.4:8333", "subver": "/Satoshi:25.0.0/", "connection_type": "outbound-full-relay" },
            { "inbound": true,  "addr": "5.6.7.8:8333", "subver": "/Satoshi:24.0.0/", "connection_type": "inbound" },
            { "inbound": false, "addr": "9.9.9.9:8333", "subver": "/Satoshi:25.0.0/", "connection_type": "block-relay-only" },
        ]))
    }

    // ── new() validation ────────────────────────────────────────────

    #[test]
    fn new_rejects_invalid_url() {
        let conn = BitcoinNodeConnection {
            rpc_url: "not a url".into(),
            rpc_auth: BitcoinRpcAuth::UserPass {
                user: "u".into(),
                pass: SecretString::from("p".to_string()),
            },
            zmq_endpoint: None,
        };
        let err = BitcoinCoreRpcCollector::new(
            descriptor(),
            conn,
            reqwest::Client::new(),
            Default::default(),
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::InvalidUrl(_)));
    }

    #[test]
    fn new_rejects_wrong_target_kind() {
        let mut desc = descriptor();
        desc.target = CollectorTarget::Host(HostId("h".into()));
        let conn = BitcoinNodeConnection {
            rpc_url: "http://127.0.0.1:18443/".into(),
            rpc_auth: BitcoinRpcAuth::UserPass {
                user: "u".into(),
                pass: SecretString::from("p".to_string()),
            },
            zmq_endpoint: None,
        };
        let err =
            BitcoinCoreRpcCollector::new(desc, conn, reqwest::Client::new(), Default::default())
                .unwrap_err();
        assert!(matches!(err, BuildError::WrongTargetKind(_)));
    }

    #[test]
    fn new_does_not_hit_the_network() {
        // Use a clearly unroutable URL — if `new` tried to dial, this
        // would hang or error. We expect immediate success because
        // validation is shape-only per ADR-C3 §C3.5.
        let conn = BitcoinNodeConnection {
            rpc_url: "http://203.0.113.1:18443/".into(),
            rpc_auth: BitcoinRpcAuth::UserPass {
                user: "u".into(),
                pass: SecretString::from("p".to_string()),
            },
            zmq_endpoint: None,
        };
        BitcoinCoreRpcCollector::new(
            descriptor(),
            conn,
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor is offline");
    }

    // ── happy path ──────────────────────────────────────────────────

    #[tokio::test]
    async fn successful_poll_emits_four_state_and_four_health_observations() {
        let addr = spawn_mock(|method, _| match method {
            "getblockchaininfo" => Reply::Json(ok_blockchain()),
            "getmempoolinfo" => Reply::Json(ok_mempool()),
            "getnetworkinfo" => Reply::Json(ok_network()),
            "getpeerinfo" => Reply::Json(ok_peers()),
            other => panic!("unexpected method {}", other),
        })
        .await;

        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");

        let batch = collector.poll(ctx()).await;

        let observations = match batch.result {
            ProbeResult::Ok { observations } => observations,
            ProbeResult::Failed { error, .. } => panic!("expected Ok, got Failed: {:?}", error),
        };

        assert_eq!(observations.len(), 8);
        let state_count = observations
            .iter()
            .filter(|o| matches!(o.payload, ObservationPayload::State(_)))
            .count();
        let health_count = observations
            .iter()
            .filter(|o| matches!(o.payload, ObservationPayload::Health(_)))
            .count();
        assert_eq!(state_count, 4);
        assert_eq!(health_count, 4);

        // Confirm we got each expected state variant exactly once.
        let mut saw_blockchain = false;
        let mut saw_mempool = false;
        let mut saw_network = false;
        let mut saw_peer = false;
        for obs in &observations {
            if let ObservationPayload::State(s) = &obs.payload {
                match s {
                    StateObservation::BitcoinBlockchain(_) => saw_blockchain = true,
                    StateObservation::BitcoinMempool(_) => saw_mempool = true,
                    StateObservation::BitcoinNetwork(_) => saw_network = true,
                    StateObservation::BitcoinPeerSummary(s) => {
                        saw_peer = true;
                        assert_eq!(s.peer_count, 3);
                        assert_eq!(s.inbound_count, Some(1));
                        assert_eq!(s.outbound_count, Some(2));
                        assert_eq!(s.block_relay_only_count, Some(1));
                    }
                    other => panic!("unexpected state variant {:?}", other),
                }
            }
        }
        assert!(saw_blockchain && saw_mempool && saw_network && saw_peer);
    }

    #[tokio::test]
    async fn poll_issues_rpcs_in_parallel_not_serial() {
        // Each RPC sleeps 200ms before responding. Sequential execution
        // would take ~800ms total; parallel execution should complete in
        // roughly one delay window. Bound the assertion well below the
        // sequential floor and well above expected parallel runtime to
        // stay reliable on a busy CI host.
        const RPC_DELAY: std::time::Duration = std::time::Duration::from_millis(200);
        let addr = spawn_mock(|method, _| match method {
            "getblockchaininfo" => Reply::DelayedJson(RPC_DELAY, ok_blockchain()),
            "getmempoolinfo" => Reply::DelayedJson(RPC_DELAY, ok_mempool()),
            "getnetworkinfo" => Reply::DelayedJson(RPC_DELAY, ok_network()),
            "getpeerinfo" => Reply::DelayedJson(RPC_DELAY, ok_peers()),
            other => panic!("unexpected method {}", other),
        })
        .await;

        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");

        let start = std::time::Instant::now();
        let batch = collector.poll(ctx()).await;
        let elapsed = start.elapsed();

        assert!(matches!(batch.result, ProbeResult::Ok { .. }));
        // Sequential floor is 4 × 200ms = 800ms. A well-functioning
        // parallel poll lands near 200ms; allow a generous 600ms ceiling
        // so a slow CI runner still passes while a regression to serial
        // RPC issuance trips the bound.
        assert!(
            elapsed < std::time::Duration::from_millis(600),
            "poll took {:?} — expected parallel execution (<600ms), got near-sequential",
            elapsed
        );
    }

    // ── failure paths ──────────────────────────────────────────────

    #[tokio::test]
    async fn first_rpc_failure_returns_failed_with_zero_partials() {
        let addr = spawn_mock(|_method, _idx| Reply::Status(500)).await;
        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");

        let batch = collector.poll(ctx()).await;
        match batch.result {
            ProbeResult::Failed {
                health,
                partial_observations,
                error,
            } => {
                assert_eq!(partial_observations.len(), 0);
                assert_eq!(health.target.as_str(), HEALTH_BLOCKCHAIN);
                assert_eq!(error.kind, CollectionErrorKind::ProtocolError);
            }
            ProbeResult::Ok { .. } => panic!("expected Failed"),
        }
    }

    #[tokio::test]
    async fn third_rpc_failure_returns_failed_with_four_partials() {
        let addr = spawn_mock(|method, _idx| match method {
            "getblockchaininfo" => Reply::Json(ok_blockchain()),
            "getmempoolinfo" => Reply::Json(ok_mempool()),
            "getnetworkinfo" => Reply::Status(500),
            _ => Reply::Status(500),
        })
        .await;

        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");

        let batch = collector.poll(ctx()).await;
        match batch.result {
            ProbeResult::Failed {
                health,
                partial_observations,
                error,
            } => {
                assert_eq!(
                    partial_observations.len(),
                    4,
                    "state+health for the first two RPCs"
                );
                assert_eq!(health.target.as_str(), HEALTH_NETWORK);
                assert_eq!(error.kind, CollectionErrorKind::ProtocolError);
            }
            ProbeResult::Ok { .. } => panic!("expected Failed"),
        }
    }

    #[tokio::test]
    async fn timeout_on_first_rpc_maps_to_timeout_error_kind() {
        let addr = spawn_mock(|_, _| Reply::Hang).await;
        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            BitcoinCoreRpcCollectorConfig {
                timeout_per_rpc: Duration::from_millis(100),
            },
        )
        .expect("ctor");
        let batch = collector.poll(ctx()).await;
        match batch.result {
            ProbeResult::Failed { error, .. } => {
                assert_eq!(error.kind, CollectionErrorKind::Timeout);
            }
            ProbeResult::Ok { .. } => panic!("expected Failed"),
        }
    }

    #[tokio::test]
    async fn auth_failure_maps_to_authentication_failed_error_kind() {
        let addr = spawn_mock(|_, _| Reply::Status(401)).await;
        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            connection(addr),
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");
        let batch = collector.poll(ctx()).await;
        match batch.result {
            ProbeResult::Failed { error, .. } => {
                assert_eq!(error.kind, CollectionErrorKind::AuthenticationFailed);
            }
            ProbeResult::Ok { .. } => panic!("expected Failed"),
        }
    }

    // ── RpcError → CollectionErrorKind mapping (direct) ────────────

    #[test]
    fn rpc_error_kind_mapping_covers_every_variant() {
        assert_eq!(
            RpcError::Timeout.collection_error_kind(),
            CollectionErrorKind::Timeout
        );
        assert_eq!(
            RpcError::Auth.collection_error_kind(),
            CollectionErrorKind::AuthenticationFailed
        );
        assert_eq!(
            RpcError::HttpStatus(500).collection_error_kind(),
            CollectionErrorKind::ProtocolError
        );
        assert_eq!(
            RpcError::BitcoindError {
                code: -5,
                message: "x".into()
            }
            .collection_error_kind(),
            CollectionErrorKind::InvalidResponse
        );
        // Network and Decode are constructed from underlying types; their
        // mappings are exercised via the integration paths above.
    }

    // Unused-in-this-test imports we still want to keep referenced.
    use crate::collectors::CollectionErrorKind;
    #[allow(dead_code)]
    fn _suppress_unused_obs_id() -> ObservationId {
        ObservationId::new()
    }

    // ── regtest integration (env-gated) ────────────────────────────

    /// Integration test against a real regtest Bitcoin Core node.
    /// Gated behind `BITHOUND_TEST_REGTEST_URL` (e.g.
    /// `http://rpcuser:rpcpass@127.0.0.1:18443`). CI skips when unset.
    #[tokio::test]
    async fn regtest_integration() {
        let Ok(url) = std::env::var("BITHOUND_TEST_REGTEST_URL") else {
            return;
        };
        let parsed = reqwest::Url::parse(&url).expect("BITHOUND_TEST_REGTEST_URL parse");
        let user = parsed.username().to_string();
        let pass = parsed.password().unwrap_or("").to_string();
        let mut bare = parsed.clone();
        bare.set_username("").ok();
        bare.set_password(None).ok();

        let conn = BitcoinNodeConnection {
            rpc_url: bare.to_string(),
            rpc_auth: BitcoinRpcAuth::UserPass {
                user,
                pass: SecretString::from(pass),
            },
            zmq_endpoint: None,
        };
        let collector = BitcoinCoreRpcCollector::new(
            descriptor(),
            conn,
            reqwest::Client::new(),
            Default::default(),
        )
        .expect("ctor");
        let batch = collector.poll(ctx()).await;
        match batch.result {
            ProbeResult::Ok { observations } => {
                assert_eq!(observations.len(), 8);
            }
            ProbeResult::Failed { error, .. } => {
                panic!("regtest poll failed: {} ({:?})", error.message, error.kind);
            }
        }
    }
}
