use crate::model::*;
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub ambiguous: bool,
    pub retry_after: u64,
    pub retryable: bool,
}
impl ApiError {
    fn network() -> Self {
        Self {
            message: "Network request did not complete. Check your connection.".into(),
            ambiguous: true,
            retry_after: 30,
            retryable: true,
        }
    }
}
#[derive(Clone)]
pub struct Provider {
    client: Client,
    schedule: Arc<Mutex<Instant>>,
    base: String,
}
impl Provider {
    #[cfg(test)]
    pub fn mock(base: String) -> Self {
        let mut provider = Self::new();
        provider.base = base;
        provider
    }
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(40))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("ResendClient/0.1.0")
                .build()
                .expect("HTTP client"),
            schedule: Arc::new(Mutex::new(Instant::now())),
            base: "https://api.resend.com".into(),
        }
    }
    pub async fn request(
        &self,
        key: &str,
        method: Method,
        path: &str,
        payload: Option<&Value>,
        intent: Option<&str>,
    ) -> std::result::Result<Value, ApiError> {
        let mut schedule = self.schedule.lock().await;
        let delay = schedule.saturating_duration_since(Instant::now());
        if delay > Duration::from_secs(1) {
            return Err(ApiError {
                message: "Resend requests are paused until the server backoff ends.".into(),
                ambiguous: false,
                retry_after: delay.as_secs() + 1,
                retryable: true,
            });
        }
        if *schedule > Instant::now() {
            tokio::time::sleep_until((*schedule).into()).await;
        }
        *schedule = Instant::now() + Duration::from_millis(250);
        let mut req = self
            .client
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(key);
        if let Some(v) = payload {
            req = req.json(v);
        }
        if let Some(id) = intent {
            req = req.header("Idempotency-Key", format!("resend-client/{id}"));
        }
        let mut response = req.send().await.map_err(|_| ApiError::network())?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .map(parse_retry_after)
            .unwrap_or(30)
            .clamp(1, 3600);
        if status == StatusCode::TOO_MANY_REQUESTS {
            *schedule = Instant::now() + Duration::from_secs(retry_after);
        }
        // Keep the shared scheduler locked through the response, bounding team concurrency.
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ApiError::network())? {
            if bytes.len() + chunk.len() > 8_000_000 {
                return Err(ApiError {
                    message: "Provider response exceeded the safe size limit.".into(),
                    ambiguous: status.is_success(),
                    retry_after: 300,
                    retryable: false,
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| ApiError {
            message: "Provider returned an unreadable response.".into(),
            ambiguous: true,
            retry_after: 60,
            retryable: true,
        })?;
        if status.is_success() {
            return Ok(value);
        }
        let name = value["name"].as_str().unwrap_or("");
        let quota = name.contains("quota")
            || value["message"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .contains("quota");
        let message=match status.as_u16(){
            401|403=>"Access denied. Check the credential and its team permissions.",
            404=>"This record is no longer retrievable from Resend. Downloaded content is preserved.",
            429 if quota=>"Account quota reached. Sending is paused; check your Resend plan and usage.",
            429=>"Resend is rate limiting requests. Work will resume after the server's backoff.",
            400|422=>"Resend rejected this request. Check sender verification, recipients, content, and account limits.",
            409=>"The provider could not resolve this attempt yet. Its original payload and key are preserved.",
            _=>"Resend is temporarily unavailable. The request may have been accepted.",
        }.to_string();
        if quota {
            *schedule = Instant::now() + Duration::from_secs(60);
        }
        Err(ApiError {
            message,
            ambiguous: status.is_server_error() || matches!(status.as_u16(), 408 | 409),
            retry_after: if quota { 3600 } else { retry_after },
            retryable: !quota
                && (status == StatusCode::TOO_MANY_REQUESTS
                    || status.is_server_error()
                    || matches!(status.as_u16(), 408 | 409)),
        })
    }
    pub async fn get(&self, key: &str, path: &str) -> Result<Value> {
        self.request(key, Method::GET, path, None, None)
            .await
            .map_err(|e| e.message)
    }
}
fn parse_retry_after(s: &str) -> u64 {
    if let Ok(seconds) = s.parse() {
        return seconds;
    }
    chrono::DateTime::parse_from_rfc2822(s)
        .map(|d| (d.timestamp() - chrono::Utc::now().timestamp()).max(1) as u64)
        .unwrap_or(30)
}

pub fn download_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not prepare attachment download.".into())
}
pub async fn download(url: &str) -> Result<Vec<u8>> {
    let url = reqwest::Url::parse(url).map_err(|_| "Invalid attachment URL.")?;
    let host = url.host_str().ok_or("Invalid attachment host.")?;
    // Only documented provider CDN hosts; unknown storage hosts fail closed.
    if url.scheme() != "https"
        || !["inbound-cdn.resend.com", "outbound-cdn.resend.com"].contains(&host)
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("The attachment host is outside the supported Resend CDN policy.".into());
    }
    let mut res = download_client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "Attachment download failed. Check your connection.")?;
    if !res.status().is_success() {
        return Err("Attachment link is unavailable. Refresh and try again.".into());
    }
    let mut bytes = vec![];
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|_| "Attachment download was interrupted.")?
    {
        if bytes.len() + chunk.len() > 40_000_000 {
            return Err("Attachment is too large (40 MB maximum).".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_timing() {
        assert_eq!(parse_retry_after("47"), 47);
        assert_eq!(parse_retry_after("bad"), 30);
    }
    #[tokio::test]
    async fn attachment_hosts_fail_closed() {
        for u in [
            "http://inbound-cdn.resend.com/x",
            "https://127.0.0.1/x",
            "https://inbound-cdn.resend.com.evil.test/a",
            "https://user@inbound-cdn.resend.com/a",
        ] {
            assert!(download(u).await.is_err());
        }
    }
}
