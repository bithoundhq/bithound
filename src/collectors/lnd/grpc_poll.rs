//! `LndGrpcPollingCollector` — V0.8's first concrete LND collector.
//!
//! Per poll: three RPCs (`GetInfo`, `ListChannels`, `ListPeers`)
//! fired in parallel via `tokio::join!`. `ListPeers` is cross-
//! referenced into `LndChannelState.peer_online` at observation-build
//! time, so failed `ListPeers` lands as a partial batch with channel
//! observations still emitted (with `peer_online = None`).
//!
//! Mirrors the Bitcoin polling collector shape: parallel RPCs,
//! spec-order processing, per-RPC health observations, partial-
//! failure preservation.

use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use thiserror::Error;

use super::grpc_client::{BuildError as ClientBuildError, LndGrpcClient, LndRpcError};
use super::lnrpc::{Channel, GetInfoResponse, ListChannelsResponse, ListPeersResponse};
use crate::collectors::helpers::{duration_ms, empty_attrs, safe_probe_window, timed};
use crate::collectors::registry::LndNodeConnection;
use crate::collectors::traits::PollingCollector;
use crate::collectors::{CollectionContext, CollectionError, CollectorDescriptor, CollectorTarget};
use crate::observations::{
    HealthCheckObservation, HealthStatus, HealthTargetId, LndChannelState, LndChannelSummaryState,
    LndNodeState, Observation, ObservationBatch, ObservationContext, ObservationOrigin,
    ObservationSource, ProbeResult, ProbeWindow, StateObservation,
};
use crate::shared::types::{EntityRef, LndChannelId, LndNodeId, ObservationBatchId, SidecarId};

#[derive(Debug, Clone)]
pub struct LndGrpcPollingCollectorConfig {
    pub timeout_per_rpc: Duration,
}

