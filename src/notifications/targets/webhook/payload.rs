use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WebhookPayload {
    pub body: Value,
}
