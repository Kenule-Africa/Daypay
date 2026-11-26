use reqwest::{Client, StatusCode, Url};
use serde::Serialize;
use std::{future::Future, pin::Pin, time::Duration};
use uuid::Uuid;

pub const SENDER: &str = "DEYPAY <deypay@kenuleafrica.xyz>";

#[derive(Serialize)]
pub struct Message {
    /// Names the flow in the provider idempotency key; never sent as content.
    #[serde(skip)]
    pub kind: &'static str,
    pub from: &'static str,
    pub to: [String; 1],
    pub subject: &'static str,
    pub text: String,
    pub html: String,
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn verification_message(address: &str, link: &Url, expires: &str) -> Message {
    let subject = "Verify your DEYPAY email address";
    let warning = "If you did not request this verification, you can ignore this email.";
    let text = format!(
        "Verify your DEYPAY email address\n\nOpen this link and select Verify email:\n{link}\n\nThis link expires at {expires} UTC.\n\n{warning}\n\nDEYPAY · Kenule Africa"
    );
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body style="margin:0;background:#f5f7f8;color:#172e2c;font:16px/1.6 Arial,sans-serif"><main style="max-width:540px;margin:auto;background:white;padding:32px 24px"><p>DEYPAY</p><h1 style="font-size:26px">Verify your email address</h1><p>Confirm this email address for your DEYPAY account.</p><p><a href="{}" style="display:inline-block;background:#07594c;color:white;padding:12px 20px;border-radius:6px">Verify email address</a></p><p>This link expires at {} UTC.</p><p>{warning}</p><p>DEYPAY · Kenule Africa</p></main></body></html>"#,
        escape(link.as_str()),
        escape(expires)
    );
    Message {
        kind: "verify-email",
        from: SENDER,
        to: [address.into()],
        subject,
        text,
        html,
    }
}

/// Reset email (IMP-P3-008). The link carries the single-use secret in its fragment.
pub fn password_reset_message(address: &str, link: &Url, expires: &str) -> Message {
    let subject = "Reset your DEYPAY password";
    let warning = "If you did not ask to reset your password, you can ignore this email. Your password stays the same.";
    let effect = "The link works once. Resetting your password signs you out on every device.";
    let text = format!(
        "Reset your DEYPAY password

Open this link to choose a new password:
{link}

This link expires at {expires} UTC. {effect}

{warning}

DEYPAY · Kenule Africa"
    );
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body style="margin:0;background:#f5f7f8;color:#172e2c;font:16px/1.6 Arial,sans-serif"><main style="max-width:540px;margin:auto;background:white;padding:32px 24px"><p>DEYPAY</p><h1 style="font-size:26px">Reset your password</h1><p>Choose a new password for your DEYPAY account.</p><p><a href="{}" style="display:inline-block;background:#07594c;color:white;padding:12px 20px;border-radius:6px">Choose a new password</a></p><p>This link expires at {} UTC. {effect}</p><p>{warning}</p><p>DEYPAY · Kenule Africa</p></main></body></html>"#,
        escape(link.as_str()),
        escape(expires)
    );
    Message {
        kind: "reset-password",
        from: SENDER,
        to: [address.into()],
        subject,
        text,
        html,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Accepted(Option<Uuid>),
    Transient,
    RateLimited(u64),
    Permanent,
}
pub type SendResult<'a> = Pin<Box<dyn Future<Output = Delivery> + Send + 'a>>;
pub trait EmailDelivery: Send + Sync {
    fn send<'a>(&'a self, id: Uuid, message: &'a Message) -> SendResult<'a>;
}

// This adapter makes no external calls. The bounded in-memory inbox is inspectable by local tests.
#[derive(Default)]
pub struct MockDelivery(std::sync::Mutex<Option<(Uuid, String)>>);
impl MockDelivery {
    pub fn last_message(&self) -> Option<(Uuid, String)> {
        self.0.lock().ok()?.clone()
    }
}
impl EmailDelivery for MockDelivery {
    fn send<'a>(&'a self, id: Uuid, message: &'a Message) -> SendResult<'a> {
        Box::pin(async move {
            if let Ok(mut inbox) = self.0.lock() {
                *inbox = Some((id, message.text.clone()));
                Delivery::Accepted(None)
            } else {
                Delivery::Transient
            }
        })
    }
}

