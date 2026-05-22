use chrono::{Duration as ChronoDuration, Utc};
use reqwest::header::{HeaderName, HeaderValue};
use secrecy::ExposeSecret;

use super::{WebhookPayload, WebhookTarget};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, PermanentError, TransientError,
};

pub struct WebhookSender {
    http: reqwest::Client,
}

impl WebhookSender {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn send(&self, target: &WebhookTarget, payload: &WebhookPayload) -> DeliveryReceipt {
        let started_at = Utc::now();
        let outcome = self.do_send(target, payload).await;
        DeliveryReceipt {
            outcome,
            started_at,
            completed_at: Utc::now(),
        }
    }

    async fn do_send(&self, target: &WebhookTarget, payload: &WebhookPayload) -> DeliveryOutcome {
        let mut builder = self
            .http
            .post(target.url.expose_secret())
            .header("Content-Type", "application/json")
            .body(payload.body.to_string());

        for header in &target.headers {
            let name = match HeaderName::from_bytes(header.name.as_bytes()) {
                Ok(n) => n,
                Err(_) => {
                    return DeliveryOutcome::Permanent {
                        error: PermanentError::BadRequest {
                            detail: format!("invalid header name: {}", header.name),
                        },
                    };
                }
            };
            let value = match HeaderValue::from_str(header.value.expose_secret()) {
                Ok(v) => v,
                Err(_) => {
                    return DeliveryOutcome::Permanent {
                        error: PermanentError::BadRequest {
                            detail: format!("invalid header value for {}", header.name),
                        },
                    };
                }
            };
            builder = builder.header(name, value);
        }

        let response = match builder.send().await {
            Ok(r) => r,
            Err(_) => {
                return DeliveryOutcome::Transient {
                    error: TransientError::Network,
                    retry_after: None,
                };
            }
        };

        let status = response.status();
        match status.as_u16() {
            200..=299 => DeliveryOutcome::Delivered { external_ref: None },
            401 | 403 => DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure,
            },
            410 => DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone,
            },
            429 => DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after: parse_retry_after_seconds(response.headers().get("retry-after")),
            },
            400..=499 => DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest {
                    detail: format!("HTTP {}", status.as_u16()),
                },
            },
            500..=599 => DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx {
                    status: status.as_u16(),
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

/// Parse RFC 7231 `Retry-After`. Accepts either an integer number of
/// seconds (e.g. `Retry-After: 120`) or an HTTP-date
/// (e.g. `Retry-After: Wed, 21 Oct 2026 07:28:00 GMT`). HTTP-date is
/// converted to "seconds from now" using the current wall clock.
/// Clamps to [0, 1 hour] so a hostile upstream cannot schedule a
/// retry trillions of years in the future.
fn parse_retry_after_seconds(header: Option<&HeaderValue>) -> Option<ChronoDuration> {
    let raw = header.and_then(|v| v.to_str().ok()).map(str::trim)?;
    const MAX_SECS: i64 = 3_600;

    if let Ok(secs) = raw.parse::<i64>() {
        let bounded = secs.clamp(0, MAX_SECS);
        return Some(ChronoDuration::seconds(bounded));
    }

    // HTTP-date format. chrono's parse_from_rfc2822 covers the
    // RFC 7231 IMF-fixdate / RFC 850 / asctime-style families that
    // Retry-After permits in practice.
    if let Ok(when) = chrono::DateTime::parse_from_rfc2822(raw) {
        let now = chrono::Utc::now();
        let delta = when.with_timezone(&chrono::Utc) - now;
        let secs = delta.num_seconds().clamp(0, MAX_SECS);
        return Some(ChronoDuration::seconds(secs));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::targets::webhook::{WebhookHeader, WebhookMethod};
    use secrecy::SecretString;
    use serde_json::json;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Captured request for assertions.
    #[derive(Debug, Clone, Default)]
    struct Captured {
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    enum Reply {
        Status(u16, Vec<(&'static str, String)>, &'static str), // status, headers, body
        Network,                                                // close socket without responding
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
                    let mut lines = req.split("\r\n");
                    if let Some(request_line) = lines.next() {
                        if let Some(path) = request_line.split_whitespace().nth(1) {
                            cap.path = path.to_string();
                        }
                    }
                    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                    let header_block = &req[..body_start.saturating_sub(4)];
                    for line in header_block.split("\r\n").skip(1) {
                        if let Some((name, value)) = line.split_once(':') {
                            cap.headers
                                .push((name.trim().to_lowercase(), value.trim().to_string()));
                        }
                    }
                    cap.body = req[body_start..].to_string();
                    captured.lock().unwrap().push(cap);

                    match handler() {
                        Reply::Status(code, headers, body) => {
                            let mut resp = format!(
                                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                                code,
                                body.len()
                            );
                            for (name, val) in headers {
                                resp.push_str(&format!("{}: {}\r\n", name, val));
                            }
                            resp.push_str("\r\n");
                            socket.write_all(resp.as_bytes()).await.ok();
                            socket.write_all(body.as_bytes()).await.ok();
                        }
                        Reply::Network => {
                            // Drop the socket without writing — reqwest sees connection closed.
                        }
                    }
                });
            }
        });
        (addr, captured)
    }

    fn target_for(addr: SocketAddr, headers: Vec<WebhookHeader>) -> WebhookTarget {
        WebhookTarget {
            url: SecretString::from(format!("http://{}/hook", addr)),
            method: WebhookMethod::Post,
            headers,
        }
    }

    fn payload() -> WebhookPayload {
        WebhookPayload {
            body: json!({ "hello": "world" }),
        }
    }

    fn sender() -> WebhookSender {
        WebhookSender::new(reqwest::Client::new())
    }

    #[tokio::test]
    async fn http_200_returns_delivered_with_no_external_ref() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(200, vec![], "{}")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Delivered { external_ref: None }
        ));
    }

    #[tokio::test]
    async fn custom_headers_are_sent() {
        let (addr, cap) = spawn_mock(|| Reply::Status(200, vec![], "{}")).await;
        let headers = vec![WebhookHeader {
            name: "X-Custom".into(),
            value: SecretString::from("the-value".to_string()),
        }];
        let _ = sender().send(&target_for(addr, headers), &payload()).await;
        let cap = cap.lock().unwrap().clone();
        assert_eq!(cap.len(), 1);
        let observed: Vec<_> = cap[0]
            .headers
            .iter()
            .filter(|(n, _)| n == "x-custom")
            .collect();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].1, "the-value");
        assert_eq!(cap[0].body, "{\"hello\":\"world\"}");
    }

    #[tokio::test]
    async fn http_401_returns_permanent_auth_failure() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(401, vec![], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure
            }
        ));
    }

    #[tokio::test]
    async fn http_403_returns_permanent_auth_failure() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(403, vec![], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure
            }
        ));
    }

    #[tokio::test]
    async fn http_410_returns_permanent_destination_gone() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(410, vec![], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Permanent {
                error: PermanentError::DestinationGone
            }
        ));
    }

    #[tokio::test]
    async fn http_400_returns_permanent_bad_request() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(400, vec![], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest { detail },
            } => assert!(detail.contains("400")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn http_429_returns_rate_limited_with_retry_after() {
        let (addr, _cap) =
            spawn_mock(|| Reply::Status(429, vec![("Retry-After", "12".into())], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after,
            } => {
                assert_eq!(retry_after, Some(ChronoDuration::seconds(12)));
            }
            other => panic!("expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn http_503_returns_transient_upstream() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(503, vec![], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::Upstream5xx { status },
                ..
            } => assert_eq!(status, 503),
            other => panic!("expected Upstream5xx, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn network_close_returns_transient_network() {
        let (addr, _cap) = spawn_mock(|| Reply::Network).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        assert!(matches!(
            r.outcome,
            DeliveryOutcome::Transient {
                error: TransientError::Network,
                ..
            }
        ));
    }

    /// Negative test: a header name containing CRLF must be rejected
    /// before the request goes out (would otherwise enable header
    /// injection via SecretString contents).
    #[tokio::test]
    async fn invalid_header_name_short_circuits_to_bad_request() {
        let (addr, _cap) = spawn_mock(|| Reply::Status(200, vec![], "{}")).await;
        let headers = vec![WebhookHeader {
            name: "X-Bad\r\n".into(),
            value: SecretString::from("ok".to_string()),
        }];
        let r = sender().send(&target_for(addr, headers), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Permanent {
                error: PermanentError::BadRequest { detail },
            } => assert!(detail.contains("invalid header name")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    /// RFC 7231 also allows `Retry-After: HTTP-date`. Confirm we parse
    /// it as "seconds from now" instead of returning None and losing
    /// the upstream's backoff guidance.
    #[tokio::test]
    async fn http_429_with_retry_after_http_date_is_parsed() {
        // Build a date a few seconds in the future so the parser
        // converts it to a small positive duration without flaking.
        let future = chrono::Utc::now() + ChronoDuration::seconds(45);
        let http_date = future.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let http_date_clone = http_date.clone();
        let (addr, _cap) = spawn_mock(move || {
            Reply::Status(429, vec![("Retry-After", http_date_clone.clone())], "")
        })
        .await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after: Some(d),
            } => {
                // Allow ±5s wiggle room for clock drift between the
                // mock construction and the parser running.
                assert!(d >= ChronoDuration::seconds(30), "got {:?}", d);
                assert!(d <= ChronoDuration::seconds(60), "got {:?}", d);
            }
            other => panic!(
                "expected RateLimited with parsed retry_after, got {:?}",
                other
            ),
        }
    }

    /// Defend against a hostile `Retry-After: <huge number>` that
    /// would otherwise schedule the next attempt years in the future.
    #[tokio::test]
    async fn retry_after_clamps_to_one_hour() {
        let (addr, _cap) =
            spawn_mock(|| Reply::Status(429, vec![("Retry-After", "999999999".into())], "")).await;
        let r = sender().send(&target_for(addr, vec![]), &payload()).await;
        match r.outcome {
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after: Some(d),
            } => assert_eq!(d, ChronoDuration::seconds(3_600)),
            other => panic!("expected clamped RateLimited, got {:?}", other),
        }
    }
}
