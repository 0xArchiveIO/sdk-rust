//! Internal HTTP client wrapping `reqwest`.

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::Arc;
pub(crate) use std::time::Duration;

use crate::error::{Error, ErrorCode};
use crate::types::{ApiEnvelope, MetaEnvelope, MetaResponse, ResponseMeta};

/// The API version this SDK is built for, sent on every request as the
/// `0xArchive-Version` header and on the WebSocket connection as `version`.
///
/// It selects the response shapes the SDK types describe: the standard
/// `{success, data, meta}` envelope on every route, RFC 3339 times with
/// `*_ms` integers, the contract error codes and, on the WebSocket, Lighter
/// replay rows in the live shapes.
pub const API_VERSION: &str = "2026-10-01";

/// Name of the request header that carries [`API_VERSION`].
pub const API_VERSION_HEADER: &str = "0xArchive-Version";

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
        headers.insert(
            reqwest::header::HeaderName::from_static("0xarchive-version"),
            HeaderValue::from_static(API_VERSION),
        );

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

    /// Send a GET request and return the data with its paging state
    /// (`next_cursor`, `has_more`) and the full `meta` block.
    pub async fn get_with_cursor<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> crate::error::Result<MetaResponse<T>> {
        let (data, meta) = self.get_with_meta(path, params).await?;
        Ok(MetaResponse::new(data, meta))
    }

    /// Send a GET request and return the data with the response's full
    /// `meta` block (cursor, snapshot context, finalization boundaries).
    pub async fn get_with_meta<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> crate::error::Result<(T, ResponseMeta)> {
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

        let status = resp.status();
        let body = resp.text().await.map_err(Error::Http)?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body));
        }

        let envelope: MetaEnvelope<T> = serde_json::from_str(&body)
            .map_err(|e| Error::Deserialize(format!("{e}: {body}")))?;
        Ok((envelope.data, envelope.meta.unwrap_or_default()))
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

    /// Send a request and return the whole decoded JSON body rather than just
    /// its `data` member.
    ///
    /// The webhook management routes put information in siblings of `data`:
    /// the watched wallet cap arrives as `limit`, the secret and resume routes
    /// add `note`, and the resume routes add `gap` and `resumed_count`. Some
    /// routes answer `{"success": true}` with no `data` at all. Reading the
    /// whole body keeps all of that.
    pub(crate) async fn request_envelope(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> crate::error::Result<serde_json::Value> {
        let url = format!("{}{}", self.config.base_url, path);

        let filtered: Vec<(&str, &str)> = query
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let mut req = self.inner.request(method, &url).query(&filtered);
        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Http(e)
            }
        })?;

        let status = resp.status();
        let text = resp.text().await.map_err(Error::Http)?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &text));
        }

        // A 204 or an empty 200 is a success with nothing to read.
        if text.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }

        serde_json::from_str(&text).map_err(|e| Error::Deserialize(format!("{e}: {text}")))
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
        let envelope_error = match serde_json::from_str::<ApiEnvelope<T>>(&body) {
            Ok(envelope) => return Ok(envelope.data),
            Err(e) => e,
        };

        serde_json::from_str::<T>(&body).map_err(|direct_error| {
            // Report the error that describes the body: the envelope's when
            // the body is an envelope, the direct one otherwise.
            let is_envelope = serde_json::from_str::<serde_json::Value>(&body)
                .map(|v| v.get("data").is_some())
                .unwrap_or(false);
            let e = if is_envelope { envelope_error } else { direct_error };
            Error::Deserialize(format!("{e}: {body}"))
        })
    }

    fn parse_error(&self, code: u16, body: &str) -> Error {
        parse_error(code, body)
    }

    /// Expose the base URL (used in WebSocket client).
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}

/// Map a non-2xx response to [`Error::Api`], keeping the contract fields
/// (`error_code`, `request_id`, `param`, `valid_values`) when the body is the
/// JSON error envelope.
pub(crate) fn parse_error(code: u16, body: &str) -> Error {
    #[derive(Deserialize, Default)]
    struct ErrBody {
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        request_id: Option<String>,
        #[serde(default)]
        error_code: Option<ErrorCode>,
        #[serde(default)]
        param: Option<String>,
        #[serde(default)]
        valid_values: Option<Vec<String>>,
    }

    let parsed: ErrBody = serde_json::from_str(body).unwrap_or_default();

    let message = parsed
        .error
        .or(parsed.message)
        .unwrap_or_else(|| format!("Request failed with status {code}"));

    Error::Api {
        message,
        code,
        request_id: parsed.request_id,
        error_code: parsed.error_code,
        param: parsed.param,
        valid_values: parsed.valid_values.map(Vec::into_boxed_slice),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_stays_small() {
        // Keeps `Result<T, Error>` under clippy's large-error threshold.
        assert!(std::mem::size_of::<Error>() < 128, "{}", std::mem::size_of::<Error>());
    }

    #[test]
    fn contract_error_body_keeps_every_field() {
        let body = r#"{"code":400,"error":"Invalid side 'B'. Use buy or sell.","error_code":"invalid_parameter","param":"side","request_id":"req-1","success":false,"valid_values":["buy","sell"]}"#;
        let err = parse_error(400, body);
        assert_eq!(err.status(), Some(400));
        assert_eq!(err.error_code(), Some(&ErrorCode::InvalidParameter));
        assert_eq!(err.request_id(), Some("req-1"));
        assert_eq!(err.param(), Some("side"));
        assert_eq!(err.valid_values(), Some(&["buy".to_string(), "sell".to_string()][..]));
        assert_eq!(
            err.to_string(),
            "Invalid side 'B'. Use buy or sell. (HTTP 400, invalid_parameter)"
        );
    }

    #[test]
    fn a_body_that_is_not_json_still_maps_to_an_api_error() {
        let err = parse_error(502, "<html>Bad Gateway</html>");
        match err {
            Error::Api {
                message,
                code,
                request_id,
                error_code,
                param,
                valid_values,
            } => {
                assert_eq!(message, "Request failed with status 502");
                assert_eq!(code, 502);
                assert!(request_id.is_none());
                assert!(error_code.is_none());
                assert!(param.is_none());
                assert!(valid_values.is_none());
            }
            other => panic!("expected an API error, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_code_is_kept_as_sent() {
        let err = parse_error(409, r#"{"code":409,"error":"moved","error_code":"snapshot_advanced"}"#);
        assert_eq!(
            err.error_code(),
            Some(&ErrorCode::Other("snapshot_advanced".to_string()))
        );
        assert_eq!(err.to_string(), "moved (HTTP 409, snapshot_advanced)");
    }
}
