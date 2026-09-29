//! Error types for the 0xArchive SDK.

use std::fmt;

/// The stable, machine-readable reason for an API or WebSocket error, from the
/// `error_code` field of the error body.
///
/// Match on the named variants for the codes the API documents. A code this
/// SDK version does not know arrives as [`ErrorCode::Other`] with the code
/// string unchanged, so an older SDK keeps working when the API adds one. The
/// enum is `#[non_exhaustive]`: a `match` needs a wildcard arm, and a later
/// minor release may give a code that arrives as `Other` today its own
/// variant.
///
/// ```
/// use oxarchive::ErrorCode;
///
/// assert_eq!(ErrorCode::from("invalid_symbol"), ErrorCode::InvalidSymbol);
/// assert_eq!(ErrorCode::InvalidSymbol.as_str(), "invalid_symbol");
/// assert_eq!(
///     ErrorCode::from("a_future_code"),
///     ErrorCode::Other("a_future_code".to_string()),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// A parameter is malformed or has a value the route does not accept.
    /// `param` and `valid_values` on [`Error::Api`] say which, when sent.
    InvalidParameter,
    /// The symbol does not exist on this venue.
    InvalidSymbol,
    /// The `interval` value is not one the route accepts.
    InvalidInterval,
    /// The `cursor` could not be read. Restart paging without a cursor.
    InvalidCursor,
    /// The time range is invalid: `start` after `end`, or a time that cannot
    /// be parsed.
    InvalidTimeRange,
    /// The whole requested range is before the dataset's coverage begins.
    RangeBeforeCoverage,
    /// The requested span is longer than the plan allows per request.
    HistoricalRangeExceeded,
    /// The requested time is older than the plan's history window.
    HistoricalDepthExceeded,
    /// The venue does not offer this datatype, channel or mode. The message
    /// names the venues that do.
    UnsupportedForVenue,
    /// No route matches the request path.
    RouteNotFound,
    /// The resource id is unknown.
    NotFound,
    /// Missing or invalid credentials.
    Unauthorized,
    /// The credentials do not grant access to this resource.
    Forbidden,
    /// The monthly credits are spent.
    InsufficientCredits,
    /// Too many requests or subscribe operations; retry later.
    RateLimited,
    /// The request conflicts with current state, for example a replay is
    /// already running on the connection.
    Conflict,
    /// A backing store failed or timed out; retry later.
    UpstreamUnavailable,
    /// An unexpected server error.
    InternalError,
    /// WebSocket only: this endpoint does not serve the channel or operation.
    /// The message names the endpoint that does.
    EndpointUnsupported,
    /// WebSocket only: the connection fell behind a stream and messages were
    /// dropped. Subscribe again, or restart the replay, to resync.
    SlowConsumer,
    /// Account positions are temporarily unavailable.
    PositionsUnavailable,
    /// The account already holds the maximum number of API keys.
    ApiKeyLimitReached,
    /// The operation is not permitted for an OAuth-authorized client.
    OauthNotPermitted,
    /// A code this SDK version does not name, kept as sent.
    Other(String),
}

impl ErrorCode {
    /// The wire value, for example `"invalid_symbol"`.
    pub fn as_str(&self) -> &str {
        match self {
            ErrorCode::InvalidParameter => "invalid_parameter",
            ErrorCode::InvalidSymbol => "invalid_symbol",
            ErrorCode::InvalidInterval => "invalid_interval",
            ErrorCode::InvalidCursor => "invalid_cursor",
            ErrorCode::InvalidTimeRange => "invalid_time_range",
            ErrorCode::RangeBeforeCoverage => "range_before_coverage",
            ErrorCode::HistoricalRangeExceeded => "historical_range_exceeded",
            ErrorCode::HistoricalDepthExceeded => "historical_depth_exceeded",
            ErrorCode::UnsupportedForVenue => "unsupported_for_venue",
            ErrorCode::RouteNotFound => "route_not_found",
            ErrorCode::NotFound => "not_found",
            ErrorCode::Unauthorized => "unauthorized",
            ErrorCode::Forbidden => "forbidden",
            ErrorCode::InsufficientCredits => "insufficient_credits",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::Conflict => "conflict",
            ErrorCode::UpstreamUnavailable => "upstream_unavailable",
            ErrorCode::InternalError => "internal_error",
            ErrorCode::EndpointUnsupported => "endpoint_unsupported",
            ErrorCode::SlowConsumer => "slow_consumer",
            ErrorCode::PositionsUnavailable => "positions_unavailable",
            ErrorCode::ApiKeyLimitReached => "api_key_limit_reached",
            ErrorCode::OauthNotPermitted => "oauth_not_permitted",
            ErrorCode::Other(code) => code,
        }
    }
}

