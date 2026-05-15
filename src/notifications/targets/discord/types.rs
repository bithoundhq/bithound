use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::incidents::{IncidentNotificationEventKind, IncidentSeverity};

mod secret_url_serde {
    use secrecy::{ExposeSecret, SecretString};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(s: &SecretString, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(s.expose_secret())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<SecretString, D::Error> {
        Ok(SecretString::from(String::deserialize(de)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordGuildId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordChannelId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordMessageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordThreadId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordSubscriptionId(pub Uuid);

#[derive(Debug, Clone)]
pub struct DiscordTarget {
    pub webhook_url: SecretString,
    pub thread_id: Option<DiscordThreadId>,
    pub username_override: Option<String>,
    pub avatar_url_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordSubscription {
    pub id: DiscordSubscriptionId,
    #[serde(with = "secret_url_serde")]
    pub webhook_url: SecretString,
    pub channel_label: String,
    pub guild_id: Option<DiscordGuildId>,
    pub channel_id: Option<DiscordChannelId>,
    pub min_severity: IncidentSeverity,
    pub lifecycle_events: Vec<IncidentNotificationEventKind>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
