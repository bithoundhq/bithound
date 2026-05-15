use chrono::Utc;

use super::{DiscordPayload, DiscordTarget};
use crate::notifications::types::{DeliveryOutcome, DeliveryReceipt, PermanentError};

pub struct DiscordSender {
    #[allow(dead_code)]
    http: reqwest::Client,
}

impl DiscordSender {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn send(
        &self,
        _target: &DiscordTarget,
        _payload: &DiscordPayload,
    ) -> DeliveryReceipt {
        let now = Utc::now();
        DeliveryReceipt {
            outcome: DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest {
                    detail: "DiscordSender::send is not yet implemented".into(),
                },
            },
            started_at: now,
            completed_at: now,
        }
    }
}