impl From<&str> for ErrorCode {
    fn from(code: &str) -> Self {
        match code {
            "invalid_parameter" => ErrorCode::InvalidParameter,
            "invalid_symbol" => ErrorCode::InvalidSymbol,
            "invalid_interval" => ErrorCode::InvalidInterval,
            "invalid_cursor" => ErrorCode::InvalidCursor,
            "invalid_time_range" => ErrorCode::InvalidTimeRange,
            "range_before_coverage" => ErrorCode::RangeBeforeCoverage,
            "historical_range_exceeded" => ErrorCode::HistoricalRangeExceeded,
            "historical_depth_exceeded" => ErrorCode::HistoricalDepthExceeded,
            "unsupported_for_venue" => ErrorCode::UnsupportedForVenue,
            "route_not_found" => ErrorCode::RouteNotFound,
            "not_found" => ErrorCode::NotFound,
            "unauthorized" => ErrorCode::Unauthorized,
            "forbidden" => ErrorCode::Forbidden,
            "insufficient_credits" => ErrorCode::InsufficientCredits,
            "rate_limited" => ErrorCode::RateLimited,
            "conflict" => ErrorCode::Conflict,
            "upstream_unavailable" => ErrorCode::UpstreamUnavailable,
            "internal_error" => ErrorCode::InternalError,
            "endpoint_unsupported" => ErrorCode::EndpointUnsupported,
            "slow_consumer" => ErrorCode::SlowConsumer,
            "positions_unavailable" => ErrorCode::PositionsUnavailable,
            "api_key_limit_reached" => ErrorCode::ApiKeyLimitReached,
            "oauth_not_permitted" => ErrorCode::OauthNotPermitted,
            other => ErrorCode::Other(other.to_string()),
        }
    }
}

impl From<String> for ErrorCode {
    fn from(code: String) -> Self {
        ErrorCode::from(code.as_str())
    }
}

impl std::str::FromStr for ErrorCode {
    type Err = std::convert::Infallible;

    fn from_str(code: &str) -> std::result::Result<Self, Self::Err> {
        Ok(ErrorCode::from(code))
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Ok(ErrorCode::from(code))
    }
}

/// An error returned by the 0xArchive API or the SDK itself.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API returned an error response.
    ///
    /// `error_code` is the stable reason to branch on; `message` is for
    /// people and may change wording. Every field but `message` and `code`
    /// is `None` when the response body did not carry it (for example a
    /// plain-text error from a proxy).
    #[error(
        "{message} (HTTP {code}{})",
        .error_code.as_ref().map(|c| format!(", {c}")).unwrap_or_default()
    )]
    Api {
        /// Human-readable description of the error.
        message: String,
        /// HTTP status code.
        code: u16,
        /// Request identifier, useful when contacting support.
        request_id: Option<String>,
        /// Stable machine-readable reason (`invalid_symbol`, `rate_limited`, ...).
        error_code: Option<ErrorCode>,
        /// The request parameter the error is about, when there is one.
        param: Option<String>,
        /// The values `param` accepts, when the API lists them. Boxed to keep
        /// `Error` small; it reads as a slice.
        valid_values: Option<Box<[String]>>,
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

impl Error {
    /// The API's `error_code` for an [`Error::Api`], or `None` for other
    /// errors and for API errors whose body carried no code.
    pub fn error_code(&self) -> Option<&ErrorCode> {
        match self {
            Error::Api { error_code, .. } => error_code.as_ref(),
            _ => None,
        }
    }

    /// The HTTP status of an [`Error::Api`].
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// The request identifier of an [`Error::Api`], when the API sent one.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Error::Api { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// The request parameter an [`Error::Api`] is about, when the API named
    /// one.
    pub fn param(&self) -> Option<&str> {
        match self {
            Error::Api { param, .. } => param.as_deref(),
            _ => None,
        }
    }

    /// The values the parameter accepts, when the API listed them.
    pub fn valid_values(&self) -> Option<&[String]> {
        match self {
            Error::Api { valid_values, .. } => valid_values.as_deref(),
            _ => None,
        }
    }
}

/// Convenience alias used throughout the SDK.
pub type Result<T> = std::result::Result<T, Error>;
