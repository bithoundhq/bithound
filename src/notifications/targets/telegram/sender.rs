use chrono::{Duration as ChronoDuration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::{TelegramChatId, TelegramParseMode, TelegramPayload, TelegramTarget};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, ExternalMessageRef, PermanentError, TransientError,
};

pub struct TelegramSender {
    bot_token: SecretString,
    http: reqwest::Client,
    base_url: String,
}

impl TelegramSender {
    pub fn new(bot_token: SecretString, http: reqwest::Client) -> Self {
        Self {
            bot_token,
            http,
            base_url: "https://api.telegram.org".to_string(),
        }
    }

    /// Override the API base URL. Used by tests to point the sender
    /// at a local mock server. The runtime keeps the default.
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub async fn send(
        &self,
        target: &TelegramTarget,
        payload: &TelegramPayload,
    ) -> DeliveryReceipt {
        let started_at = Utc::now();
        let outcome = self.do_send(target, payload).await;
        DeliveryReceipt {
            outcome,
            started_at,
            completed_at: Utc::now(),
        }
    }

    async fn do_send(&self, target: &TelegramTarget, payload: &TelegramPayload) -> DeliveryOutcome {
        let url = format!(
            "{}/bot{}/sendMessage",
            self.base_url,
            self.bot_token.expose_secret()
        );

        let mut body = serde_json::json!({
            "chat_id": target.chat_id.0,
            "text": payload.text,
        });
        if matches!(payload.parse_mode, TelegramParseMode::Html) {
            body["parse_mode"] = serde_json::Value::String("HTML".into());
        }
        if let Some(silent) = payload.disable_notification {
            body["disable_notification"] = serde_json::Value::Bool(silent);
        }
        if let Some(reply) = payload.reply_to_message_id {
            body["reply_to_message_id"] = serde_json::Value::Number(reply.into());
        }

        let response = match self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => {
                return DeliveryOutcome::Transient {
                    error: TransientError::Network,
                    retry_after: None,
                };
            }
        };

        let http_status = response.status().as_u16();
        let body_bytes = match response.bytes().await {
            Ok(b) => b,
            Err(_) => {
                return DeliveryOutcome::Transient {
                    error: TransientError::Network,
                    retry_after: None,
                };
            }
        };

