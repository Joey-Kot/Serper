use chrono::{DateTime, Local};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use std::time::Duration;

const GOOGLE_API: &str = "https://google.serper.dev";
const SCRAPE_API: &str = "https://scrape.serper.dev";

#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub status_code: Option<u16>,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status_code: None,
        }
    }
}

pub struct ApiResponse {
    pub value: Value,
    pub received_at: DateTime<Local>,
}

pub struct SerperClient {
    client: Client,
    key: String,
    google_api: String,
    scrape_api: String,
}

impl SerperClient {
    pub fn new(key: String, timeout: u64) -> Result<Self, ApiError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("serper_cli/1.0")
            .build()
            .map_err(|err| ApiError::new(format!("failed to initialize HTTP client: {err}")))?;
        Ok(Self {
            client,
            key: key.trim().to_owned(),
            google_api: GOOGLE_API.to_owned(),
            scrape_api: SCRAPE_API.to_owned(),
        })
    }

    #[cfg(test)]
    pub fn with_test_server(mut self, url: &str) -> Self {
        self.google_api = url.to_owned();
        self.scrape_api = format!("{url}/scrape");
        self
    }

    pub fn request(
        &self,
        endpoint: &str,
        payload: Option<&Value>,
    ) -> Result<ApiResponse, ApiError> {
        if self.key.is_empty() {
            return Err(ApiError::new("SERPER_KEY is required"));
        }
        let url = if endpoint == "scrape" {
            self.scrape_api.clone()
        } else {
            format!("{}/{endpoint}", self.google_api)
        };
        let request = if let Some(payload) = payload {
            self.client.post(url).json(payload)
        } else {
            self.client.get(url)
        }
        .header("X-API-KEY", &self.key);
        let response = request
            .send()
            .map_err(|err| ApiError::new(format!("{endpoint} request error: {err}")))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let body = response
            .bytes()
            .map_err(|err| ApiError::new(format!("{endpoint} response read failed: {err}")))?;
        // Record the local system time when the complete response has been received.
        let received_at = Local::now();
        let parsed: Result<Value, _> = serde_json::from_slice(&body);
        if !status.is_success() {
            let mut message = format!("{endpoint} HTTP status error: {}", status.as_u16());
            match parsed {
                Ok(ref value) => {
                    if let Some(upstream) = value
                        .get("message")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        message.push_str(&format!(": {upstream}"));
                    }
                }
                Err(_) if !body.is_empty() => {
                    message = response_context(&message, status.as_u16(), &content_type, &body)
                }
                Err(_) => {}
            }
            return Err(ApiError {
                message,
                status_code: Some(status.as_u16()),
            });
        }
        if body.is_empty() {
            return Err(ApiError::new(format!("{endpoint} response is empty")));
        }
        let value = parsed.map_err(|err| {
            ApiError::new(response_context(
                &format!("{endpoint} response JSON parse failed: {err}"),
                status.as_u16(),
                &content_type,
                &body,
            ))
        })?;
        if !value.is_object() {
            return Err(ApiError::new(format!(
                "{endpoint} response must be a JSON object"
            )));
        }
        if value.get("error") == Some(&Value::Bool(true)) {
            return Err(ApiError {
                message: value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("upstream returned an error")
                    .to_owned(),
                status_code: value
                    .get("statusCode")
                    .and_then(Value::as_u64)
                    .and_then(|v| u16::try_from(v).ok()),
            });
        }
        Ok(ApiResponse { value, received_at })
    }
}

fn response_context(message: &str, status: u16, content_type: &str, body: &[u8]) -> String {
    let mut details = vec![format!("status={status}")];
    if !content_type.trim().is_empty() {
        details.push(format!("content-type={:?}", content_type.trim()));
    }
    let preview = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let preview = if preview.chars().count() > 300 {
        format!("{}...", preview.chars().take(300).collect::<String>())
    } else {
        preview
    };
    if !preview.is_empty() {
        details.push(format!("body={preview:?}"));
    }
    format!("{message} ({})", details.join(", "))
}
