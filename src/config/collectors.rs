use serde::Deserialize;

/// `[[collectors]]` array-of-tables.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectorDescriptorConfig {
    /// `CollectorId` — opaque string, unique per sidecar.
    pub id: String,

    pub target: CollectorTargetConfig,
    pub integration: IntegrationConfig,

    /// Operator-visible label used in incident summaries and logs.
    pub instance_label: String,

    #[serde(default)]
    pub description: Option<String>,
}

/// `[collectors.target]` inline table.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectorTargetConfig {
    BitcoinNode { id: String },
    LndNode { id: String },
    Host { id: String },
}

impl CollectorTargetConfig {
    pub fn target_id(&self) -> &str {
        match self {
            Self::BitcoinNode { id } | Self::LndNode { id } | Self::Host { id } => id,
        }
    }
}

/// `[collectors.integration]` inline table. Tag mirrors the variants
/// of `IntegrationKind` in the runtime model so building one from
/// the other is mechanical.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntegrationConfig {
    BitcoinCoreRpc { interval_seconds: u32 },
    BitcoinCoreZmq,
    LndGrpcPoll { interval_seconds: u32 },
    LndGrpcStream,
    LndRest { interval_seconds: u32 },
    Host { interval_seconds: u32 },
}