        // 5xx at the HTTP layer is upstream regardless of body shape.
        if (500..=599).contains(&http_status) {
            return DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx {
                    status: http_status,
                },
                retry_after: None,
            };
        }

        // Telegram always returns a JSON envelope (even on logical errors).
        let envelope: TelegramEnvelope = match serde_json::from_slice(&body_bytes) {
            Ok(e) => e,
            Err(_) => {
                return DeliveryOutcome::Transient {
                    error: TransientError::Unknown {
                        detail: format!("non-JSON response, HTTP {}", http_status),
                    },
                    retry_after: None,
                };
            }
        };

        if envelope.ok {
            let message_id = envelope.result.as_ref().and_then(|r| r.message_id);
            let chat_id_returned = envelope
                .result
                .as_ref()
                .and_then(|r| r.chat.as_ref())
                .and_then(|c| c.id);
            let external_ref = message_id.map(|mid| ExternalMessageRef::Telegram {
                chat_id: TelegramChatId(chat_id_returned.unwrap_or(target.chat_id.0)),
                message_id: mid,
            });
            return DeliveryOutcome::Delivered { external_ref };
        }

        let detail = envelope.description.clone().unwrap_or_default();
        let retry_after = envelope
            .parameters
            .as_ref()
            .and_then(|p| p.retry_after)
            .map(ChronoDuration::seconds);
        match envelope.error_code {
            Some(429) => DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after,
            },
            Some(401) => DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure,
            },
            Some(403) => DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone,
            },
            Some(code) if (400..500).contains(&code) => DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest {
                    detail: format!("[{}] {}", code, detail),
                },
            },
            Some(code) if (500..600).contains(&code) => DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx {
                    status: code as u16,
                },
                retry_after: None,
            },
            _ => DeliveryOutcome::Transient {
                error: TransientError::Unknown { detail },
                retry_after,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct TelegramEnvelope {
    ok: bool,
    #[serde(default)]
    result: Option<TelegramResult>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    error_code: Option<i32>,
    #[serde(default)]
    parameters: Option<TelegramErrorParameters>,
}

#[derive(Debug, Deserialize)]
struct TelegramResult {
    #[serde(default)]
    message_id: Option<i64>,
    #[serde(default)]
    chat: Option<TelegramChat>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    #[serde(default)]
    id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TelegramErrorParameters {
    #[serde(default)]
    retry_after: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[derive(Debug, Clone, Default)]
    struct Captured {
        path: String,
        body: String,
    }

    enum Reply {
        Json(u16, serde_json::Value),
        Status(u16, &'static str),
        Network,
    }

    async fn spawn_mock<F>(handler: F) -> (SocketAddr, Arc<Mutex<Vec<Captured>>>)
    where
        F: Fn() -> Reply + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured: Arc<Mutex<Vec<Captured>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured.clone();
        let handler = Arc::new(handler);
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(p) => p,
                    Err(_) => return,
                };
                let captured = captured_clone.clone();
                let handler = handler.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let mut cap = Captured::default();
                    if let Some(request_line) = req.split("\r\n").next() {
                        if let Some(path) = request_line.split_whitespace().nth(1) {
                            cap.path = path.to_string();
                        }
                    }
                    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                    cap.body = req[body_start..].to_string();
                    captured.lock().unwrap().push(cap);

                    match handler() {
                        Reply::Json(code, value) => {
                            let body = serde_json::to_vec(&value).unwrap();
                            let resp = format!(
                                "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                code,
                                body.len()
                            );
                            socket.write_all(resp.as_bytes()).await.ok();
                            socket.write_all(&body).await.ok();
                        }
                        Reply::Status(code, body) => {
                            let resp = format!(
                                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                code,
                                body.len()
                            );
                            socket.write_all(resp.as_bytes()).await.ok();
                            socket.write_all(body.as_bytes()).await.ok();
                        }
                        Reply::Network => {}
                    }
                });
            }
        });
        (addr, captured)
    }

    fn sender(addr: SocketAddr) -> TelegramSender {
        TelegramSender::new(
            SecretString::from("test-token".to_string()),
            reqwest::Client::new(),
        )
        .with_base_url(format!("http://{}", addr))
    }

    fn target() -> TelegramTarget {
        TelegramTarget {
            chat_id: TelegramChatId(-1001234567890),
            parse_mode: TelegramParseMode::Html,
        }
    }

    fn payload() -> TelegramPayload {
        TelegramPayload {
            text: "Hello world".into(),
            parse_mode: TelegramParseMode::Html,
            disable_notification: None,
            reply_to_message_id: None,
            reply_markup: None,
        }
    }

    #[tokio::test]
    async fn success_returns_delivered_with_telegram_external_ref() {
        let (addr, cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({
                    "ok": true,
                    "result": {
                        "message_id": 4242,
                        "chat": { "id": -1001234567890i64, "type": "supergroup" },
                        "text": "Hello world"
                    }
                }),
            )
        })
        .await;
        let r = sender(addr).send(&target(), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Delivered {
                external_ref:
                    Some(ExternalMessageRef::Telegram {
                        chat_id,
                        message_id,
                    }),
            } => {
                assert_eq!(chat_id.0, -1001234567890);
                assert_eq!(message_id, 4242);
            }
            other => panic!("expected Delivered+Telegram, got {:?}", other),
        }
        let cap = cap.lock().unwrap().clone();
        assert_eq!(cap.len(), 1);
        // URL embeds the bot token after `/bot`.
        assert!(cap[0].path.starts_with("/bottest-token/sendMessage"));
        let parsed: serde_json::Value = serde_json::from_str(&cap[0].body).unwrap();
        assert_eq!(parsed["chat_id"], json!(-1001234567890i64));
        assert_eq!(parsed["text"], json!("Hello world"));
        assert_eq!(parsed["parse_mode"], json!("HTML"));
    }

    #[tokio::test]
    async fn plain_text_parse_mode_omits_field() {
        let (addr, cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({
                    "ok": true,
                    "result": { "message_id": 1, "chat": { "id": 1 } }
                }),
            )
        })
        .await;
        let mut p = payload();
        p.parse_mode = TelegramParseMode::PlainText;
        let _ = sender(addr).send(&target(), &p).await;
        let cap = cap.lock().unwrap().clone();
        let parsed: serde_json::Value = serde_json::from_str(&cap[0].body).unwrap();
        assert!(parsed.get("parse_mode").is_none());
    }

    #[tokio::test]
    async fn rate_limited_surfaces_retry_after() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({
                    "ok": false,
                    "error_code": 429,
                    "description": "Too Many Requests: retry after 7",
                    "parameters": { "retry_after": 7 }
                }),
            )
        })
        .await;
        let r = sender(addr).send(&target(), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after,
            } => assert_eq!(retry_after, Some(ChronoDuration::seconds(7))),
            other => panic!("expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn error_code_401_returns_auth_failure() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({ "ok": false, "error_code": 401, "description": "Unauthorized" }),
            )
        })
        .await;
        let r = sender(addr).send(&target(), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure
            }
        ));
    }

    #[tokio::test]
    async fn error_code_403_returns_destination_gone() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({ "ok": false, "error_code": 403, "description": "bot was blocked by the user" }),
            )
        })
        .await;
        let r = sender(addr).send(&target(), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone
            }
        ));
    }

    #[tokio::test]
    async fn other_4xx_returns_bad_request_with_detail() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({ "ok": false, "error_code": 400, "description": "Bad Request: chat not found" }),
            )
        })
        .await;
        let r = sender(addr).send(&target(), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest { detail },
            } => {
                assert!(detail.contains("400"));
                assert!(detail.contains("chat not found"));
            }
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn http_5xx_returns_transient_upstream() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(503, "")).await;
        let r = sender(addr).send(&target(), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx { status },
                ..
            } => assert_eq!(status, 503),
            other => panic!("expected Upstream5xx, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn network_drop_returns_transient_network() {
        let (addr, _cap) = spawn_mock(|| Reply::Network).await;
        let r = sender(addr).send(&target(), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Transient {
                error: TransientError::Network,
                ..
            }
        ));
    }
}
