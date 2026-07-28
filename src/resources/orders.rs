use crate::error::Result;
use crate::http::HttpClient;
use crate::resources::liquidations::LevelsHistoryParams;
use crate::types::{
    CursorResponse, OrderHistoryEntry, Timestamp, TriggerLevels, TriggerLevelsHistoryItem,
};

/// Parameters for paginated order history.
#[derive(Debug, Default)]
pub struct OrderHistoryParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub user: Option<String>,
    pub status: Option<String>,
    pub order_type: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for order flow aggregation.
#[derive(Debug, Default)]
pub struct OrderFlowParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub interval: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for the current trigger-levels map.
#[derive(Debug, Default)]
pub struct TriggerLevelsParams {
    /// Percentage range around the mid price (1-50, default 10).
    pub range_pct: Option<f64>,
    /// Number of price buckets (10-200, default 50).
    pub buckets: Option<u32>,
    /// Side filter (`bid`/`buy`/`B` keeps bids, `ask`/`sell`/`A` keeps asks).
    pub side: Option<String>,
}

/// Parameters for TP/SL order queries.
#[derive(Debug, Default)]
pub struct TpslParams {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
    pub user: Option<String>,
    pub triggered: Option<bool>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Access to order history, flow, and TP/SL endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct OrdersResource {
    http: HttpClient,
    prefix: String,
}

impl OrdersResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated order history for a symbol.
    pub async fn history(
        &self,
        symbol: &str,
        params: OrderHistoryParams,
    ) -> Result<CursorResponse<Vec<OrderHistoryEntry>>> {
        let mut qp = vec![];
        if let Some(ref s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(ref e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(ref u) = params.user {
            qp.push(("user", u.clone()));
        }
        if let Some(ref s) = params.status {
            qp.push(("status", s.clone()));
        }
        if let Some(ref t) = params.order_type {
            qp.push(("order_type", t.clone()));
        }
        if let Some(ref c) = params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/orders/{}/history", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get order flow aggregation for a symbol.
    pub async fn flow(
        &self,
        symbol: &str,
        params: OrderFlowParams,
    ) -> Result<CursorResponse<Vec<serde_json::Value>>> {
        let mut qp = vec![];
        if let Some(ref s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(ref e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(ref i) = params.interval {
            qp.push(("interval", i.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/orders/{}/flow", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get TP/SL (take-profit / stop-loss) orders for a symbol.
    pub async fn tpsl(
        &self,
        symbol: &str,
        params: TpslParams,
    ) -> Result<CursorResponse<Vec<serde_json::Value>>> {
        let mut qp = vec![];
        if let Some(ref s) = params.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(ref e) = params.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(ref u) = params.user {
            qp.push(("user", u.clone()));
        }
        if let Some(t) = params.triggered {
            qp.push(("triggered", t.to_string()));
        }
        if let Some(ref c) = params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/orders/{}/tpsl", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get the pending trigger-order map for a symbol.
    ///
    /// Currently pending stop-loss and take-profit trigger orders grouped
    /// into price buckets near the current mid/mark price. Voluntary trigger
    /// orders, not projected forced liquidations (see the liquidations
    /// resource's `levels` for those). The response's `as_of` is the server
    /// read time.
    pub async fn trigger_levels(
        &self,
        symbol: &str,
        params: TriggerLevelsParams,
    ) -> Result<TriggerLevels> {
        let mut qp = vec![];
        if let Some(r) = params.range_pct {
            qp.push(("range_pct", r.to_string()));
        }
        if let Some(b) = params.buckets {
            qp.push(("buckets", b.to_string()));
        }
        if let Some(ref s) = params.side {
            qp.push(("side", s.clone()));
        }
        self.http
            .get(
                &format!("{}/orders/{}/trigger-levels", self.prefix, symbol),
                &qp,
            )
            .await
    }

    /// Get historical trigger-levels snapshots with cursor pagination.
    ///
    /// Ascending by snapshot time (15-minute cadence, retained from
    /// 2026-07-27). Set `params.summary = Some(true)` to list snapshots
    /// without histograms.
    pub async fn trigger_levels_history(
        &self,
        symbol: &str,
        params: LevelsHistoryParams,
    ) -> Result<CursorResponse<Vec<TriggerLevelsHistoryItem>>> {
        let qp = params.to_query();
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/orders/{}/trigger-levels/history", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
