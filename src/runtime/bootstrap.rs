//! Build runtime collector handles from the parsed config.
//!
//! Two entry points: one for polling collectors (consumed by the
//! supervisor's polling slot), one for subscription collectors (the
//! supervisor's subscription slot). Each takes the parsed config
//! and the `NodeRegistry` so it can resolve a collector's
//! target id into a concrete connection.

use std::collections::HashMap;

use chrono::Duration as ChronoDuration;
use thiserror::Error;

use crate::collectors::bitcoin_core::rpc::{
    BitcoinCoreRpcCollector, BitcoinCoreRpcCollectorConfig, BuildError as RpcBuildError,
};
use crate::collectors::lnd::grpc_poll::{
    BuildError as LndBuildError, LndGrpcPollingCollector, LndGrpcPollingCollectorConfig,
};
use crate::collectors::registry::{
    BitcoinNodeConnection, BitcoinRpcAuth, LndNodeConnection, NodeRegistry,
};
use crate::collectors::traits::{PollingCollector, SubscriptionCollector};
use crate::collectors::{CollectorDescriptor, CollectorTarget, IntegrationKind};
use crate::config::collectors::{
    CollectorDescriptorConfig, CollectorTargetConfig, IntegrationConfig,
};
use crate::config::targets::BitcoinAuthConfig;
use crate::config::Config;
use crate::config::ResolvedSecrets;
use crate::shared::types::{BitcoinNodeId, CollectorId, LndNodeId};

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("collector {id:?} targets unknown id {target:?}")]
    TargetNotFound { id: String, target: String },

    #[error(
        "collector {id:?} mixes target kind {target_kind} with integration kind {integration_kind}"
    )]
    WrongTargetKind {
        id: String,
        target_kind: &'static str,
        integration_kind: &'static str,
    },

    #[error("integration kind {0} is not implemented in V0")]
    NotImplemented(&'static str),

    #[error("secret env var {0} was not resolved")]
    SecretNotResolved(String),

    #[error("collector {id:?} build failed: {source}")]
    Inner {
        id: String,
        #[source]
        source: RpcBuildError,
    },

    #[error("LND collector {id:?} build failed: {source}")]
    InnerLnd {
        id: String,
        #[source]
        source: LndBuildError,
    },
}

/// Builds runtime polling collector handles from a slice of
/// `CollectorDescriptorConfig`s. Subscription-kind entries are
/// skipped — they go through `build_subscription_collectors`.
pub fn build_polling_collectors(
    configs: &[CollectorDescriptorConfig],
    registry: &NodeRegistry,
    http: &reqwest::Client,
) -> Result<Vec<Box<dyn PollingCollector>>, BuildError> {
    let mut out: Vec<Box<dyn PollingCollector>> = Vec::new();

    for cfg in configs {
        match &cfg.integration {
            IntegrationConfig::BitcoinCoreRpc { interval_seconds } => {
                let target = collector_target(&cfg.target);
                let node_id = match &target {
                    CollectorTarget::BitcoinNode(id) => id,
                    _ => {
                        return Err(BuildError::WrongTargetKind {
                            id: cfg.id.clone(),
                            target_kind: target_kind_name(&target),
                            integration_kind: "bitcoin_core_rpc",
                        });
                    }
                };
                let connection = registry
                    .bitcoin_nodes
                    .get(node_id)
                    .cloned()
                    .ok_or_else(|| BuildError::TargetNotFound {
                        id: cfg.id.clone(),
                        target: node_id.0.clone(),
                    })?;

                let descriptor = CollectorDescriptor {
                    id: CollectorId(cfg.id.clone()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: ChronoDuration::seconds(*interval_seconds as i64),
                    },
                    target,
                    instance_label: cfg.instance_label.clone(),
                    description: cfg.description.clone(),
                };

                let collector = BitcoinCoreRpcCollector::new(
                    descriptor,
                    connection,
                    http.clone(),
                    BitcoinCoreRpcCollectorConfig::default(),
                )
                .map_err(|source| BuildError::Inner {
                    id: cfg.id.clone(),
                    source,
                })?;

                out.push(Box::new(collector));
            }
            // Subscription kinds belong to `build_subscription_collectors`;
            // they're not an error here.
            IntegrationConfig::BitcoinCoreZmq | IntegrationConfig::LndGrpcStream => continue,
            IntegrationConfig::LndGrpcPoll { interval_seconds } => {
                let target = collector_target(&cfg.target);
                let node_id = match &target {
                    CollectorTarget::LndNode(id) => id,
                    _ => {
                        return Err(BuildError::WrongTargetKind {
                            id: cfg.id.clone(),
                            target_kind: target_kind_name(&target),
                            integration_kind: "lnd_grpc_poll",
                        });
                    }
                };
                let connection = registry.lnd_nodes.get(node_id).cloned().ok_or_else(|| {
                    BuildError::TargetNotFound {
                        id: cfg.id.clone(),
                        target: node_id.0.clone(),
                    }
                })?;

                let descriptor = CollectorDescriptor {
                    id: CollectorId(cfg.id.clone()),
                    integration: IntegrationKind::LndGrpcPoll {
                        interval: ChronoDuration::seconds(*interval_seconds as i64),
                    },
                    target,
                    instance_label: cfg.instance_label.clone(),
                    description: cfg.description.clone(),
                };

                let collector = LndGrpcPollingCollector::new(
                    descriptor,
                    connection,
                    LndGrpcPollingCollectorConfig::default(),
                )
                .map_err(|source| BuildError::InnerLnd {
                    id: cfg.id.clone(),
                    source,
                })?;

                out.push(Box::new(collector));
            }
            // Polling kinds deferred to V0.9+.
            IntegrationConfig::LndRest { .. } => {
                return Err(BuildError::NotImplemented("lnd_rest"));
            }
            IntegrationConfig::Host { .. } => {
                return Err(BuildError::NotImplemented("host"));
            }
        }
    }

    Ok(out)
}

