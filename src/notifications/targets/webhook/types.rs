use secrecy::SecretString;

#[derive(Debug, Clone)]
pub struct WebhookTarget {
    pub url: SecretString,
    pub method: WebhookMethod,
    pub headers: Vec<WebhookHeader>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookMethod {
    Post,
}

#[derive(Debug, Clone)]
pub struct WebhookHeader {
    pub name: String,
    pub value: SecretString,
}
