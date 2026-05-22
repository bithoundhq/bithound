use serde::Deserialize;

/// `[notifications]` block. Per-sink defaults live here; per-rule
/// secrets (webhook URLs, discord webhooks) live on the rule.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub telegram: Option<TelegramSinkConfig>,
    #[serde(default)]
    pub discord: Option<DiscordSinkConfig>,
    #[serde(default)]
    pub webhook: Option<WebhookSinkConfig>,
}

/// `[notifications.telegram]` — one bot token serves every Telegram
/// rule (each rule supplies its own `chat_id`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelegramSinkConfig {
    pub bot_token_env: String,

    #[serde(default = "default_parse_mode")]
    pub parse_mode: TelegramParseModeConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelegramParseModeConfig {
    #[default]
    Plain,
    Html,
    MarkdownV2,
}

fn default_parse_mode() -> TelegramParseModeConfig {
    TelegramParseModeConfig::default()
}

/// `[notifications.discord]` — reserved for sink-wide defaults
/// (currently none). Empty struct keeps the table valid in TOML and
/// gives us a place to add fields later without re-shaping the
/// schema.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscordSinkConfig {}

/// `[notifications.webhook]` — same intent as Discord above.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookSinkConfig {}

/// `[[notification_rules]]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRuleConfig {
    /// UUIDv7 stable identifier. Operator-supplied so two sidecars
    /// running the same config produce comparable rule histories.
    pub id: uuid::Uuid,
    pub name: String,
    pub enabled: bool,
    pub min_severity: SeverityConfig,

    /// Empty vec ⇒ match every kind.
    #[serde(default)]
    pub event_kinds: Vec<String>,

    pub target: NotificationTargetConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeverityConfig {
    Info,
    Warning,
    Critical,
}

/// `[notification_rules.target]` inline tag-union. Discord and
/// webhook variants carry the secret reference (`*_env`) because the
/// URL itself is the credential; Telegram's `chat_id` isn't secret.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationTargetConfig {
    Telegram {
        chat_id: i64,
    },
    Discord {
        webhook_env: String,
        #[serde(default)]
        thread_id: Option<u64>,
    },
    Webhook {
        url_env: String,
    },
}
