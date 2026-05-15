use super::{
    DiscordAllowedMentions, DiscordEmbed, DiscordEmbedField, DiscordNotificationConfig,
    DiscordPayload, DiscordSeverityPalette,
};
use crate::incidents::{IncidentLifecycleEvent, IncidentSeverity};
use crate::notifications::types::NotificationMessage;

pub fn render(message: &NotificationMessage, config: &DiscordNotificationConfig) -> DiscordPayload {
    let color = pick_color(&message.incident_lifecycle_event, &config.color_palette);

    DiscordPayload {
        content: None,
        username: config.default_username.clone(),
        avatar_url: config.default_avatar_url.clone(),
        embeds: vec![DiscordEmbed {
            title: Some(message.title.clone()),
            description: Some(message.summary.clone()),
            url: None,
            color: Some(color),
            timestamp: Some(message.occurred_at),
            footer: None,
            author: None,
            fields: build_fields(message),
        }],
        allowed_mentions: Some(DiscordAllowedMentions::none()),
    }
}

fn pick_color(event: &IncidentLifecycleEvent, palette: &DiscordSeverityPalette) -> u32 {
    if matches!(event, IncidentLifecycleEvent::Resolved(_)) {
        return palette.resolved;
    }
    match event.incident().severity {
        IncidentSeverity::Info => palette.info,
        IncidentSeverity::Warning => palette.warning,
        IncidentSeverity::Critical => palette.critical,
    }
}

fn build_fields(message: &NotificationMessage) -> Vec<DiscordEmbedField> {
    let mut fields = Vec::new();
    if let Some(c) = &message.affected_component {
        fields.push(DiscordEmbedField {
            name: "Affected".into(),
            value: c.clone(),
            inline: true,
        });
    }
    if let Some(d) = &message.diagnostic_summary {
        fields.push(DiscordEmbedField {
            name: "Diagnostic".into(),
            value: d.clone(),
            inline: false,
        });
    }
    fields
}
