//! Exchange-specific client types that group resources under a common API
//! prefix (e.g. `/v1/hyperliquid`, `/v1/lighter`, `/v1/rh-lighter`).

use crate::error::Result;
use crate::http::HttpClient;
use crate::resources::{
    BreadthResource, CandlesResource, FundingResource, Hip3InstrumentsResource,
    Hip4InstrumentsResource, InstrumentsResource, L2OrderBookResource, L3OrderBookResource,
    L4OrderBookResource, LighterAccountsResource, LighterInstrumentsResource,
    LighterLiquidationsResource, LighterPositionsResource, LiquidationsResource,
    OpenInterestResource, OrderBookResource, OrdersResource, PositionsResource, SpotPairsResource,
    SpotTwapResource, TradesResource,
};
use crate::types::{
    CoinFreshness, CoinSummary, CursorResponse, Hip4OpenInterestRecord, Hip4Outcome,
    Hip4OutcomeAggregate, L4DiffEntry, L4OrderBookSnapshot, OrderBook, OrderHistoryEntry,
    PriceSnapshot, Timestamp, Trade,
};

// ---------------------------------------------------------------------------
// Hyperliquid
// ---------------------------------------------------------------------------

/// Client for Hyperliquid endpoints (`/v1/hyperliquid`).
#[derive(Debug, Clone)]
pub struct HyperliquidClient {
    http: HttpClient,
    pub orderbook: OrderBookResource,
    pub trades: TradesResource,
    pub instruments: InstrumentsResource,
    pub funding: FundingResource,
    pub open_interest: OpenInterestResource,
    /// OHLCV candle history (maximum 10,000 rows per page).
    pub candles: CandlesResource,
    pub liquidations: LiquidationsResource,
    pub orders: OrdersResource,
    pub l4_orderbook: L4OrderBookResource,
    pub l2_orderbook: L2OrderBookResource,
    /// Account positions by wallet address, market listings and summaries.
    pub positions: PositionsResource,
    pub hip3: Hip3Client,
    /// HIP-4 outcome markets (binary outcome perps, `#`-prefixed coins).
    pub hip4: Hip4,
    /// Hyperliquid Spot markets (dashed canonical pairs, `HYPE-USDC`, `PURR-USDC`).
    pub spot: SpotClient,
}

