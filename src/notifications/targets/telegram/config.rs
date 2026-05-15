use secrecy::SecretString;

use crate::{
    incidents::{IncidentNotificationEventKind, IncidentSeverity},
    notifications::targets::telegram::TelegramParseMode,
};

#[derive(Debug, Clone)]
pub enum TelegramSetup {
    Disabled,
    Enabled(TelegramNotificationConfig),
}

#[derive(Debug, Clone)]
pub struct TelegramNotificationConfig {
    pub bot_token: SecretString,
    pub pairing_enabled: bool,
    pub parse_mode: TelegramParseMode,
    pub min_severity: IncidentSeverity,
    pub lifecycle_events: Vec<IncidentNotificationEventKind>,
}
