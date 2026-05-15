use crate::incidents::{IncidentNotificationEventKind, IncidentSeverity};

#[derive(Debug, Clone)]
pub struct DiscordNotificationConfig {
    pub enabled: bool,
    pub min_severity: IncidentSeverity,
    pub lifecycle_events: Vec<IncidentNotificationEventKind>,
    pub default_username: Option<String>,
    pub default_avatar_url: Option<String>,
    pub color_palette: DiscordSeverityPalette,
}

#[derive(Debug, Clone)]
pub struct DiscordSeverityPalette {
    pub info: u32,
    pub warning: u32,
    pub critical: u32,
    pub resolved: u32,
}

impl Default for DiscordSeverityPalette {
    fn default() -> Self {
        Self {
            info: 0x3498DB,
            warning: 0xF39C12,
            critical: 0xE74C3C,
            resolved: 0x2ECC71,
        }
    }
}