impl HyperliquidClient {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = "/v1/hyperliquid";
        Self {
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            instruments: InstrumentsResource::new(http.clone(), prefix),
            funding: FundingResource::new(http.clone(), prefix),
            open_interest: OpenInterestResource::new(http.clone(), prefix),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                prefix,
                |symbol| symbol.to_string(),
                Some(10_000),
            ),
            liquidations: LiquidationsResource::new(http.clone(), prefix),
            orders: OrdersResource::new(http.clone(), prefix),
            l4_orderbook: L4OrderBookResource::new(http.clone(), prefix),
            l2_orderbook: L2OrderBookResource::new(http.clone(), prefix),
            positions: PositionsResource::new(http.clone(), prefix),
            hip3: Hip3Client::new(http.clone()),
            hip4: Hip4::new(http.clone()),
            spot: SpotClient::new(http.clone()),
            http,
        }
    }

    /// Get data freshness (lag) for a symbol across all data types.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        self.http
            .get(&format!("/v1/hyperliquid/freshness/{}", symbol), &[])
            .await
    }

    /// Get a combined market summary for a symbol.
    pub async fn summary(&self, symbol: &str) -> Result<CoinSummary> {
        self.http
            .get(&format!("/v1/hyperliquid/summary/{}", symbol), &[])
            .await
    }

    /// Get historical price snapshots for a symbol.
    pub async fn price_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        interval: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
        let mut qp = vec![
            ("start", start.into().to_millis().to_string()),
            ("end", end.into().to_millis().to_string()),
        ];
        if let Some(i) = interval {
            qp.push(("interval", i.to_string()));
        }
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(c) = cursor {
            qp.push(("cursor", c.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("/v1/hyperliquid/prices/{}", symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}

// ---------------------------------------------------------------------------
// HIP-3 (builder-deployed perps, nested under Hyperliquid)
// ---------------------------------------------------------------------------

/// Client for HIP-3 builder-deployed perp endpoints (`/v1/hyperliquid/hip3`).
///
/// HIP-3 coin symbols are **case-sensitive** (e.g. `km:US500`, `xyz:XYZ100`).
#[derive(Debug, Clone)]
pub struct Hip3Client {
    http: HttpClient,
    pub orderbook: OrderBookResource,
    pub trades: TradesResource,
    pub instruments: Hip3InstrumentsResource,
    pub funding: FundingResource,
    pub open_interest: OpenInterestResource,
    /// OHLCV candle history (maximum 10,000 rows per page).
    pub candles: CandlesResource,
    pub liquidations: LiquidationsResource,
    pub orders: OrdersResource,
    /// Percent of eligible instruments above their current UTC-session VWAP.
    pub breadth: BreadthResource,
    pub l4_orderbook: L4OrderBookResource,
    pub l2_orderbook: L2OrderBookResource,
    /// Account positions by wallet address (optional `dex` filter), market
    /// listings and summaries.
    pub positions: PositionsResource,
}

impl Hip3Client {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = "/v1/hyperliquid/hip3";
        Self {
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            instruments: Hip3InstrumentsResource::new(http.clone(), prefix),
            funding: FundingResource::new(http.clone(), prefix),
            open_interest: OpenInterestResource::new(http.clone(), prefix),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                prefix,
                |symbol| symbol.to_string(),
                Some(10_000),
            ),
            liquidations: LiquidationsResource::new(http.clone(), prefix),
            orders: OrdersResource::new(http.clone(), prefix),
            breadth: BreadthResource::new(http.clone(), prefix),
            l4_orderbook: L4OrderBookResource::new(http.clone(), prefix),
            l2_orderbook: L2OrderBookResource::new(http.clone(), prefix),
            positions: PositionsResource::new(http.clone(), prefix),
            http,
        }
    }

    /// Get data freshness for a HIP-3 symbol.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        self.http
            .get(&format!("/v1/hyperliquid/hip3/freshness/{}", symbol), &[])
            .await
    }

    /// Get a combined market summary for a HIP-3 symbol.
    pub async fn summary(&self, symbol: &str) -> Result<CoinSummary> {
        self.http
            .get(&format!("/v1/hyperliquid/hip3/summary/{}", symbol), &[])
            .await
    }

    /// Get historical price snapshots for a HIP-3 symbol.
    pub async fn price_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        interval: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
        let mut qp = vec![
            ("start", start.into().to_millis().to_string()),
            ("end", end.into().to_millis().to_string()),
        ];
        if let Some(i) = interval {
            qp.push(("interval", i.to_string()));
        }
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(c) = cursor {
            qp.push(("cursor", c.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("/v1/hyperliquid/hip3/prices/{}", symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}

// ---------------------------------------------------------------------------
// Hyperliquid Spot (nested under Hyperliquid)
// ---------------------------------------------------------------------------

/// Client for Hyperliquid Spot endpoints (`/v1/hyperliquid/spot`).
///
/// Spot symbols use the dashed canonical form (e.g. `HYPE-USDC`, `PURR-USDC`).
/// The server resolves the dashed form to the wire format (`PURR/USDC`, `@107`)
/// internally.
///
/// Spot has **no funding, no open interest, and no liquidations**.
/// Trade history backfills to 2025-03-22; candle history starts exactly at
/// 2025-03-22T10:50:22Z. Orderbook, L4, TWAP, and freshness data are live-only
/// from May 5, 2026.
#[derive(Debug, Clone)]
pub struct SpotClient {
    http: HttpClient,
    /// Pair discovery (`/pairs`, `/pairs/{symbol}`).
    pub pairs: SpotPairsResource,
    /// L2 orderbook (current and history).
    pub orderbook: OrderBookResource,
    /// Trades (history + recent).
    pub trades: TradesResource,
    /// OHLCV candle history (coverage starts 2025-03-22T10:50:22Z; max 1,000 rows per page).
    pub candles: CandlesResource,
    /// L4 orderbook (snapshot, diffs, checkpoint history).
    pub l4_orderbook: L4OrderBookResource,
    /// Order lifecycle events.
    pub orders: OrdersResource,
    /// TWAP execution statuses (by symbol or by user).
    pub twap: SpotTwapResource,
}

impl SpotClient {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = "/v1/hyperliquid/spot";
        Self {
            pairs: SpotPairsResource::new(http.clone(), prefix),
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                prefix,
                |symbol| symbol.to_string(),
                Some(1000),
            ),
            l4_orderbook: L4OrderBookResource::new(http.clone(), prefix),
            orders: OrdersResource::new(http.clone(), prefix),
            twap: SpotTwapResource::new(http.clone(), prefix),
            http,
        }
    }

    /// Get data freshness (lag) for a spot symbol across all data types.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        self.http
            .get(
                &format!("/v1/hyperliquid/spot/freshness/{}", symbol),
                &[],
            )
            .await
    }
}

// ---------------------------------------------------------------------------
// HIP-4 (outcome markets, nested under Hyperliquid)
// ---------------------------------------------------------------------------

const HIP4_PREFIX: &str = "/v1/hyperliquid/hip4";

/// Build the wire-level path segment for a HIP-4 coin symbol.
///
/// The primary user-facing path form is the bare numeric value (`"0"`,
/// `"1"`, ...), which is sent unchanged. Legacy `#`-prefixed inputs such as
/// `"#0"` remain accepted and are percent-encoded to `%230` for URL
/// transport because raw `#` is the fragment delimiter per RFC 3986.
/// Response `coin` / `symbol` fields retain the backend's `#`-prefixed form.
fn hip4_encode(symbol: &str) -> String {
    urlencoding::encode(symbol).into_owned()
}

/// Optional filters for [`Hip4::list_outcomes`].
#[derive(Debug, Default, Clone)]
pub struct Hip4ListOutcomesParams {
    /// Filter by settlement state. Server default returns both.
    pub is_settled: Option<bool>,
    /// Server-side slug filter. Matches the per-outcome OR per-side slug
    /// (e.g. `"btc-above-78213-may-04-0600"`). When set, the response is
    /// at most one item, still subject to `is_settled` if provided.
    pub slug: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Optional point-in-time / depth controls for orderbook reads.
#[derive(Debug, Default, Clone)]
pub struct Hip4OrderBookParams {
    pub timestamp: Option<Timestamp>,
    pub depth: Option<i32>,
}

/// Range for paginated history endpoints.
#[derive(Debug, Clone)]
pub struct Hip4HistoryRange {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Range + side filter for trade-history pagination.
#[derive(Debug, Clone)]
pub struct Hip4TradesParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    /// `"A"` (sell) or `"B"` (buy).
    pub side: Option<String>,
}

/// Optional depth + range params for L4 history pagination.
#[derive(Debug, Clone)]
pub struct Hip4L4HistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub depth: Option<i32>,
}

/// Filters for order-history pagination.
#[derive(Debug, Default, Clone)]
pub struct Hip4OrderHistoryParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub user: Option<String>,
    pub status: Option<String>,
    pub order_type: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Filters for order-flow aggregation.
#[derive(Debug, Default, Clone)]
pub struct Hip4OrderFlowParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub interval: Option<String>,
    pub limit: Option<i64>,
}

