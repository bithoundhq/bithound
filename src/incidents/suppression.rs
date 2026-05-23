//! Suppression command vocabulary and service trait.
//!
//! V0/V0.1 do not ship a concrete `SuppressionService`; the types and
//! trait stub exist so V0.2 can wire suppression in without breaking
//! the command vocabulary downstream consumers depend on (per ADR-D3
//! and ADR-L5). The engine continues to surface every active draft
//! as immediate-open in V0; suppression in V0.1 is notifier-side via
//! [`crate::notifications`]-layer `SuppressionRule`s.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::incidents::repository::RepoError;
use crate::incidents::IncidentFingerprint;
use crate::shared::types::ActorId;

/// A request to mutate suppression state for an incident fingerprint.
///
/// Per ADR-D3, suppression commands are intentionally a *separate*
/// vocabulary from [`crate::incidents::engine::IncidentCommand`];
/// suppression lives off the engine's hot path and is handled by a
/// distinct [`SuppressionService`] when one is wired up in V0.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuppressionCommand {
    /// Add or refresh a suppression for `fingerprint`. `until = None`
    /// is "indefinite"; the operator can later issue [`Unsuppress`].
    Suppress {
        fingerprint: IncidentFingerprint,
        until: Option<DateTime<Utc>>,
        by: ActorId,
        reason: String,
    },
    /// Clear an existing suppression. A no-op if none was active.
    Unsuppress {
        fingerprint: IncidentFingerprint,
        by: ActorId,
    },
}

/// Service that applies [`SuppressionCommand`]s to durable state.
///
/// V0/V0.1 ship no concrete impl; the trait exists so V0.2 (and any
/// CLI/admin surface that lands earlier) can target a stable shape.
#[async_trait]
pub trait SuppressionService: Send + Sync {
    async fn handle(
        &self,
        cmd: SuppressionCommand,
        now: DateTime<Utc>,
    ) -> Result<(), SuppressionError>;
}

#[derive(Debug, Error)]
pub enum SuppressionError {
    /// Returned by every V0 placeholder impl until V0.2 wires the
    /// real service. The static `&str` describes what was requested
    /// so a UI can show "suppression isn't supported yet" without
    /// inspecting the variant.
    #[error("suppression not yet implemented: {0}")]
    NotYetImplemented(&'static str),

    /// Wraps repository errors when the eventual impl persists state.
    #[error("repository: {0}")]
    Repository(#[from] RepoError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::IncidentKind;
    use crate::shared::types::{BitcoinNodeId, EntityRef};

    fn fp() -> IncidentFingerprint {
        IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            kind: IncidentKind::parse("bitcoin.tip_lag").expect("valid"),
            dimension: None,
        }
    }

    #[test]
    fn suppression_command_round_trips_via_serde() {
        let cmd = SuppressionCommand::Suppress {
            fingerprint: fp(),
            until: Some(Utc::now()),
            by: ActorId::operator("h4vismat"),
            reason: "planned maintenance".into(),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let _: SuppressionCommand = serde_json::from_str(&json).expect("deserialize");
    }

    #[test]
    fn unsuppress_round_trips_via_serde() {
        let cmd = SuppressionCommand::Unsuppress {
            fingerprint: fp(),
            by: ActorId::system(),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let _: SuppressionCommand = serde_json::from_str(&json).expect("deserialize");
    }

    #[test]
    fn not_yet_implemented_renders_message() {
        let err = SuppressionError::NotYetImplemented("Suppress");
        assert!(err.to_string().contains("Suppress"));
    }
}
