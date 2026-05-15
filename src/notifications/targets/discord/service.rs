use super::{DiscordNotificationConfig, DiscordSender};

pub struct DiscordService {
    pub sender: DiscordSender,
    pub config: DiscordNotificationConfig,
}