/// Filters for TP/SL queries.
#[derive(Debug, Default, Clone)]
pub struct Hip4TpslParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub user: Option<String>,
    pub triggered: Option<bool>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Client for HIP-4 outcome-market endpoints (`/v1/hyperliquid/hip4`).
///
/// HIP-4 candles and outcome-side open interest are served from May 2, 2026;
/// raw OI updates arrive at roughly 10-second cadence. HIP-4 has no funding
/// rates or liquidations. Response coin symbols are `#`-prefixed (`#0`, `#1`,
/// ...), following `#<10*outcome_id + side>`. Path inputs use the bare
/// numeric form (`"0"`, `"1"`, ...); legacy `#N` inputs remain supported.
/// Candle pages accept at most 1,000 rows.
/// Use the **bare numeric form** (`"0"`) in your code. For compatibility,
/// `"#0"` is also accepted and encoded to `%230` on the wire; do not
/// pre-encode it yourself.
///
/// `mark_price` on HIP-4 endpoints is an implied probability in `[0, 1]`,
/// not a USD price. The field name matches perp/HIP-3 to stay consistent
/// with the Hyperliquid upstream `markPx`.
#[derive(Debug, Clone)]
pub struct Hip4 {
    http: HttpClient,
    /// Per-side instrument resource (`/instruments`, `/instruments/{symbol}`).
    pub instruments: Hip4InstrumentsResource,
    /// Implied-probability OHLCV candles.
    pub candles: CandlesResource,
}

