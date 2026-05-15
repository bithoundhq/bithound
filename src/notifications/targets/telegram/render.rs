use super::{TelegramNotificationConfig, TelegramPayload};
use crate::notifications::types::NotificationMessage;

pub fn render(message: &NotificationMessage, config: &TelegramNotificationConfig) -> TelegramPayload {
    TelegramPayload {
        text: format_body(message),
        parse_mode: config.parse_mode.clone(),
        disable_notification: None,
        reply_to_message_id: None,
        reply_markup: None,
    }
}

fn format_body(message: &NotificationMessage) -> String {
    let mut body = format!("{}\n\n{}", message.title, message.summary);
    if let Some(c) = &message.affected_component {
        body.push_str(&format!("\n\nAffected: {c}"));
    }
    if let Some(d) = &message.diagnostic_summary {
        body.push_str(&format!("\n\nDiagnostic: {d}"));
    }
    body
}
