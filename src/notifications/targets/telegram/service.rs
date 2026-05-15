use super::{TelegramNotificationConfig, TelegramSender};

pub struct TelegramService {
    pub sender: TelegramSender,
    pub config: TelegramNotificationConfig,
}