impl Hip4 {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self {
            instruments: Hip4InstrumentsResource::new(http.clone(), HIP4_PREFIX),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                HIP4_PREFIX,
                hip4_encode,
                Some(1000),
            ),
            http,
        }
    }

    // ---- Outcomes (HIP-4-specific, no HIP-3 analog) -----------------------

    /// List outcome markets (per-outcome view, both sides combined).
    pub async fn list_outcomes(
        &self,
        params: Option<Hip4ListOutcomesParams>,
    ) -> Result<CursorResponse<Vec<Hip4OutcomeAggregate>>> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(s) = p.is_settled {
            qp.push(("is_settled", s.to_string()));
        }
        if let Some(s) = p.slug {
            qp.push(("slug", s));
        }
        if let Some(c) = p.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = p.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/outcomes", HIP4_PREFIX), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get a single outcome detail (includes `aggregated_oi`).
    pub async fn get_outcome(&self, outcome_id: i64) -> Result<Hip4OutcomeAggregate> {
        self.http
            .get(&format!("{}/outcomes/{}", HIP4_PREFIX, outcome_id), &[])
            .await
    }

    /// Look up an outcome aggregate by its synthesized slug. Accepts either
    /// the per-outcome slug (`"btc-above-78213-may-04-0600"`) or a per-side
    /// slug (`"btc-above-78213-yes-may-04-0600"`); both resolve to the same
    /// outcome with `aggregated_oi` populated, like `/outcomes/{id}`.
    pub async fn get_outcome_by_slug(&self, slug: &str) -> Result<Hip4OutcomeAggregate> {
        self.http
            .get(
                &format!("{}/outcomes/by-slug/{}", HIP4_PREFIX, urlencoding::encode(slug)),
                &[],
            )
            .await
    }

    // ---- Discovery (mirror of HIP-3) --------------------------------------

    /// List per-side HIP-4 instruments.
    pub async fn get_instruments(&self) -> Result<Vec<Hip4Outcome>> {
        self.instruments.list().await
    }

    /// Get a single per-side instrument by path symbol (e.g. `"0"`; legacy
    /// `"#0"` is also accepted).
    pub async fn get_instrument(&self, symbol: &str) -> Result<Hip4Outcome> {
        self.instruments.get(symbol).await
    }

    // ---- Orderbook (L2) ---------------------------------------------------

    /// Get the current (or point-in-time) L2 orderbook for a HIP-4 coin.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        params: Option<Hip4OrderBookParams>,
    ) -> Result<OrderBook> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(ts) = p.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(d) = p.depth {
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(
                &format!("{}/orderbook/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &qp,
            )
            .await
    }

    /// Get paginated L2 orderbook history.
    pub async fn get_orderbook_history(
        &self,
        symbol: &str,
        params: Hip4HistoryRange,
    ) -> Result<CursorResponse<Vec<OrderBook>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orderbook/{}/history",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    // ---- Trades -----------------------------------------------------------

    /// Get paginated historical trades for a HIP-4 coin.
    pub async fn get_trades(
        &self,
        symbol: &str,
        params: Hip4TradesParams,
    ) -> Result<CursorResponse<Vec<Trade>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(s) = params.side {
            qp.push(("side", s));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/trades/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get the most recent trades for a HIP-4 coin.
    pub async fn get_trades_recent(
        &self,
        symbol: &str,
        limit: Option<i64>,
    ) -> Result<Vec<Trade>> {
        let mut qp = vec![];
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        self.http
            .get(
                &format!(
                    "{}/trades/{}/recent",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await
    }

    // ---- Open interest ----------------------------------------------------

    /// Get paginated per-side open-interest history.
    pub async fn get_open_interest(
        &self,
        symbol: &str,
        params: Hip4HistoryRange,
    ) -> Result<CursorResponse<Vec<Hip4OpenInterestRecord>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/openinterest/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get the latest per-side open-interest snapshot.
    pub async fn get_open_interest_current(
        &self,
        symbol: &str,
    ) -> Result<Hip4OpenInterestRecord> {
        self.http
            .get(
                &format!(
                    "{}/openinterest/{}/current",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &[],
            )
            .await
    }

    // ---- Summary / freshness / prices -------------------------------------

    /// Get the combined market summary for a HIP-4 coin.
    pub async fn get_summary(&self, symbol: &str) -> Result<CoinSummary> {
        self.http
            .get(
                &format!("{}/summary/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &[],
            )
            .await
    }

    /// Get the per-data-type freshness for a HIP-4 coin.
    pub async fn get_freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        self.http
            .get(
                &format!("{}/freshness/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &[],
            )
            .await
    }

    /// Get historical mid-price snapshots for a HIP-4 coin.
    pub async fn get_prices(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        interval: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
        let mut qp = vec![
            ("start", start.into().to_millis().to_string()),
            ("end", end.into().to_millis().to_string()),
        ];
        if let Some(i) = interval {
            qp.push(("interval", i.to_string()));
        }
        if let Some(l) = limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(c) = cursor {
            qp.push(("cursor", c.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/prices/{}", HIP4_PREFIX, hip4_encode(symbol)),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    // ---- Order lifecycle --------------------------------------------------

    /// Get paginated order-lifecycle history.
    pub async fn get_order_history(
        &self,
        symbol: &str,
        params: Hip4OrderHistoryParams,
    ) -> Result<CursorResponse<Vec<OrderHistoryEntry>>> {
        let mut qp = vec![];
        if let Some(s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(u) = params.user {
            qp.push(("user", u));
        }
        if let Some(s) = params.status {
            qp.push(("status", s));
        }
        if let Some(t) = params.order_type {
            qp.push(("order_type", t));
        }
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orders/{}/history",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get aggregated order flow for a HIP-4 coin.
    pub async fn get_order_flow(
        &self,
        symbol: &str,
        params: Hip4OrderFlowParams,
    ) -> Result<CursorResponse<Vec<serde_json::Value>>> {
        let mut qp = vec![];
        if let Some(s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(i) = params.interval {
            qp.push(("interval", i));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orders/{}/flow",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get TP/SL orders for a HIP-4 coin.
    pub async fn get_tpsl(
        &self,
        symbol: &str,
        params: Hip4TpslParams,
    ) -> Result<CursorResponse<Vec<serde_json::Value>>> {
        let mut qp = vec![];
        if let Some(s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(u) = params.user {
            qp.push(("user", u));
        }
        if let Some(t) = params.triggered {
            qp.push(("triggered", t.to_string()));
        }
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orders/{}/tpsl",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    // ---- L4 ---------------------------------------------------------------

    /// Get the current (or point-in-time) L4 orderbook reconstruction.
    pub async fn get_l4_orderbook(
        &self,
        symbol: &str,
        params: Option<Hip4OrderBookParams>,
    ) -> Result<L4OrderBookSnapshot> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(ts) = p.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(d) = p.depth {
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(
                &format!("{}/orderbook/{}/l4", HIP4_PREFIX, hip4_encode(symbol)),
                &qp,
            )
            .await
    }

    /// Get paginated L4 diffs (event stream) for a HIP-4 coin.
    pub async fn get_l4_diffs(
        &self,
        symbol: &str,
        params: Hip4HistoryRange,
    ) -> Result<CursorResponse<Vec<L4DiffEntry>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orderbook/{}/l4/diffs",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get paginated L4 checkpoint history (hard-capped at `limit=10` server-side).
    pub async fn get_l4_history(
        &self,
        symbol: &str,
        params: Hip4L4HistoryParams,
    ) -> Result<CursorResponse<Vec<L4OrderBookSnapshot>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(d) = params.depth {
            qp.push(("depth", d.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!(
                    "{}/orderbook/{}/l4/history",
                    HIP4_PREFIX,
                    hip4_encode(symbol)
                ),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}

#[cfg(test)]
mod hip4_tests {
    use super::*;

    fn make_hip4() -> Hip4 {
        let http = HttpClient::new(crate::http::HttpConfig {
            base_url: "https://example.test".into(),
            api_key: "test-key".into(),
            timeout: crate::http::Duration::from_secs(5),
        })
        .unwrap();
        Hip4::new(http)
    }

    #[test]
    fn percent_encodes_hash_for_wire_only() {
        // Legacy `#N` input is retained for compatibility and encoded because
        // raw `#` is the fragment delimiter (RFC 3986).
        assert_eq!(hip4_encode("#0"), "%230");
        assert_eq!(hip4_encode("#1"), "%231");
        assert_eq!(hip4_encode("#42"), "%2342");
    }

    #[test]
    fn bare_numeric_path_is_primary_with_legacy_hash_compatibility() {
        assert_eq!(hip4_encode("0"), "0");
        assert_eq!(hip4_encode("1"), "1");
        assert_eq!(hip4_encode("#0"), "%230");
    }

    #[test]
    fn orderbook_path_matches_spec() {
        let hip4 = make_hip4();
        let primary_path = format!("{}/orderbook/{}", HIP4_PREFIX, hip4_encode("0"));
        assert_eq!(primary_path, "/v1/hyperliquid/hip4/orderbook/0");
        let legacy_path = format!("{}/orderbook/{}", HIP4_PREFIX, hip4_encode("#0"));
        assert_eq!(legacy_path, "/v1/hyperliquid/hip4/orderbook/%230");
        // Field exists & is constructible without panicking.
        let _ = &hip4.instruments;
    }

    #[test]
    fn outcomes_paths_are_stable() {
        assert_eq!(
            format!("{}/outcomes", HIP4_PREFIX),
            "/v1/hyperliquid/hip4/outcomes"
        );
        assert_eq!(
            format!("{}/outcomes/{}", HIP4_PREFIX, 7_i64),
            "/v1/hyperliquid/hip4/outcomes/7"
        );
    }
}

// ---------------------------------------------------------------------------
// Lighter (mainnet and Robinhood Chain)
// ---------------------------------------------------------------------------

const LIGHTER_PREFIX: &str = "/v1/lighter";
const RH_LIGHTER_PREFIX: &str = "/v1/rh-lighter";

async fn venue_freshness(http: &HttpClient, prefix: &str, symbol: &str) -> Result<CoinFreshness> {
    http.get(&format!("{}/freshness/{}", prefix, symbol), &[]).await
}

async fn venue_summary(http: &HttpClient, prefix: &str, symbol: &str) -> Result<CoinSummary> {
    http.get(&format!("{}/summary/{}", prefix, symbol), &[]).await
}

#[allow(clippy::too_many_arguments)]
async fn venue_price_history(
    http: &HttpClient,
    prefix: &str,
    symbol: &str,
    start: Timestamp,
    end: Timestamp,
    interval: Option<&str>,
    limit: Option<i64>,
    cursor: Option<&str>,
) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
    let mut qp = vec![
        ("start", start.to_millis().to_string()),
        ("end", end.to_millis().to_string()),
    ];
    if let Some(i) = interval {
        qp.push(("interval", i.to_string()));
    }
    if let Some(l) = limit {
        qp.push(("limit", l.to_string()));
    }
    if let Some(c) = cursor {
        qp.push(("cursor", c.to_string()));
    }
    let (data, next_cursor) = http
        .get_with_cursor(&format!("{}/prices/{}", prefix, symbol), &qp)
        .await?;
    Ok(CursorResponse { data, next_cursor })
}

/// Client for Lighter mainnet endpoints (`/v1/lighter`).
///
/// Lighter has two deployments: mainnet (this client) and Robinhood Chain
/// ([`RhLighterClient`], `client.rh_lighter`).
#[derive(Debug, Clone)]
pub struct LighterClient {
    http: HttpClient,
    pub orderbook: OrderBookResource,
    /// Trades. `list` returns final trades only and clamps `end` to the
    /// finalization boundary (about a day behind); `recent` serves the
    /// preliminary tier. `list_with_meta` and `recent_with_meta` also return
    /// the boundary (`meta.finalized_through`), the clamp (`meta.clamped_to`)
    /// and `meta.preliminary_row_count`.
    pub trades: TradesResource,
    pub instruments: LighterInstrumentsResource,
    pub funding: FundingResource,
    pub open_interest: OpenInterestResource,
    /// OHLCV candle history (maximum 10,000 rows per page).
    pub candles: CandlesResource,
    /// Liquidation trades and liquidation volume (from 2026-06-10).
    pub liquidations: LighterLiquidationsResource,
    pub l3_orderbook: L3OrderBookResource,
    /// Account positions by account index, market listings and summaries.
    pub positions: LighterPositionsResource,
    /// Account lookup by L1 address.
    pub accounts: LighterAccountsResource,
}

impl LighterClient {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = LIGHTER_PREFIX;
        Self {
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            instruments: LighterInstrumentsResource::new(http.clone(), prefix),
            funding: FundingResource::new(http.clone(), prefix),
            open_interest: OpenInterestResource::new(http.clone(), prefix),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                prefix,
                |symbol| symbol.to_string(),
                Some(10_000),
            ),
            liquidations: LighterLiquidationsResource::new(http.clone(), prefix),
            l3_orderbook: L3OrderBookResource::new(http.clone(), prefix),
            positions: LighterPositionsResource::new(http.clone(), prefix),
            accounts: LighterAccountsResource::new(http.clone(), prefix),
            http,
        }
    }

    /// Get data freshness for a Lighter symbol.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        venue_freshness(&self.http, LIGHTER_PREFIX, symbol).await
    }

    /// Get a combined market summary for a Lighter symbol.
    pub async fn summary(&self, symbol: &str) -> Result<CoinSummary> {
        venue_summary(&self.http, LIGHTER_PREFIX, symbol).await
    }

    /// Get historical price snapshots for a Lighter symbol.
    pub async fn price_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        interval: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
        venue_price_history(
            &self.http,
            LIGHTER_PREFIX,
            symbol,
            start.into(),
            end.into(),
            interval,
            limit,
            cursor,
        )
        .await
    }
}

/// Client for Lighter on Robinhood Chain (`/v1/rh-lighter`), the second
/// Lighter deployment, available as `client.rh_lighter`.
///
/// The same resources as the mainnet [`LighterClient`] except the L3 order
/// book, which is not captured on this deployment. Markets are quoted in
/// USDG: perps use uppercase symbols (`BTC`) and spot markets use dashed
/// pairs (`AAPL-USDG`). Market ids and symbols are separate from mainnet, so
/// the same symbol can name a different market on each deployment.
///
/// Coverage: trades and liquidations from 2026-06-26 20:10:26 UTC (the venue
/// launch); order book, open interest and funding from
/// 2026-08-22 18:43 UTC; candles from 2026-06-26 once candle history is
/// enabled for this deployment. Trades behave as on mainnet: `list` is final
/// up to the finalization boundary and `recent` is the preliminary tier; read
/// the boundary with `trades.list_with_meta()` (`meta.finalized_through`,
/// `meta.clamped_to`) and `trades.recent_with_meta()`
/// (`meta.preliminary_row_count`).
#[derive(Debug, Clone)]
pub struct RhLighterClient {
    http: HttpClient,
    pub orderbook: OrderBookResource,
    /// Trades. `list` returns final trades only and clamps `end` to the
    /// finalization boundary (about a day behind); `recent` serves the
    /// preliminary tier. `list_with_meta` and `recent_with_meta` also return
    /// the boundary (`meta.finalized_through`), the clamp (`meta.clamped_to`)
    /// and `meta.preliminary_row_count`.
    pub trades: TradesResource,
    pub instruments: LighterInstrumentsResource,
    pub funding: FundingResource,
    pub open_interest: OpenInterestResource,
    /// OHLCV candle history (maximum 10,000 rows per page).
    pub candles: CandlesResource,
    /// Liquidation trades and liquidation volume (from the venue launch,
    /// 2026-06-26 20:10:26 UTC).
    pub liquidations: LighterLiquidationsResource,
    /// Account positions by account index, market listings and summaries.
    pub positions: LighterPositionsResource,
}

impl RhLighterClient {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = RH_LIGHTER_PREFIX;
        Self {
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            instruments: LighterInstrumentsResource::new(http.clone(), prefix),
            funding: FundingResource::new(http.clone(), prefix),
            open_interest: OpenInterestResource::new(http.clone(), prefix),
            candles: CandlesResource::new_with_transform_and_limit(
                http.clone(),
                prefix,
                |symbol| symbol.to_string(),
                Some(10_000),
            ),
            liquidations: LighterLiquidationsResource::new(http.clone(), prefix),
            positions: LighterPositionsResource::new(http.clone(), prefix),
            http,
        }
    }

    /// Get data freshness for a Robinhood Chain symbol.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        venue_freshness(&self.http, RH_LIGHTER_PREFIX, symbol).await
    }

    /// Get a combined market summary for a Robinhood Chain symbol.
    pub async fn summary(&self, symbol: &str) -> Result<CoinSummary> {
        venue_summary(&self.http, RH_LIGHTER_PREFIX, symbol).await
    }

    /// Get historical price snapshots for a Robinhood Chain symbol.
    pub async fn price_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        interval: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<CursorResponse<Vec<PriceSnapshot>>> {
        venue_price_history(
            &self.http,
            RH_LIGHTER_PREFIX,
            symbol,
            start.into(),
            end.into(),
            interval,
            limit,
            cursor,
        )
        .await
    }
}
