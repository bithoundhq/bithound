use chrono::Utc;
use futures::future::join_all;

use crate::incidents::IncidentLifecycleEvent;
use crate::notifications::targets::discord::{self, DiscordService};
use crate::notifications::targets::telegram::{self, TelegramService};
use crate::notifications::targets::webhook::{self, WebhookSender};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, NotificationMessage, NotificationRule, NotificationRuleId,
    NotificationTarget, PermanentError,
};

pub struct Notifier {
    rules: Vec<NotificationRule>,
    telegram: Option<TelegramService>,
    discord: Option<DiscordService>,
    webhook: WebhookSender,
}

impl Notifier {
    pub fn new(
        rules: Vec<NotificationRule>,
        webhook: WebhookSender,
        telegram: Option<TelegramService>,
        discord: Option<DiscordService>,
    ) -> Self {
        Self {
            rules,
            telegram,
            discord,
            webhook,
        }
    }

    pub async fn dispatch(
        &self,
        event: &IncidentLifecycleEvent,
        message: &NotificationMessage,
    ) -> Vec<(NotificationRuleId, DeliveryReceipt)> {
        let matching: Vec<&NotificationRule> =
            self.rules.iter().filter(|r| r.matches(event)).collect();

        let futures = matching.into_iter().map(|rule| async move {
            let receipt = self.deliver(&rule.target, message).await;
            (rule.id.clone(), receipt)
        });

        join_all(futures).await
    }

    async fn deliver(
        &self,
        target: &NotificationTarget,
        message: &NotificationMessage,
    ) -> DeliveryReceipt {
        let started_at = Utc::now();
        match target {
            #[cfg(debug_assertions)]
            NotificationTarget::Stdout => {
                println!("[notification] {}\n{}", message.title, message.summary);
                DeliveryReceipt {
                    outcome: DeliveryOutcome::Delivered { external_ref: None },
                    started_at,
                    completed_at: Utc::now(),
                }
            }
            NotificationTarget::Discord(t) => match &self.discord {
                Some(svc) => {
                    let payload = discord::render::render(message, &svc.config);
                    svc.sender.send(t, &payload).await
                }
                None => not_configured(started_at),
            },
            NotificationTarget::Telegram(t) => match &self.telegram {
                Some(svc) => {
                    let payload = telegram::render::render(message, &svc.config);
                    svc.sender.send(t, &payload).await
                }
                None => not_configured(started_at),
            },
            NotificationTarget::Webhook(t) => {
                let payload = webhook::render::render(message);
                self.webhook.send(t, &payload).await
            }
        }
    }
}

fn not_configured(started_at: chrono::DateTime<Utc>) -> DeliveryReceipt {
    DeliveryReceipt {
        outcome: DeliveryOutcome::Permanent {
            error: PermanentError::NotConfigured,
        },
        started_at,
        completed_at: Utc::now(),
    }
}
