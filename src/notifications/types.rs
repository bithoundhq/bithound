use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    incidents::{IncidentKind, IncidentLifecycleEvent, IncidentSeverity},
    notifications::targets::{
        discord::{DiscordChannelId, DiscordTarget},
        telegram::{TelegramChatId, TelegramTarget},
        webhook::WebhookTarget,
    },
    shared::types::IncidentId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationId(pub Uuid);

/// Operator-supplied stable slug for a notification rule. Follows
/// the same convention as `CollectorId` and `BitcoinNodeId`: the
/// operator picks a short, human-readable string that stays the
/// same across config reloads and shows up directly in audit logs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationRuleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationAttemptId(pub Uuid);

impl NotificationAttemptId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationTargetId(pub Uuid);

impl NotificationTargetId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRuleName(pub String);

#[derive(Debug, Clone)]
pub struct NotificationRule {
    pub id: NotificationRuleId,
    pub name: NotificationRuleName,
    pub enabled: bool,
    pub min_severity: IncidentSeverity,
    pub event_kinds: Vec<IncidentKind>,
    pub target: NotificationTarget,
}

#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub incident_lifecycle_event: IncidentLifecycleEvent,
    pub title: String,
    pub summary: String,
    pub affected_component: Option<String>,
    pub diagnostic_summary: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NotificationAttempt {
    pub id: NotificationAttemptId,
    pub rule_id: NotificationRuleId,
    pub incident_lifecycle_event: IncidentLifecycleEvent,
    pub target: NotificationTarget,
    pub status: NotificationDeliveryStatus,
    pub attempted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeliveryReceipt {
    pub outcome: DeliveryOutcome,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSource {
    Incident(IncidentId),
    System,
    #[cfg(debug_assertions)]
    ManualTest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    IncidentOpened,
    IncidentEscalated,
    IncidentResolved,
    System,
    #[cfg(debug_assertions)]
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationDeliveryStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone)]
pub enum NotificationTarget {
    #[cfg(debug_assertions)]
    Stdout,
    Discord(DiscordTarget),
    Telegram(TelegramTarget),
    Webhook(WebhookTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryOutcome {
    Delivered {
        external_ref: Option<ExternalMessageRef>, // None for webhooks; Some for telegram/discord
    },
    Transient {
        error: TransientError,
        retry_after: Option<Duration>, // telegram surfaces this, others won't
    },
    Permanent {
        error: PermanentError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransientError {
    RateLimited,
    Network,
    Upstream5xx { status: u16 },
    Unknown { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentError {
    AuthFailure,                   // bad bot token, expired creds
    DestinationGone,               // user blocked bot, chat deleted, webhook 410
    BadRequest { detail: String }, // we sent something the API rejected
    NotConfigured,                 // rule points at a target whose sender wasn't initialized
}

impl NotificationRule {
    pub fn matches(&self, event: &IncidentLifecycleEvent) -> bool {
        if !self.enabled {
            return false;
        }
        let incident = event.incident();
        if !self.event_kinds.is_empty() && !self.event_kinds.contains(&incident.kind) {
            return false;
        }
        severity_at_least(&incident.severity, &self.min_severity)
    }
}

fn severity_at_least(actual: &IncidentSeverity, floor: &IncidentSeverity) -> bool {
    severity_rank(actual) >= severity_rank(floor)
}

fn severity_rank(s: &IncidentSeverity) -> u8 {
    match s {
        IncidentSeverity::Info => 0,
        IncidentSeverity::Warning => 1,
        IncidentSeverity::Critical => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalMessageRef {
    Telegram {
        chat_id: TelegramChatId,
        message_id: i64,
    },
    Discord {
        channel_id: DiscordChannelId,
        message_id: u64,
    },
}
