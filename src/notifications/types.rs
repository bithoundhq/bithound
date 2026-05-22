use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    incidents::{
        IncidentKind, IncidentLifecycleEvent, IncidentNotificationEventKind, IncidentSeverity,
    },
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

/// Persisted record of a notification dispatch attempt.
///
/// Per-row immutable: initial dispatch inserts the row with
/// `status = Pending`, then a single UPDATE moves it to a terminal status.
/// Retries don't mutate the row — they INSERT a new one with
/// `attempt_number + 1` and `parent_attempt_id` pointing back. In V0 the
/// retry path is unused (audit-only); `next_retry_at` is always `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAttempt {
    pub id: NotificationAttemptId,
    pub rule_id: NotificationRuleId,
    pub incident_id: IncidentId,
    pub lifecycle_kind: IncidentNotificationEventKind,

    pub target_kind: TargetKind,
    /// Human-readable, redacted target description.
    ///
    /// Full targets carry `SecretString`; those are never persisted. The
    /// summary uses sketches like `telegram:chat_id=-1001234` or
    /// `webhook:host=ops.example.com`.
    pub target_summary: String,

    pub status: NotificationDeliveryStatus,
    pub attempt_number: u32,
    pub parent_attempt_id: Option<NotificationAttemptId>,
    /// Only set when `status == FailedTransient` (V0.1+). V0 always `None`.
    pub next_retry_at: Option<DateTime<Utc>>,

    /// `None` while `status == Pending`; populated on terminal UPDATE.
    pub outcome: Option<DeliveryOutcome>,
    pub external_ref: Option<ExternalMessageRef>,

    pub attempted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Discriminant for the target a [`NotificationAttempt`] dispatched to.
///
/// Pulled out of the persisted form so that the secret material in
/// [`NotificationTarget`] never reaches storage.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetKind {
    Telegram,
    Discord,
    Webhook,
    #[cfg(debug_assertions)]
    Stdout,
}

impl TargetKind {
    /// Convert a live [`NotificationTarget`] into its persistable
    /// `(TargetKind, target_summary)` pair. None of these contain the
    /// underlying secret.
    pub fn summarize(target: &NotificationTarget) -> (Self, String) {
        match target {
            #[cfg(debug_assertions)]
            NotificationTarget::Stdout => (TargetKind::Stdout, "stdout".into()),
            NotificationTarget::Telegram(t) => (
                TargetKind::Telegram,
                format!("telegram:chat_id={}", t.chat_id.0),
            ),
            NotificationTarget::Discord(t) => (
                TargetKind::Discord,
                format!("discord:webhook=host={}", host_of_secret_url(&t.webhook_url)),
            ),
            NotificationTarget::Webhook(t) => (
                TargetKind::Webhook,
                format!("webhook:host={}", host_of_secret_url(&t.url)),
            ),
        }
    }
}

fn host_of_secret_url(s: &secrecy::SecretString) -> String {
    use secrecy::ExposeSecret;
    let raw = s.expose_secret();
    // No URL parser dep — split on '/' manually to extract host, never log the value.
    raw.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(|host_port| host_port.split('@').next_back().unwrap_or(host_port))
        .map(|host_port| host_port.split(':').next().unwrap_or(host_port))
        .unwrap_or("unknown")
        .to_string()
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
    /// Terminal-for-this-row; the row stays at this status while the V0.1
    /// retry scheduler creates a new row with `attempt_number + 1`. In V0
    /// (audit-only) this state is unused — a protocol-level transient that
    /// exhausts retries in V0 lands as `FailedPermanent` with
    /// `outcome_kind = 'Transient'`.
    FailedTransient,
    FailedPermanent,
    /// Dispatch dropped by a suppression rule. Recorded with
    /// `DeliveryOutcome::Suppressed` in `outcome_json` for audit.
    Suppressed,
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
