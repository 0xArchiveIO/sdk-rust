use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, MetaResponse, Timestamp, Trade, TradeSide};

/// Parameters for paginated trade history.
#[derive(Debug)]
pub struct GetTradesParams {
    pub start: Timestamp,
    pub end: Timestamp,
    /// The previous page's `next_cursor`, passed through unchanged.
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    /// Keep only buy-side (`side` `"B"`) or sell-side (`"A"`) rows. The
    /// filter applies before paging, so a full page still holds `limit`
    /// matching rows; keep it unchanged while paging.
    pub side: Option<TradeSide>,
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
        if let Some(s) = self.side {
            qp.push(("side", s.as_str().to_string()));
        }
        qp
    }
}

/// Parameters for the most recent trades, from
/// [`TradesResource::recent_with`] and `hip4.get_trades_recent_with()`.
#[derive(Debug, Default, Clone)]
pub struct RecentTradesParams {
    /// Number of trades to return.
    pub limit: Option<i64>,
    /// Keep only buy-side (`side` `"B"`) or sell-side (`"A"`) rows.
    pub side: Option<TradeSide>,
}

impl RecentTradesParams {
    pub(crate) fn query(&self) -> Vec<(&'static str, String)> {
        let mut qp = vec![];
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(s) = self.side {
            qp.push(("side", s.as_str().to_string()));
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
    /// Page with `next_cursor` while `has_more` is `true`. The response's
    /// `meta` carries the canonical `symbol` and the `venue`.
    ///
    /// On both Lighter deployments the server clamps `end` to the
    /// finalization boundary, so a range that reaches past it ends early with
    /// `has_more` `false`; `meta.finalized_through`, `meta.requested_end` and
    /// `meta.clamped_to` say where and why.
    pub async fn list(
        &self,
        symbol: &str,
        params: GetTradesParams,
    ) -> Result<CursorResponse<Vec<Trade>>> {
        self.http
            .get_with_cursor(
                &format!("{}/trades/{}", self.prefix, symbol),
                &params.query(),
            )
            .await
    }

    /// Alias of [`TradesResource::list`], named for the `history` verb that
    /// paged series use.
    pub async fn history(
        &self,
        symbol: &str,
        params: GetTradesParams,
    ) -> Result<CursorResponse<Vec<Trade>>> {
        self.list(symbol, params).await
    }

    /// Get paginated historical trades with the response's full `meta` block.
    ///
    /// Sends the same request as [`TradesResource::list`] and returns the
    /// same value, since every paged response now carries `meta`. On both
    /// Lighter deployments `list` serves final trades only:
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
        self.list(symbol, params).await
    }

    /// Get recent trades.
    ///
    /// Available on HIP-3 (`/v1/hyperliquid/hip3`), Hyperliquid Spot and both
    /// Lighter deployments (`/v1/lighter` and `/v1/rh-lighter`), where it
    /// serves the preliminary tier. The Hyperliquid base namespace
    /// (`/v1/hyperliquid`) does **not** expose a `/recent` endpoint;
    /// calling `client.hyperliquid.trades.recent(...)` returns
    /// [`Error::InvalidParam`] without a network round-trip. Use
    /// [`TradesResource::list`] with a time range instead.
    ///
    /// To filter by side, use [`TradesResource::recent_with`].
    pub async fn recent(&self, symbol: &str, limit: Option<i64>) -> Result<Vec<Trade>> {
        Ok(self
            .recent_with(symbol, RecentTradesParams { limit, side: None })
            .await?
            .data)
    }

    /// Get recent trades with a side filter, and the response's full `meta`
    /// block.
    ///
    /// Same venues and rules as [`TradesResource::recent`]. On both Lighter
    /// deployments `meta.preliminary_row_count` is the number of rows that are
    /// not final yet and `meta.finalized_through` is the boundary.
    ///
    /// ```no_run
    /// # use oxarchive::OxArchive;
    /// # use oxarchive::resources::trades::RecentTradesParams;
    /// # use oxarchive::types::TradeSide;
    /// # async fn example() -> oxarchive::Result<()> {
    /// # let client = OxArchive::new("key")?;
    /// let sells = client.hyperliquid.hip3.trades.recent_with("xyz:XYZ100", RecentTradesParams {
    ///     limit: Some(100),
    ///     side: Some(TradeSide::Sell),
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn recent_with(
        &self,
        symbol: &str,
        params: RecentTradesParams,
    ) -> Result<MetaResponse<Vec<Trade>>> {
        self.check_recent()?;
        let (data, meta) = self
            .http
            .get_with_meta(
                &format!("{}/trades/{}/recent", self.prefix, symbol),
                &params.query(),
            )
            .await?;
        Ok(MetaResponse::new(data, meta))
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
        self.recent_with(symbol, RecentTradesParams { limit, side: None })
            .await
    }

    fn check_recent(&self) -> Result<()> {
        // Reject the Hyperliquid base prefix: backend only exposes /recent
        // for HIP-3 (`/v1/hyperliquid/hip3`), Spot and Lighter. Match exactly
        // on `/v1/hyperliquid` to avoid catching the nested prefixes.
        if self.prefix == "/v1/hyperliquid" {
            return Err(Error::InvalidParam(
                "trades.recent() is not available on Hyperliquid; use trades.list() with a time range".to_string(),
            ));
        }
        Ok(())
    }
}
