use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::types::{BitcoinNodeId, CollectorId, HostId, LndNodeId, SidecarId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectionRunId(pub Uuid);

#[derive(Debug, Clone)]
pub struct CollectionContext {
    pub sidecar_id: SidecarId,
    pub collector_id: CollectorId,
    pub target: CollectorTarget,
    pub now: DateTime<Utc>,
    pub run_id: CollectionRunId,
}

#[derive(Debug, Clone)]
pub enum CollectorSetup {
    Disabled,
    Enabled(CollectorDescriptor),
}

#[derive(Debug, Clone)]
pub struct CollectorDescriptor {
    pub id: CollectorId,
    pub integration: IntegrationKind,
    pub target: CollectorTarget,
    pub instance_label: String,
    pub description: Option<String>,
}

impl CollectorDescriptor {
    pub fn as_ref(&self) -> CollectorRef {
        CollectorRef {
            id: self.id.clone(),
            integration: self.integration.clone(),
            instance_label: self.instance_label.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectorRef {
    pub id: CollectorId,
    pub integration: IntegrationKind,
    pub instance_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectorMode {
    Polling,
    Subscription,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntegrationKind {
    BitcoinCoreRpc { interval: Duration },
    BitcoinCoreZmq,
    LndGrpcPoll { interval: Duration },
    LndGrpcStream,
    LndRest { interval: Duration },
    Host { interval: Duration },
}

impl IntegrationKind {
    pub fn mode(&self) -> CollectorMode {
        match self {
            Self::BitcoinCoreRpc { .. }
            | Self::LndGrpcPoll { .. }
            | Self::LndRest { .. }
            | Self::Host { .. } => CollectorMode::Polling,
            Self::BitcoinCoreZmq | Self::LndGrpcStream => CollectorMode::Subscription,
        }
    }

    pub fn interval(&self) -> Option<Duration> {
        match self {
            Self::BitcoinCoreRpc { interval }
            | Self::LndGrpcPoll { interval }
            | Self::LndRest { interval }
            | Self::Host { interval } => Some(*interval),
            Self::BitcoinCoreZmq | Self::LndGrpcStream => None,
        }
    }
}

/// Identifier-only target. Connection details are resolved through `NodeRegistry`
/// at construction time, not embedded here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollectorTarget {
    BitcoinNode(BitcoinNodeId),
    LndNode(LndNodeId),
    Host(HostId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionError {
    pub kind: CollectionErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionErrorKind {
    Unreachable,
    Timeout,
    AuthenticationFailed,
    PermissionDenied,
    ProtocolError,
    DecodeError,
    InvalidResponse,
    RateLimited,
    Misconfigured,
    UnsupportedVersion,
    Internal,
}

impl CollectionErrorKind {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Unreachable | Self::Timeout | Self::RateLimited | Self::Internal
        )
    }
}
