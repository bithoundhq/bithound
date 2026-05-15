use chrono::Utc;

use super::{WebhookPayload, WebhookTarget};
use crate::notifications::types::{DeliveryOutcome, DeliveryReceipt, PermanentError};

pub struct WebhookSender {
    #[allow(dead_code)]
    http: reqwest::Client,
}

impl WebhookSender {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn send(
        &self,
        _target: &WebhookTarget,
        _payload: &WebhookPayload,
    ) -> DeliveryReceipt {
        let now = Utc::now();
        DeliveryReceipt {
            outcome: DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest {
                    detail: "WebhookSender::send is not yet implemented".into(),
                },
            },
            started_at: now,
            completed_at: now,
        }
    }
}
