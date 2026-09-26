use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, MetaResponse, Timestamp, Trade};

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

impl GetTradesParams {
    fn query(&self) -> Vec<(&'static str, String)> {
        let mut qp = vec![
            ("start", self.start.to_millis().to_string()),
            ("end", self.end.to_millis().to_string()),
        ];
        if let Some(c) = &self.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(s) = &self.side {
            qp.push(("side", s.clone()));
        }
        qp
    }
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
    ///
    /// On both Lighter deployments the server clamps `end` to the
    /// finalization boundary, so a range that reaches past it ends early with
    /// `next_cursor` `None`. Use [`TradesResource::list_with_meta`] to see
    /// the boundary and whether the range was clamped.
    pub async fn list(
        &self,
        symbol: &str,
        params: GetTradesParams,
    ) -> Result<CursorResponse<Vec<Trade>>> {
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/trades/{}", self.prefix, symbol), &params.query())
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get paginated historical trades with the response's full `meta` block.
    ///
    /// Sends the same request as [`TradesResource::list`]. On both Lighter
    /// deployments `list` serves final trades only:
    ///
    /// - `meta.finalized_through` is the finalization boundary, about a day
    ///   behind. Every trade before it is final.
    /// - When the requested `end` was past the boundary, the server clamps the
    ///   range to it: `meta.requested_end` holds the `end` you sent and
    ///   `meta.clamped_to` the boundary. Trades after it are in
    ///   [`TradesResource::recent_with_meta`] until they become final.
    ///
    /// Other venues do not send these fields, so they are `None` there.
    pub async fn list_with_meta(
        &self,
        symbol: &str,
        params: GetTradesParams,
    ) -> Result<MetaResponse<Vec<Trade>>> {
        let (data, meta) = self
            .http
            .get_with_meta(&format!("{}/trades/{}", self.prefix, symbol), &params.query())
            .await?;
        Ok(MetaResponse::new(data, meta))
    }

    /// Get recent trades.
    ///
    /// Only available on Lighter.xyz (`/v1/lighter` and `/v1/rh-lighter`),
    /// where it serves the preliminary tier, and HIP-3
    /// (`/v1/hyperliquid/hip3`). The Hyperliquid base namespace
    /// (`/v1/hyperliquid`) does **not** expose a `/recent` endpoint;
    /// calling `client.hyperliquid.trades.recent(...)` returns
    /// [`Error::InvalidParam`] without a network round-trip. Use
    /// [`TradesResource::list`] with a time range instead.
    pub async fn recent(&self, symbol: &str, limit: Option<i64>) -> Result<Vec<Trade>> {
        let qp = self.recent_query(limit)?;
        self.http
            .get(&format!("{}/trades/{}/recent", self.prefix, symbol), &qp)
            .await
    }

    /// Get recent trades with the response's full `meta` block.
    ///
    /// Sends the same request as [`TradesResource::recent`], with the same
    /// venue rules. On both Lighter deployments the rows can be newer than
    /// the finalization boundary: `meta.preliminary_row_count` is the number
    /// of rows in this response that are not final yet, and
    /// `meta.finalized_through` is the boundary.
    pub async fn recent_with_meta(
        &self,
        symbol: &str,
        limit: Option<i64>,
    ) -> Result<MetaResponse<Vec<Trade>>> {
        let qp = self.recent_query(limit)?;
        let (data, meta) = self
            .http
            .get_with_meta(&format!("{}/trades/{}/recent", self.prefix, symbol), &qp)
            .await?;
        Ok(MetaResponse::new(data, meta))
    }

    fn recent_query(&self, limit: Option<i64>) -> Result<Vec<(&'static str, String)>> {
        // Reject the Hyperliquid base prefix: backend only exposes /recent
        // for HIP-3 (`/v1/hyperliquid/hip3`), Spot and Lighter. Match exactly
        // on `/v1/hyperliquid` to avoid catching the nested prefixes.
        if self.prefix == "/v1/hyperliquid" {
            return Err(Error::InvalidParam(
                "trades.recent() is not available on Hyperliquid; use trades.list() with a time range".to_string(),
            ));
        }
        let mut qp = vec![];
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        Ok(qp)
    }
}
