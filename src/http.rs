/// Internal HTTP client wrapping `reqwest`.

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::Arc;
pub(crate) use std::time::Duration;

use crate::error::Error;
use crate::types::ApiEnvelope;

/// Configuration for [`HttpClient`].
#[derive(Debug, Clone)]
pub(crate) struct HttpConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout: Duration,
}

/// A thin wrapper around `reqwest::Client` that handles authentication,
/// response envelope unwrapping, and error mapping.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    config: Arc<HttpConfig>,
}

impl HttpClient {
    pub(crate) fn new(config: HttpConfig) -> crate::error::Result<Self> {
        if config.api_key.is_empty() {
            return Err(Error::InvalidParam("API key must not be empty".into()));
        }

        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_str(&config.api_key).map_err(|_| {
            Error::InvalidParam("API key contains invalid characters".into())
        })?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let inner = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()?;

        Ok(Self {
            inner,
            config: Arc::new(config),
        })
    }

    /// Send a GET request and deserialize the response data.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> crate::error::Result<T> {
        self.get_with_timeout(path, params, None).await
    }

    /// Like [`get`](Self::get), but with an optional per-request timeout override.
    pub async fn get_with_timeout<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        timeout: Option<Duration>,
    ) -> crate::error::Result<T> {
        let url = format!("{}{}", self.config.base_url, path);

        // Filter out empty-value params
        let filtered: Vec<(&str, &str)> = params
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let mut req = self.inner.get(&url).query(&filtered);
        if let Some(t) = timeout {
            req = req.timeout(t);
        }

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Http(e)
            }
        })?;

        self.handle_response(resp).await
    }

    /// Send a GET request and return the full envelope including cursor metadata.
    pub async fn get_with_cursor<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> crate::error::Result<(T, Option<String>)> {
        let url = format!("{}{}", self.config.base_url, path);

        let filtered: Vec<(&str, &str)> = params
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let resp = self.inner.get(&url).query(&filtered).send().await.map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Http(e)
            }
        })?;

        self.handle_cursor_response(resp).await
    }

    /// Send a POST request and deserialize the response data.
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> crate::error::Result<T> {
        let url = format!("{}{}", self.config.base_url, path);
        let resp = self.inner.post(&url).json(body).send().await.map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Http(e)
            }
        })?;

        self.handle_response(resp).await
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn handle_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> crate::error::Result<T> {
        let status = resp.status();
        let body = resp.text().await.map_err(Error::Http)?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body));
        }

        // Try unwrapping the API envelope first, fall back to direct deserialization.
        if let Ok(envelope) = serde_json::from_str::<ApiEnvelope<T>>(&body) {
            return Ok(envelope.data);
        }

        serde_json::from_str::<T>(&body)
            .map_err(|e| Error::Deserialize(format!("{e}: {body}")))
    }

    async fn handle_cursor_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> crate::error::Result<(T, Option<String>)> {
        let status = resp.status();
        let body = resp.text().await.map_err(Error::Http)?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body));
        }

        let envelope: ApiEnvelope<T> = serde_json::from_str(&body)
            .map_err(|e| Error::Deserialize(format!("{e}: {body}")))?;

        let cursor = envelope.meta.and_then(|m| m.next_cursor);
        Ok((envelope.data, cursor))
    }

    fn parse_error(&self, code: u16, body: &str) -> Error {
        #[derive(Deserialize)]
        struct ErrBody {
            error: Option<String>,
            message: Option<String>,
            request_id: Option<String>,
        }

        let parsed: ErrBody = serde_json::from_str(body).unwrap_or(ErrBody {
            error: None,
            message: None,
            request_id: None,
        });

        let message = parsed
            .error
            .or(parsed.message)
            .unwrap_or_else(|| format!("Request failed with status {code}"));

        Error::Api {
            message,
            code,
            request_id: parsed.request_id,
        }
    }

    /// Expose the base URL (used in WebSocket client).
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}