pub struct Resend {
    client: Client,
    key: reqwest::header::HeaderValue,
    endpoint: Url,
}

impl Resend {
    pub fn new(key: &str, timeout: Duration) -> Result<Self, &'static str> {
        if key.trim() != key || key.is_empty() || key.chars().any(char::is_whitespace) {
            return Err("Invalid RESEND_API_KEY");
        }
        let mut header = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| "Invalid RESEND_API_KEY")?;
        header.set_sensitive(true);
        let client = Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Cannot configure Resend transport")?;
        Ok(Self {
            client,
            key: header,
            endpoint: Url::parse("https://api.resend.com/emails").expect("fixed Resend URL"),
        })
    }
    async fn deliver(&self, id: Uuid, message: &Message) -> Delivery {
        let Ok(mut response) = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::AUTHORIZATION, self.key.clone())
            .header("Idempotency-Key", format!("{}/{id}", message.kind))
            .json(message)
            .send()
            .await
        else {
            return Delivery::Transient;
        };
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let delay = response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            return Delivery::RateLimited(delay);
        }
        if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
            return Delivery::Transient;
        }
        if !status.is_success() && status != StatusCode::CONFLICT {
            return Delivery::Permanent;
        }
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if bytes.len() + chunk.len() <= 16_384 => {
                    bytes.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => return Delivery::Transient,
            }
        }
        let body = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default();
        if status == StatusCode::CONFLICT {
            return if matches!(
                body["name"].as_str(),
                Some("concurrent_idempotent_requests" | "resource_locked")
            ) {
                Delivery::Transient
            } else {
                Delivery::Permanent
            };
        }
        match body["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => Delivery::Accepted(Some(id)),
            None => Delivery::Transient,
        }
    }
}
impl EmailDelivery for Resend {
    fn send<'a>(&'a self, id: Uuid, message: &'a Message) -> SendResult<'a> {
        Box::pin(self.deliver(id, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn resend_wire_contract_and_errors() {
        for (status, response, outcome) in [
            (
                "200 OK",
                r#"{"id":"9dc49069-1849-49ae-9548-34493e43f2ac"}"#,
                Delivery::Accepted(Some(Uuid::from_u128(0x9dc49069184949ae954834493e43f2ac))),
            ),
            ("200 OK", "{}", Delivery::Transient),
            (
                "409 Conflict",
                r#"{"name":"concurrent_idempotent_requests"}"#,
                Delivery::Transient,
            ),
            (
                "409 Conflict",
                r#"{"name":"invalid_idempotent_request"}"#,
                Delivery::Permanent,
            ),
            ("429 Too Many Requests", "{}", Delivery::RateLimited(40)),
            ("503 Service Unavailable", "{}", Delivery::Transient),
            ("403 Forbidden", "{}", Delivery::Permanent),
            ("302 Found", "{}", Delivery::Permanent),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                let split = loop {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(end) = request.windows(4).position(|p| p == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&request[..split]).to_ascii_lowercase();
                assert!(headers.starts_with("post /emails http/1.1"));
                assert!(headers.contains("authorization: bearer test-key"));
                assert!(headers.contains(
                    "idempotency-key: verify-email/00000000-0000-0000-0000-000000000001"
                ));
                let size = headers
                    .lines()
                    .find_map(|s| s.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                while request.len() < split + size {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                }
                let data: serde_json::Value = serde_json::from_slice(&request[split..]).unwrap();
                assert_eq!(data["from"], SENDER);
                assert_eq!(data["to"][0], "test@example.test");
                assert!(data["text"].as_str().unwrap().contains("did not request"));
                assert!(
                    data["html"]
                        .as_str()
                        .unwrap()
                        .contains("Verify email address")
                );
                write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nRetry-After: 40\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
            });
            let mut adapter = Resend::new("test-key", Duration::from_secs(5)).unwrap();
            adapter.endpoint = Url::parse(&format!("http://{address}/emails")).unwrap();
            let msg = verification_message(
                "test@example.test",
                &Url::parse("https://example.test/verify-email#example").unwrap(),
                "2030-05-20 09:15:30",
            );
            assert_eq!(adapter.send(Uuid::from_u128(1), &msg).await, outcome);
            server.join().unwrap();
        }
        assert_eq!(escape("<&\"'>"), "&lt;&amp;&quot;&#39;&gt;");
    }
}