/// Builds runtime subscription collector handles. V0 has no
/// subscription integration implemented yet — every subscription
/// entry returns `NotImplemented`.
pub fn build_subscription_collectors(
    configs: &[CollectorDescriptorConfig],
    _registry: &NodeRegistry,
    _http: &reqwest::Client,
) -> Result<Vec<Box<dyn SubscriptionCollector>>, BuildError> {
    for cfg in configs {
        match &cfg.integration {
            IntegrationConfig::BitcoinCoreZmq => {
                return Err(BuildError::NotImplemented("bitcoin_core_zmq"));
            }
            IntegrationConfig::LndGrpcStream => {
                return Err(BuildError::NotImplemented("lnd_grpc_stream"));
            }
            // Polling kinds — not our concern here.
            _ => {
                let _ = cfg;
                continue;
            }
        }
    }
    Ok(Vec::new())
}

/// Walks the parsed config and produces the runtime `NodeRegistry`.
/// Resolves every `*_env` secret reference into a concrete
/// `SecretString` via the supplied `ResolvedSecrets` map.
pub fn node_registry_from_config(
    config: &Config,
    secrets: &ResolvedSecrets,
) -> Result<NodeRegistry, BuildError> {
    let mut bitcoin_nodes: HashMap<BitcoinNodeId, BitcoinNodeConnection> = HashMap::new();

    for node in &config.bitcoin_nodes {
        let rpc_auth = match &node.auth {
            BitcoinAuthConfig::UserPass { user, password_env } => {
                let pass = secrets
                    .get(password_env)
                    .ok_or_else(|| BuildError::SecretNotResolved(password_env.clone()))?
                    .clone();
                BitcoinRpcAuth::UserPass {
                    user: user.clone(),
                    pass,
                }
            }
            BitcoinAuthConfig::CookieFile { path } => {
                BitcoinRpcAuth::CookieFile { path: path.clone() }
            }
        };

        bitcoin_nodes.insert(
            BitcoinNodeId(node.id.clone()),
            BitcoinNodeConnection {
                rpc_url: node.rpc_url.clone(),
                rpc_auth,
                zmq_endpoint: node.zmq_endpoint.clone(),
            },
        );
    }

    let mut lnd_nodes: HashMap<LndNodeId, LndNodeConnection> = HashMap::new();

    for node in &config.lnd_nodes {
        let macaroon = secrets
            .get(&node.macaroon_env)
            .ok_or_else(|| BuildError::SecretNotResolved(node.macaroon_env.clone()))?
            .clone();

        lnd_nodes.insert(
            LndNodeId(node.id.clone()),
            LndNodeConnection {
                grpc_endpoint: node.grpc_endpoint.clone(),
                rest_endpoint: node.rest_endpoint.clone(),
                macaroon,
                tls_cert_path: node.tls_cert_path.clone(),
            },
        );
    }

    // Host registry remains empty in V0.8; host collector lands in V0.9+.
    Ok(NodeRegistry {
        bitcoin_nodes,
        lnd_nodes,
        ..Default::default()
    })
}

