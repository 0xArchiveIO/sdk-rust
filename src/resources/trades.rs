use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, Timestamp, Trade};

/// Parameters for paginated trade history.
#[derive(Debug)]
pub struct GetTradesParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    /// Filter by taker side: `"A"` (sell) or `"B"` (buy).
    pub side: Option<String>,
}

/// Access to trade/fill endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct TradesResource {
    http: HttpClient,
    prefix: String,
}

impl TradesResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated historical trades.
    pub async fn list(
        &self,
        symbol: &str,
        params: GetTradesParams,
    ) -> Result<CursorResponse<Vec<Trade>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(s) = &params.side {
            qp.push(("side", s.clone()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/trades/{}", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get recent trades.
    ///
    /// Only available on Lighter.xyz (`/v1/lighter`) and HIP-3
    /// (`/v1/hyperliquid/hip3`). The Hyperliquid base namespace
    /// (`/v1/hyperliquid`) does **not** expose a `/recent` endpoint;
    /// calling `client.hyperliquid.trades.recent(...)` returns
    /// [`Error::InvalidParam`] without a network round-trip. Use
    /// [`TradesResource::list`] with a time range instead.
    pub async fn recent(&self, symbol: &str, limit: Option<i64>) -> Result<Vec<Trade>> {
        // Reject the Hyperliquid base prefix: backend only exposes /recent
        // for HIP-3 (`/v1/hyperliquid/hip3`) and Lighter (`/v1/lighter`).
        // Match exactly on `/v1/hyperliquid` to avoid catching the hip3
        // nested prefix.
        if self.prefix == "/v1/hyperliquid" {
            return Err(Error::InvalidParam(
                "trades.recent() is not available on Hyperliquid; use trades.list() with a time range".to_string(),
            ));
        }
        let mut qp = vec![];
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        self.http
            .get(&format!("{}/trades/{}/recent", self.prefix, symbol), &qp)
            .await
    }
}
