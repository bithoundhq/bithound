use super::TelegramParseMode;

#[derive(Debug, Clone)]
pub struct TelegramPayload {
    pub text: String,
    pub parse_mode: TelegramParseMode,
    pub disable_notification: Option<bool>,
    pub reply_to_message_id: Option<i32>,
    pub reply_markup: Option<TelegramReplyMarkup>,
}

#[derive(Debug, Clone)]
pub struct TelegramReplyMarkup {
    pub inline_keyboard: Vec<Vec<TelegramInlineButton>>,
}

#[derive(Debug, Clone)]
pub struct TelegramInlineButton {
    pub text: String,
    pub callback_data: String,
}
