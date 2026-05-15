use chrono::Utc;
use secrecy::SecretString;

use super::{TelegramPayload, TelegramTarget};
use crate::notifications::types::{DeliveryOutcome, DeliveryReceipt, PermanentError};

pub struct TelegramSender {
    #[allow(dead_code)]
    bot_token: SecretString,
}

impl TelegramSender {
    pub fn new(bot_token: SecretString) -> Self {
        Self { bot_token }
    }

    pub async fn send(
        &self,
        _target: &TelegramTarget,
        _payload: &TelegramPayload,
    ) -> DeliveryReceipt {
        let now = Utc::now();
        DeliveryReceipt {
            outcome: DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest {
                    detail: "TelegramSender::send is not yet implemented".into(),
                },
            },
            started_at: now,
            completed_at: now,
        }
    }
}
