use super::WebhookPayload;
use crate::notifications::types::NotificationMessage;

pub fn render(message: &NotificationMessage) -> WebhookPayload {
    let event_kind = message.incident_lifecycle_event.notification_kind();
    let incident = message.incident_lifecycle_event.incident();

    WebhookPayload {
        body: serde_json::json!({
            "event": event_kind,
            "incident_id": incident.id,
            "kind": incident.kind,
            "severity": incident.severity,
            "title": message.title,
            "summary": message.summary,
            "affected_component": message.affected_component,
            "diagnostic_summary": message.diagnostic_summary,
            "occurred_at": message.occurred_at,
        }),
    }
}