impl Default for LndGrpcPollingCollectorConfig {
    fn default() -> Self {
        Self {
            timeout_per_rpc: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("collector target must be LndNode, got {0:?}")]
    WrongTargetKind(CollectorTarget),
    #[error("client build failed: {0}")]
    Client(#[from] ClientBuildError),
}

#[derive(Debug)]
pub struct LndGrpcPollingCollector {
    descriptor: CollectorDescriptor,
    node_id: LndNodeId,
    client: LndGrpcClient,
}

impl LndGrpcPollingCollector {
    pub fn new(
        descriptor: CollectorDescriptor,
        connection: LndNodeConnection,
        config: LndGrpcPollingCollectorConfig,
    ) -> Result<Self, BuildError> {
        let node_id = match &descriptor.target {
            CollectorTarget::LndNode(id) => id.clone(),
            other => return Err(BuildError::WrongTargetKind(other.clone())),
        };

        let client = LndGrpcClient::new(
            connection.grpc_endpoint,
            connection.tls_cert_path,
            &connection.macaroon,
            config.timeout_per_rpc,
        )?;

        Ok(Self {
            descriptor,
            node_id,
            client,
        })
    }

    fn obs_context_for_node(
        &self,
        sidecar_id: &SidecarId,
        observed_at: chrono::DateTime<Utc>,
    ) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: sidecar_id.clone(),
                collector: self.descriptor.as_ref(),
            },
            subject: EntityRef::LndNode(self.node_id.clone()),
            observed_at,
            origin: ObservationOrigin::Collected,
        }
    }

    fn obs_context_for_channel(
        &self,
        sidecar_id: &SidecarId,
        channel_id: LndChannelId,
        observed_at: chrono::DateTime<Utc>,
    ) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: sidecar_id.clone(),
                collector: self.descriptor.as_ref(),
            },
            subject: EntityRef::LndChannel {
                node_id: self.node_id.clone(),
                channel_id,
            },
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
impl PollingCollector for LndGrpcPollingCollector {
    fn descriptor(&self) -> &CollectorDescriptor {
        &self.descriptor
    }

    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch {
        let started_at = Utc::now();

        let (gi, lc, lp) = tokio::join!(
            timed(self.client.get_info()),
            timed(self.client.list_channels()),
            timed(self.client.list_peers()),
        );

        let mut partials: Vec<Observation> = Vec::new();
        let mut first_failure: Option<(
            &'static str,
            chrono::DateTime<Utc>,
            chrono::DateTime<Utc>,
            LndRpcError,
        )> = None;

        // GetInfo
        let (s, e, r) = gi;
        match r {
            Ok(info) => {
                partials.push(Observation::state(
                    self.obs_context_for_node(&ctx.sidecar_id, s),
                    StateObservation::LndNode(node_state_from(info)),
                    empty_attrs(),
                ));
                partials.push(Observation::health(
                    self.obs_context_for_node(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_GET_INFO),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => first_failure = Some((HEALTH_GET_INFO, s, e, err)),
        }

        // ListPeers joined into a lookup first so the ListChannels
        // branch can stamp peer_online without re-borrowing or
        // chaining await points.
        let (lp_s, lp_e, lp_result) = lp;
        let list_peers_failed = lp_result.is_err();
        let connected_peer_pubkeys: HashSet<String> = match &lp_result {
            Ok(resp) => connected_peer_set(resp),
            Err(_) => HashSet::new(),
        };

        // ListChannels
        let (s, e, r) = lc;
        match r {
            Ok(resp) => {
                partials.push(Observation::state(
                    self.obs_context_for_node(&ctx.sidecar_id, s),
                    StateObservation::LndChannelSummary(channel_summary_from(&resp)),
                    empty_attrs(),
                ));

                for channel in &resp.channels {
                    // Skip channels without a funding outpoint. LND
                    // returns empty `channel_point` for transient
                    // pending states; the EntityRef would be ambiguous
                    // and downstream rules can't key off it.
                    if channel.channel_point.is_empty() {
                        continue;
                    }
                    let channel_id = LndChannelId(channel.channel_point.clone());
                    let peer_online = if list_peers_failed {
                        None
                    } else {
                        Some(connected_peer_pubkeys.contains(&channel.remote_pubkey))
                    };
                    partials.push(Observation::state(
                        self.obs_context_for_channel(&ctx.sidecar_id, channel_id, s),
                        StateObservation::LndChannel(channel_state_from(channel, peer_online)),
                        empty_attrs(),
                    ));
                }

                partials.push(Observation::health(
                    self.obs_context_for_node(&ctx.sidecar_id, s),
                    HealthTargetId::from_well_known(HEALTH_LIST_CHANNELS),
                    HealthStatus::Ok,
                    duration_ms(s, e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some((HEALTH_LIST_CHANNELS, s, e, err));
                }
            }
        }

        // ListPeers health observation / failure record.
        match lp_result {
            Ok(_) => {
                partials.push(Observation::health(
                    self.obs_context_for_node(&ctx.sidecar_id, lp_s),
                    HealthTargetId::from_well_known(HEALTH_LIST_PEERS),
                    HealthStatus::Ok,
                    duration_ms(lp_s, lp_e),
                    None,
                    None,
                    empty_attrs(),
                ));
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some((HEALTH_LIST_PEERS, lp_s, lp_e, err));
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

impl LndGrpcPollingCollector {
    #[allow(clippy::too_many_arguments)]
    fn failed(
        &self,
        ctx: &CollectionContext,
        started_at: chrono::DateTime<Utc>,
        target: &str,
        err: LndRpcError,
        partials: Vec<Observation>,
        observed_at: chrono::DateTime<Utc>,
        completed_at: chrono::DateTime<Utc>,
    ) -> ObservationBatch {
        let kind = err.collection_error_kind();
        let message = err.to_string();
        let latency_ms = duration_ms(observed_at, completed_at);

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

fn node_state_from(info: GetInfoResponse) -> LndNodeState {
    LndNodeState {
        identity_pubkey: info.identity_pubkey,
        alias: if info.alias.is_empty() {
            None
        } else {
            Some(info.alias)
        },
        version: if info.version.is_empty() {
            None
        } else {
            Some(info.version)
        },
        num_active_channels: u64::from(info.num_active_channels),
        num_inactive_channels: Some(u64::from(info.num_inactive_channels)),
        num_pending_channels: u64::from(info.num_pending_channels),
        num_peers: u64::from(info.num_peers),
        block_height: u64::from(info.block_height),
        synced_to_chain: info.synced_to_chain,
        synced_to_graph: Some(info.synced_to_graph),
    }
}

fn channel_summary_from(resp: &ListChannelsResponse) -> LndChannelSummaryState {
    let mut active = 0u64;
    let mut inactive = 0u64;
    let mut total_capacity_sat: i64 = 0;
    let mut local_balance_sat: i64 = 0;
    let mut remote_balance_sat: i64 = 0;
    let mut unsettled_balance_sat: i64 = 0;
    for c in &resp.channels {
        if c.active {
            active += 1;
        } else {
            inactive += 1;
        }
        total_capacity_sat += c.capacity;
        local_balance_sat += c.local_balance;
        remote_balance_sat += c.remote_balance;
        unsettled_balance_sat += c.unsettled_balance;
    }
    LndChannelSummaryState {
        active_channels: active,
        inactive_channels: inactive,
        pending_channels: 0,
        total_capacity_sat: Some(total_capacity_sat.max(0) as u64),
        local_balance_sat: local_balance_sat.max(0) as u64,
        remote_balance_sat: remote_balance_sat.max(0) as u64,
        unsettled_balance_sat: Some(unsettled_balance_sat.max(0) as u64),
    }
}

fn channel_state_from(channel: &Channel, peer_online: Option<bool>) -> LndChannelState {
    // csv_delay is marked deprecated upstream but still populated;
    // v0.8 retains it as an informational field. Suppress the
    // deprecation warning narrowly here.
    #[allow(deprecated)]
    let csv_delay = channel.csv_delay;
    LndChannelState {
        remote_pubkey: channel.remote_pubkey.clone(),
        capacity_sat: channel.capacity.max(0) as u64,
        local_balance_sat: channel.local_balance.max(0) as u64,
        remote_balance_sat: channel.remote_balance.max(0) as u64,
        active: channel.active,
        private: channel.private,
        initiator: channel.initiator,
        csv_delay,
        commit_fee_sat: channel.commit_fee.max(0) as u64,
        lifetime_seconds: channel.lifetime.max(0) as u64,
        last_update_height: None,
        short_channel_id: if channel.chan_id == 0 {
            None
        } else {
            Some(channel.chan_id.to_string())
        },
        peer_online,
    }
}

fn connected_peer_set(resp: &ListPeersResponse) -> HashSet<String> {
    resp.peers.iter().map(|p| p.pub_key.clone()).collect()
}

pub const HEALTH_GET_INFO: &str = "lnd.rpc.get_info";
pub const HEALTH_LIST_CHANNELS: &str = "lnd.rpc.list_channels";
pub const HEALTH_LIST_PEERS: &str = "lnd.rpc.list_peers";

pub const HEALTH_TARGETS: &[&str] = &[HEALTH_GET_INFO, HEALTH_LIST_CHANNELS, HEALTH_LIST_PEERS];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_targets_satisfy_parse_rule() {
        for target in HEALTH_TARGETS {
            HealthTargetId::parse(target)
                .unwrap_or_else(|e| panic!("HEALTH_* constant {target:?} fails parse: {e}"));
        }
    }

    #[test]
    fn health_targets_are_unique() {
        let mut sorted = HEALTH_TARGETS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), HEALTH_TARGETS.len());
    }
}
