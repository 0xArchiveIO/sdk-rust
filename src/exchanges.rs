/// Exchange-specific client types that group resources under a common API
/// prefix (e.g. `/v1/hyperliquid`, `/v1/lighter`).

use crate::error::Result;
use crate::http::HttpClient;
use crate::resources::{
    CandlesResource, FundingResource, Hip3InstrumentsResource, InstrumentsResource,
    L3OrderBookResource, L4OrderBookResource, LighterInstrumentsResource, LiquidationsResource,
    OpenInterestResource, OrderBookResource, OrdersResource, TradesResource,
};
use crate::types::{
    CoinFreshness, CoinSummary, CursorResponse, PriceSnapshot, Timestamp,
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
    pub candles: CandlesResource,
    pub liquidations: LiquidationsResource,
    pub orders: OrdersResource,
    pub l4_orderbook: L4OrderBookResource,
    pub hip3: Hip3Client,
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
            candles: CandlesResource::new(http.clone(), prefix),
            liquidations: LiquidationsResource::new(http.clone(), prefix),
            orders: OrdersResource::new(http.clone(), prefix),
            l4_orderbook: L4OrderBookResource::new(http.clone(), prefix),
            hip3: Hip3Client::new(http.clone()),
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
    pub candles: CandlesResource,
    pub liquidations: LiquidationsResource,
    pub orders: OrdersResource,
    pub l4_orderbook: L4OrderBookResource,
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
            candles: CandlesResource::new(http.clone(), prefix),
            liquidations: LiquidationsResource::new(http.clone(), prefix),
            orders: OrdersResource::new(http.clone(), prefix),
            l4_orderbook: L4OrderBookResource::new(http.clone(), prefix),
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
// Lighter.xyz
// ---------------------------------------------------------------------------

/// Client for Lighter.xyz endpoints (`/v1/lighter`).
#[derive(Debug, Clone)]
pub struct LighterClient {
    http: HttpClient,
    pub orderbook: OrderBookResource,
    pub trades: TradesResource,
    pub instruments: LighterInstrumentsResource,
    pub funding: FundingResource,
    pub open_interest: OpenInterestResource,
    pub candles: CandlesResource,
    pub l3_orderbook: L3OrderBookResource,
}

impl LighterClient {
    pub(crate) fn new(http: HttpClient) -> Self {
        let prefix = "/v1/lighter";
        Self {
            orderbook: OrderBookResource::new(http.clone(), prefix),
            trades: TradesResource::new(http.clone(), prefix),
            instruments: LighterInstrumentsResource::new(http.clone(), prefix),
            funding: FundingResource::new(http.clone(), prefix),
            open_interest: OpenInterestResource::new(http.clone(), prefix),
            candles: CandlesResource::new(http.clone(), prefix),
            l3_orderbook: L3OrderBookResource::new(http.clone(), prefix),
            http,
        }
    }

    /// Get data freshness for a Lighter symbol.
    pub async fn freshness(&self, symbol: &str) -> Result<CoinFreshness> {
        self.http
            .get(&format!("/v1/lighter/freshness/{}", symbol), &[])
            .await
    }

    /// Get a combined market summary for a Lighter symbol.
    pub async fn summary(&self, symbol: &str) -> Result<CoinSummary> {
        self.http
            .get(&format!("/v1/lighter/summary/{}", symbol), &[])
            .await
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
            .get_with_cursor(&format!("/v1/lighter/prices/{}", symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
