use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::incidents::{IncidentNotificationEventKind, IncidentSeverity};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramChatId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramUserId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramSubscriptionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramPairingChallengeId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramSubscription {
    pub id: TelegramSubscriptionId,
    pub challenge_id: TelegramPairingChallengeId,
    pub chat_id: TelegramChatId,
    pub chat_kind: TelegramChatKind,
    pub chat_title: Option<String>,
    pub user_id: Option<TelegramUserId>,
    pub username: Option<String>,
    pub min_severity: IncidentSeverity,
    pub lifecycle_events: Vec<IncidentNotificationEventKind>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TelegramTarget {
    pub chat_id: TelegramChatId,
    pub parse_mode: TelegramParseMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramPairingChallenge {
    pub id: TelegramPairingChallengeId,
    pub code_hash: PairingCodeHash,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum TelegramChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramParseMode {
    PlainText,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramCommand {
    Start { code: String },
    TestAlert,
    Status,
    Help,
    Unpair,
}

#[derive(Debug, Clone)]
pub struct PairingCode(String);

impl PairingCode {
    pub fn normalize(raw: &str) -> Self {
        let code: String = raw
            .trim()
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .flat_map(|c| c.to_uppercase())
            .collect();

        Self(code)
    }

    pub fn generate() -> Self {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

        let mut rng = rand::rng();
        let code: String = (0..8)
            .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
            .collect();

        Self(code)
    }

    pub fn formatted(&self) -> String {
        if self.0.len() == 8 {
            format!("{}-{}", &self.0[..4], &self.0[4..])
        } else {
            self.0.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCodeHash(pub String);

impl PairingCodeHash {
    pub fn from_code(secret: &[u8], code: &PairingCode) -> Self {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any size.");
        mac.update(code.0.as_bytes());

        let result = mac.finalize().into_bytes();
        Self(hex::encode(result))
    }
}

impl PartialEq for PairingCodeHash {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl Eq for PairingCodeHash {}
