use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{
    CursorResponse, Liquidation, LiquidationLevels, LiquidationLevelsHistoryItem,
    LiquidationVolume, Timestamp,
};

/// Parameters for paginated liquidation history.
#[derive(Debug)]
pub struct LiquidationHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for liquidations-by-user queries.
#[derive(Debug)]
pub struct LiquidationsByUserParams {
    pub start: Timestamp,
    pub end: Timestamp,
    /// Optional coin filter.
    pub coin: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for aggregated liquidation volume.
#[derive(Debug)]
pub struct LiquidationVolumeParams {
    pub start: Timestamp,
    pub end: Timestamp,
    /// Bucket interval, e.g. `"1h"`, `"1d"`.
    pub interval: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for current liquidation levels.
#[derive(Debug, Default)]
pub struct LiquidationLevelsParams {
    /// Percentage range around the mark price (1-50, default 10).
    pub range_pct: Option<f64>,
    /// Number of price buckets (10-200, default 50).
    pub buckets: Option<u32>,
    /// Side filter (`bid`/`buy`/`B` keeps longs, `ask`/`sell`/`A` keeps shorts).
    pub side: Option<String>,
    /// Point-in-time read: epoch ms. Serves the newest snapshot at or before
    /// this instant. History begins 2026-07-27.
    pub at: Option<i64>,
}

/// Parameters for level history endpoints (liquidation + trigger levels).
#[derive(Debug, Default)]
pub struct LevelsHistoryParams {
    /// Range start, epoch ms inclusive. Default: 24h before `end`.
    pub start: Option<Timestamp>,
    /// Range end, epoch ms inclusive. Default: now.
    pub end: Option<Timestamp>,
    /// Cursor from the previous page's `next_cursor` (exclusive).
    pub cursor: Option<String>,
    /// Snapshots per page (1-100, default 24).
    pub limit: Option<i64>,
    /// When `true`, items omit the levels array (cheap snapshot discovery).
    pub summary: Option<bool>,
    /// Percentage range around each snapshot's mid (1-50, default 10).
    pub range_pct: Option<f64>,
    /// Number of price buckets (10-200, default 50).
    pub buckets: Option<u32>,
    /// Side filter; the other side is zeroed.
    pub side: Option<String>,
}

impl LevelsHistoryParams {
    pub(crate) fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut qp = vec![];
        if let Some(ref s) = self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(ref e) = self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(ref c) = self.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(s) = self.summary {
            qp.push(("summary", s.to_string()));
        }
        if let Some(r) = self.range_pct {
            qp.push(("range_pct", r.to_string()));
        }
        if let Some(b) = self.buckets {
            qp.push(("buckets", b.to_string()));
        }
        if let Some(ref s) = self.side {
            qp.push(("side", s.clone()));
        }
        qp
    }
}

/// Access to liquidation endpoints (Hyperliquid only).
#[derive(Debug, Clone)]
pub struct LiquidationsResource {
    http: HttpClient,
    prefix: String,
}

impl LiquidationsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated historical liquidations for a coin.
    pub async fn history(
        &self,
        symbol: &str,
        params: LiquidationHistoryParams,
    ) -> Result<CursorResponse<Vec<Liquidation>>> {
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
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/liquidations/{}", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get paginated liquidations for a specific user address.
    pub async fn by_user(
        &self,
        user_address: &str,
        params: LiquidationsByUserParams,
    ) -> Result<CursorResponse<Vec<Liquidation>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = &params.coin {
            qp.push(("coin", c.clone()));
        }
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/liquidations/user/{}", self.prefix, user_address),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get aggregated liquidation volume by time bucket.
    pub async fn volume(
        &self,
        symbol: &str,
        params: LiquidationVolumeParams,
    ) -> Result<CursorResponse<Vec<LiquidationVolume>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(i) = &params.interval {
            qp.push(("interval", i.clone()));
        }
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/liquidations/{}/volume", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get projected forced-liquidation levels for a symbol.
    ///
    /// Computed from clearinghouse positions and margin state, bucketed
    /// around the snapshot mark price. Snapshots refresh approximately every
    /// five minutes; set `params.at` (epoch ms) for a point-in-time read. History
    /// begins 2026-07-27.
    ///
    /// These are projected forced liquidations, not the pending trigger-order
    /// map (see `orders().trigger_levels` for that).
    pub async fn levels(
        &self,
        symbol: &str,
        params: LiquidationLevelsParams,
    ) -> Result<LiquidationLevels> {
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
        if let Some(at) = params.at {
            qp.push(("at", at.to_string()));
        }
        self.http
            .get(&format!("{}/liquidations/{}/levels", self.prefix, symbol), &qp)
            .await
    }

    /// Get historical liquidation-levels snapshots with cursor pagination.
    ///
    /// Ascending by snapshot time (approximately every five minutes, retained from
    /// 2026-07-27). Set `params.summary = Some(true)` to list snapshots
    /// without histograms.
    pub async fn levels_history(
        &self,
        symbol: &str,
        params: LevelsHistoryParams,
    ) -> Result<CursorResponse<Vec<LiquidationLevelsHistoryItem>>> {
        let qp = params.to_query();
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/liquidations/{}/levels/history", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