fn collector_target(cfg: &CollectorTargetConfig) -> CollectorTarget {
    match cfg {
        CollectorTargetConfig::BitcoinNode { id } => {
            CollectorTarget::BitcoinNode(BitcoinNodeId(id.clone()))
        }
        CollectorTargetConfig::LndNode { id } => {
            CollectorTarget::LndNode(crate::shared::types::LndNodeId(id.clone()))
        }
        CollectorTargetConfig::Host { id } => {
            CollectorTarget::Host(crate::shared::types::HostId(id.clone()))
        }
    }
}

fn target_kind_name(target: &CollectorTarget) -> &'static str {
    match target {
        CollectorTarget::BitcoinNode(_) => "bitcoin_node",
        CollectorTarget::LndNode(_) => "lnd_node",
        CollectorTarget::Host(_) => "host",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    fn descriptor_config(
        id: &str,
        target_id: &str,
        integration: IntegrationConfig,
    ) -> CollectorDescriptorConfig {
        CollectorDescriptorConfig {
            id: id.into(),
            target: CollectorTargetConfig::BitcoinNode {
                id: target_id.into(),
            },
            integration,
            instance_label: id.into(),
            description: None,
        }
    }

    fn registry_with(node_id: &str) -> NodeRegistry {
        let mut nodes = HashMap::new();
        nodes.insert(
            BitcoinNodeId(node_id.into()),
            BitcoinNodeConnection {
                rpc_url: "http://127.0.0.1:8332".into(),
                rpc_auth: BitcoinRpcAuth::CookieFile {
                    path: "/dev/null".into(),
                },
                zmq_endpoint: None,
            },
        );
        NodeRegistry {
            bitcoin_nodes: nodes,
            ..Default::default()
        }
    }

    #[test]
    fn builds_bitcoin_core_rpc_collector_from_descriptor() {
        let cfg = descriptor_config(
            "alice-rpc",
            "alice",
            IntegrationConfig::BitcoinCoreRpc {
                interval_seconds: 10,
            },
        );
        let registry = registry_with("alice");
        let collectors = build_polling_collectors(&[cfg], &registry, &http_client())
            .expect("build should succeed");
        assert_eq!(collectors.len(), 1);
        assert_eq!(collectors[0].descriptor().id.0, "alice-rpc");
        assert!(matches!(
            collectors[0].descriptor().integration,
            IntegrationKind::BitcoinCoreRpc { .. }
        ));
    }

    #[test]
    fn unknown_target_returns_target_not_found() {
        let cfg = descriptor_config(
            "ghost-rpc",
            "nonexistent-node",
            IntegrationConfig::BitcoinCoreRpc {
                interval_seconds: 10,
            },
        );
        let registry = registry_with("alice");
        let err = build_polling_collectors(&[cfg], &registry, &http_client())
            .map(|_| ())
            .unwrap_err();
        match err {
            BuildError::TargetNotFound { id, target } => {
                assert_eq!(id, "ghost-rpc");
                assert_eq!(target, "nonexistent-node");
            }
            other => panic!("expected TargetNotFound, got {:?}", other),
        }
    }

    #[test]
    fn subscription_variants_return_not_implemented() {
        let cfg = descriptor_config("zmq", "alice", IntegrationConfig::BitcoinCoreZmq);
        let registry = registry_with("alice");
        let err = build_subscription_collectors(&[cfg], &registry, &http_client())
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(err, BuildError::NotImplemented("bitcoin_core_zmq")),
            "got {:?}",
            err
        );

        let cfg = descriptor_config("lnd-stream", "alice", IntegrationConfig::LndGrpcStream);
        let err = build_subscription_collectors(&[cfg], &registry, &http_client())
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(err, BuildError::NotImplemented("lnd_grpc_stream")),
            "got {:?}",
            err
        );
    }

    #[test]
    fn build_polling_skips_subscription_kinds() {
        let cfgs = vec![
            descriptor_config(
                "alice-rpc",
                "alice",
                IntegrationConfig::BitcoinCoreRpc {
                    interval_seconds: 5,
                },
            ),
            descriptor_config("alice-zmq", "alice", IntegrationConfig::BitcoinCoreZmq),
        ];
        let registry = registry_with("alice");
        let collectors = build_polling_collectors(&cfgs, &registry, &http_client())
            .expect("build should succeed");
        assert_eq!(
            collectors.len(),
            1,
            "subscription entry must be skipped by build_polling_collectors",
        );
    }

    #[test]
    fn build_subscription_skips_polling_kinds() {
        let cfgs = vec![descriptor_config(
            "alice-rpc",
            "alice",
            IntegrationConfig::BitcoinCoreRpc {
                interval_seconds: 5,
            },
        )];
        let registry = registry_with("alice");
        let subs = build_subscription_collectors(&cfgs, &registry, &http_client())
            .expect("build should succeed");
        assert!(subs.is_empty());
    }

    // ─── LND polling collector tests (v0.0.8.0+) ──────────────────────

    fn lnd_descriptor(
        id: &str,
        target_id: &str,
        interval_seconds: u32,
    ) -> CollectorDescriptorConfig {
        CollectorDescriptorConfig {
            id: id.into(),
            target: CollectorTargetConfig::LndNode {
                id: target_id.into(),
            },
            integration: IntegrationConfig::LndGrpcPoll { interval_seconds },
            instance_label: id.into(),
            description: None,
        }
    }

    fn registry_with_lnd(node_id: &str, tls_cert_path: &str) -> NodeRegistry {
        let mut lnd = HashMap::new();
        lnd.insert(
            LndNodeId(node_id.into()),
            LndNodeConnection {
                grpc_endpoint: "https://127.0.0.1:10009".into(),
                rest_endpoint: None,
                macaroon: secrecy::SecretString::from("00aabbcc"),
                tls_cert_path: tls_cert_path.into(),
            },
        );
        NodeRegistry {
            lnd_nodes: lnd,
            ..Default::default()
        }
    }

    /// Writes a placeholder TLS cert into a tempfile so the LND
    /// build path can read it. The bytes don't have to be a valid
    /// cert at construction time — tonic only parses on the first
    /// dial — but the file must exist and be readable.
    fn tempfile_with(bytes: &[u8]) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(bytes).expect("write tempfile");
        f
    }

    // NOTE: a happy-path "builds an LndGrpcPollingCollector" test would
    // need a real DER-encoded self-signed cert to satisfy
    // tonic 0.12's `ClientTlsConfig::ca_certificate` parsing.
    // Generating one inside a unit test is overkill for v0.8; the
    // BTH-67 Polar e2e harness exercises the success path with real
    // LND TLS material. These tests cover the build-error shapes
    // that don't reach TLS parsing.

    #[test]
    fn lnd_collector_with_bitcoin_target_is_wrong_target_kind() {
        let cert =
            tempfile_with(b"-----BEGIN CERTIFICATE-----\nplaceholder\n-----END CERTIFICATE-----\n");
        let registry = registry_with_lnd("polar-lnd", cert.path().to_str().unwrap());

        // Bitcoin target paired with the LND integration kind.
        let mut cfg = lnd_descriptor("oops", "polar-lnd", 5);
        cfg.target = CollectorTargetConfig::BitcoinNode {
            id: "polar-lnd".into(),
        };

        let err = build_polling_collectors(&[cfg], &registry, &http_client())
            .map(|_| ())
            .unwrap_err();
        match err {
            BuildError::WrongTargetKind {
                integration_kind, ..
            } => {
                assert_eq!(integration_kind, "lnd_grpc_poll");
            }
            other => panic!("expected WrongTargetKind, got {other:?}"),
        }
    }

    #[test]
    fn lnd_collector_with_unknown_target_returns_target_not_found() {
        let cert =
            tempfile_with(b"-----BEGIN CERTIFICATE-----\nplaceholder\n-----END CERTIFICATE-----\n");
        let registry = registry_with_lnd("polar-lnd", cert.path().to_str().unwrap());
        let cfg = lnd_descriptor("ghost-lnd-grpc", "nonexistent-lnd", 5);

        let err = build_polling_collectors(&[cfg], &registry, &http_client())
            .map(|_| ())
            .unwrap_err();
        match err {
            BuildError::TargetNotFound { id, target } => {
                assert_eq!(id, "ghost-lnd-grpc");
                assert_eq!(target, "nonexistent-lnd");
            }
            other => panic!("expected TargetNotFound, got {other:?}"),
        }
    }
}
