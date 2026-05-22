use chrono::{Duration as ChronoDuration, Utc};
use secrecy::ExposeSecret;
use serde::Deserialize;

use super::{DiscordChannelId, DiscordPayload, DiscordTarget};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, ExternalMessageRef, PermanentError, TransientError,
};

pub struct DiscordSender {
    http: reqwest::Client,
}

impl DiscordSender {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn send(&self, target: &DiscordTarget, payload: &DiscordPayload) -> DeliveryReceipt {
        let started_at = Utc::now();
        let outcome = self.do_send(target, payload).await;
        DeliveryReceipt {
            outcome,
            started_at,
            completed_at: Utc::now(),
        }
    }

    async fn do_send(&self, target: &DiscordTarget, payload: &DiscordPayload) -> DeliveryOutcome {
        // wait=true asks Discord to return the created message JSON
        // (with id + channel_id) instead of a 204 with no body, which
        // we need to populate ExternalMessageRef. thread_id targets a
        // specific thread when the operator configured one.
        let mut query: Vec<(&str, String)> = vec![("wait", "true".into())];
        if let Some(thread_id) = target.thread_id {
            query.push(("thread_id", thread_id.0.to_string()));
        }

        let response = match self
            .http
            .post(target.webhook_url.expose_secret())
            .query(&query)
            .json(payload)
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

        let status_code = response.status().as_u16();
        let body_bytes = response.bytes().await.unwrap_or_default();

        match status_code {
            200..=299 => {
                // The sender always passes ?wait=true so Discord must
                // return the created message JSON on success. A 2xx
                // with an unparseable body is suspicious (captive
                // portal, misconfigured proxy, partial CDN outage),
                // not a successful delivery — return Transient::Unknown
                // so the notifier retries instead of silently writing
                // Delivered to the audit trail.
                match serde_json::from_slice::<DiscordMessageResponse>(&body_bytes)
                    .ok()
                    .and_then(|m| {
                        let mid: u64 = m.id.parse().ok()?;
                        let cid: u64 = m.channel_id.parse().ok()?;
                        Some(ExternalMessageRef::Discord {
                            channel_id: DiscordChannelId(cid),
                            message_id: mid,
                        })
                    }) {
                    Some(external_ref) => DeliveryOutcome::Delivered {
                        external_ref: Some(external_ref),
                    },
                    None => DeliveryOutcome::Transient {
                        error: TransientError::Unknown {
                            detail: format!(
                                "HTTP {} but body is not a Discord message envelope (got {} bytes)",
                                status_code,
                                body_bytes.len()
                            ),
                        },
                        retry_after: None,
                    },
                }
            }
            401 | 403 => DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure,
            },
            404 | 410 => DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone,
            },
            429 => {
                // Discord publishes retry_after as a float (seconds) in
                // the JSON body, and also sets the Retry-After header.
                // Reject NaN/Inf/negative to defend against a hostile
                // upstream sending bogus values that would otherwise
                // overflow i64 or schedule a retry in the past.
                let retry_after = serde_json::from_slice::<DiscordRateLimitBody>(&body_bytes)
                    .ok()
                    .and_then(|b| sane_retry_after_seconds_f64(b.retry_after));
                DeliveryOutcome::Transient {
                    error: TransientError::RateLimited,
                    retry_after,
                }
            }
            400..=499 => {
                let detail = String::from_utf8_lossy(&body_bytes).to_string();
                DeliveryOutcome::Permanent {
                    error: PermanentError::BadRequest {
                        detail: format!("HTTP {}: {}", status_code, detail),
                    },
                }
            }
            500..=599 => DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx {
                    status: status_code,
                },
                retry_after: None,
            },
            other => DeliveryOutcome::Transient {
                error: TransientError::Unknown {
                    detail: format!("unexpected status {}", other),
                },
                retry_after: None,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct DiscordMessageResponse {
    id: String,
    channel_id: String,
}

#[derive(Debug, Deserialize)]
struct DiscordRateLimitBody {
    retry_after: f64,
}

/// Reject NaN, infinity, and negative values, then convert to a
/// finite chrono::Duration. Caps the upper bound at one hour because
/// any `retry_after` from Discord beyond that is a sign of a malformed
/// or hostile response, and longer scheduling is the notifier's
/// retry-backoff job.
fn sane_retry_after_seconds_f64(secs: f64) -> Option<ChronoDuration> {
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    const MAX_SECS: f64 = 3_600.0;
    let bounded = secs.min(MAX_SECS);
    let ms = (bounded * 1000.0).round() as i64;
    Some(ChronoDuration::milliseconds(ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::targets::discord::{
        DiscordAllowedMentions, DiscordEmbed, DiscordPayload, DiscordThreadId,
    };
    use secrecy::SecretString;
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

    fn target(addr: SocketAddr) -> DiscordTarget {
        DiscordTarget {
            webhook_url: SecretString::from(format!("http://{}/api/webhooks/100/secret", addr)),
            thread_id: None,
            username_override: None,
            avatar_url_override: None,
        }
    }

    fn payload() -> DiscordPayload {
        DiscordPayload {
            content: None,
            username: None,
            avatar_url: None,
            embeds: vec![DiscordEmbed {
                title: Some("Title".into()),
                description: Some("Body".into()),
                url: None,
                color: Some(0xff0000),
                timestamp: None,
                footer: None,
                author: None,
                fields: vec![],
            }],
            allowed_mentions: Some(DiscordAllowedMentions::none()),
        }
    }

    fn sender() -> DiscordSender {
        DiscordSender::new(reqwest::Client::new())
    }

    #[tokio::test]
    async fn success_returns_delivered_with_discord_external_ref() {
        let (addr, cap) = spawn_mock(|| {
            Reply::Json(
                200,
                json!({
                    "id": "1234567890123456789",
                    "channel_id": "9876543210987654321",
                    "type": 0
                }),
            )
        })
        .await;
        let r = sender().send(&target(addr), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Delivered {
                external_ref:
                    Some(ExternalMessageRef::Discord {
                        channel_id,
                        message_id,
                    }),
            } => {
                assert_eq!(channel_id.0, 9876543210987654321u64);
                assert_eq!(message_id, 1234567890123456789u64);
            }
            other => panic!("expected Delivered+Discord, got {:?}", other),
        }
        let cap = cap.lock().unwrap().clone();
        assert_eq!(cap.len(), 1);
        assert!(cap[0].path.contains("wait=true"));
        // allowed_mentions: none() defaults to empty parse/roles/users
        let body: serde_json::Value = serde_json::from_str(&cap[0].body).unwrap();
        assert_eq!(body["allowed_mentions"]["parse"], json!([]));
    }

    #[tokio::test]
    async fn thread_id_appended_to_url_when_set() {
        let (addr, cap) =
            spawn_mock(|| Reply::Json(200, json!({ "id": "1", "channel_id": "2" }))).await;
        let mut t = target(addr);
        t.thread_id = Some(DiscordThreadId(777));
        let _ = sender().send(&t, &payload()).await;
        let cap = cap.lock().unwrap().clone();
        assert!(cap[0].path.contains("wait=true"));
        assert!(cap[0].path.contains("thread_id=777"));
    }

    #[tokio::test]
    async fn http_429_returns_rate_limited_with_retry_after_from_body() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                429,
                json!({
                    "message": "You are being rate limited.",
                    "retry_after": 0.5,
                    "global": false
                }),
            )
        })
        .await;
        let r = sender().send(&target(addr), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after,
            } => {
                assert_eq!(retry_after, Some(ChronoDuration::milliseconds(500)));
            }
            other => panic!("expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn http_401_returns_auth_failure() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(401, "Unauthorized")).await;
        let r = sender().send(&target(addr), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure
            }
        ));
    }

    #[tokio::test]
    async fn http_404_returns_destination_gone() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(404, "Not Found")).await;
        let r = sender().send(&target(addr), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone
            }
        ));
    }

    #[tokio::test]
    async fn http_400_returns_bad_request_with_body_detail() {
        let (addr, _cap) = spawn_mock(|| {
            Reply::Json(
                400,
                json!({ "message": "Embed title is too long", "code": 50035 }),
            )
        })
        .await;
        let r = sender().send(&target(addr), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest { detail },
            } => {
                assert!(detail.contains("400"));
                assert!(detail.contains("Embed title is too long"));
            }
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn http_503_returns_transient_upstream() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(503, "")).await;
        let r = sender().send(&target(addr), &payload()).await;
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
        let r = sender().send(&target(addr), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Transient {
                error: TransientError::Network,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn allowed_mentions_default_to_none_in_render() {
        // The render layer emits DiscordAllowedMentions::none(), which
        // serializes to empty parse/roles/users arrays. This test asserts
        // the sender preserves whatever the render layer produced
        // (defense-in-depth against future regressions that might strip
        // the allowed_mentions field).
        let (addr, cap) =
            spawn_mock(|| Reply::Json(200, json!({ "id": "1", "channel_id": "2" }))).await;
        let _ = sender().send(&target(addr), &payload()).await;
        let cap = cap.lock().unwrap().clone();
        let body: serde_json::Value = serde_json::from_str(&cap[0].body).unwrap();
        assert!(body["allowed_mentions"].is_object());
        assert_eq!(body["allowed_mentions"]["parse"], json!([]));
        assert_eq!(body["allowed_mentions"]["roles"], json!([]));
        assert_eq!(body["allowed_mentions"]["users"], json!([]));
    }

    /// A 2xx response that isn't a parseable Discord message envelope
    /// (captive portal HTML, empty body, CDN error page) must NOT be
    /// reported as a successful delivery. The engine relies on
    /// Delivered meaning "the message reached Discord and we have the
    /// message_id"; downgrade to Transient::Unknown so the caller can
    /// retry instead of writing a misleading success to the audit log.
    #[tokio::test]
    async fn http_200_with_non_json_body_returns_transient_unknown() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(200, "<html>captive portal</html>")).await;
        let r = sender().send(&target(addr), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::Unknown { detail },
                ..
            } => {
                assert!(
                    detail.contains("HTTP 200"),
                    "detail should name the suspicious status: {}",
                    detail
                );
            }
            other => panic!(
                "expected Transient::Unknown for unparseable 200 body, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn retry_after_clamp_rejects_nan_inf_and_negative() {
        assert_eq!(sane_retry_after_seconds_f64(f64::NAN), None);
        assert_eq!(sane_retry_after_seconds_f64(f64::INFINITY), None);
        assert_eq!(sane_retry_after_seconds_f64(f64::NEG_INFINITY), None);
        assert_eq!(sane_retry_after_seconds_f64(-0.001), None);
        assert_eq!(
            sane_retry_after_seconds_f64(0.5),
            Some(ChronoDuration::milliseconds(500))
        );
        assert_eq!(
            sane_retry_after_seconds_f64(1.0),
            Some(ChronoDuration::seconds(1))
        );
        // Hostile upstream sends a year — clamped to one hour.
        let huge = sane_retry_after_seconds_f64(31_536_000.0).unwrap();
        assert_eq!(huge, ChronoDuration::seconds(3_600));
    }
}
