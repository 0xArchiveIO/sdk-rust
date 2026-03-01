use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, Liquidation, LiquidationVolume, Timestamp};

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
        coin: &str,
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
            .get_with_cursor(&format!("{}/liquidations/{}", self.prefix, coin), &qp)
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
                &format!("{}/liquidations/{}", self.prefix, user_address),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get aggregated liquidation volume by time bucket.
    pub async fn volume(
        &self,
        coin: &str,
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
                &format!("{}/liquidations/{}/volume", self.prefix, coin),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
