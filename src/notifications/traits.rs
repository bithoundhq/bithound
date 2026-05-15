use anyhow::Result;
use async_trait::async_trait;

use crate::notifications::types::{
    DeliveryReceipt, NotificationKind, NotificationMessage, NotificationTarget,
};

#[async_trait]
pub trait ErasedSink: Send + Sync {
    fn notification_kind(&self) -> NotificationKind;
    async fn deliver(
        &self,
        target: &NotificationTarget,
        message: &NotificationMessage,
    ) -> Result<DeliveryReceipt>;
}
