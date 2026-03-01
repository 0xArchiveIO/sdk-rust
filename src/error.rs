/// Error types for the 0xArchive SDK.

/// An error returned by the 0xArchive API or the SDK itself.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API returned an error response.
    #[error("{message} (HTTP {code})")]
    Api {
        message: String,
        code: u16,
        request_id: Option<String>,
    },

    /// The request timed out.
    #[error("request timed out")]
    Timeout,

    /// An HTTP transport error occurred.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Failed to deserialize the response body.
    #[error("deserialization error: {0}")]
    Deserialize(String),

    /// Invalid parameter supplied by the caller.
    #[error("invalid parameter: {0}")]
    InvalidParam(String),

    /// WebSocket error (only available with the `websocket` feature).
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

/// Convenience alias used throughout the SDK.
pub type Result<T> = std::result::Result<T, Error>;
