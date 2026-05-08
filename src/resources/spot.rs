use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, SpotPair, SpotTwapStatus, Timestamp};

/// Hyperliquid Spot pairs resource (`/pairs`, `/pairs/{symbol}`).
///
/// Spot uses the term `pairs` rather than `instruments` to match the REST
/// surface and the dashed canonical symbol form (`HYPE-USDC`, `PURR-USDC`).
#[derive(Debug, Clone)]
pub struct SpotPairsResource {
    http: HttpClient,
    prefix: String,
}

impl SpotPairsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// List every active spot pair.
    pub async fn list(&self) -> Result<Vec<SpotPair>> {
        self.http
            .get(&format!("{}/pairs", self.prefix), &[])
            .await
    }

    /// Get a single spot pair by symbol (dashed canonical, e.g. `HYPE-USDC`).
    pub async fn get(&self, symbol: &str) -> Result<SpotPair> {
        self.http
            .get(&format!("{}/pairs/{}", self.prefix, symbol), &[])
            .await
    }
}

/// Parameters for paginated TWAP queries.
#[derive(Debug, Default, Clone)]
pub struct SpotTwapParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Hyperliquid Spot TWAP resource (`/twap/{symbol}`, `/twap/user/{user}`).
#[derive(Debug, Clone)]
pub struct SpotTwapResource {
    http: HttpClient,
    prefix: String,
}

impl SpotTwapResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get TWAP execution statuses for a spot symbol.
    pub async fn by_symbol(
        &self,
        symbol: &str,
        params: SpotTwapParams,
    ) -> Result<CursorResponse<Vec<SpotTwapStatus>>> {
        let mut qp = vec![];
        if let Some(s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/twap/{}", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get TWAP execution statuses for a user address (across every spot symbol).
    pub async fn by_user(
        &self,
        user: &str,
        params: SpotTwapParams,
    ) -> Result<CursorResponse<Vec<SpotTwapStatus>>> {
        let mut qp = vec![];
        if let Some(s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/twap/user/{}", self.prefix, user), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
