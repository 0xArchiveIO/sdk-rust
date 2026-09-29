use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, Timestamp};

/// Parameters for paginated L3 orderbook history.
#[derive(Debug)]
pub struct L3HistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    /// Keep only the resting orders of this Lighter account index.
    pub account: Option<i64>,
}

/// Parameters for [`L3OrderBookResource::get_with_params`].
#[derive(Debug, Default, Clone)]
pub struct L3OrderBookParams {
    /// Return the snapshot at or before this time instead of the latest.
    pub timestamp: Option<Timestamp>,
    /// Keep only the resting orders of this Lighter account index.
    pub account: Option<i64>,
    /// Resting orders per side, 1 to 250.
    pub depth: Option<i64>,
}

/// Access to L3 (order-level) orderbook endpoints (Lighter.xyz only).
#[derive(Debug, Clone)]
pub struct L3OrderBookResource {
    http: HttpClient,
    prefix: String,
}

impl L3OrderBookResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the current L3 orderbook for a symbol.
    ///
    /// `depth` caps individual resting orders per side and must be 1..=250.
    /// Use [`get_with_params`](Self::get_with_params) for a point-in-time
    /// snapshot or one account's orders.
    pub async fn get(&self, symbol: &str, depth: Option<i64>) -> Result<serde_json::Value> {
        self.get_with_params(
            symbol,
            L3OrderBookParams {
                depth,
                ..Default::default()
            },
        )
        .await
    }

    /// Get an L3 orderbook snapshot: the latest, or the one at `timestamp`,
    /// optionally filtered to one account's resting orders.
    pub async fn get_with_params(
        &self,
        symbol: &str,
        params: L3OrderBookParams,
    ) -> Result<serde_json::Value> {
        let mut qp = vec![];
        if let Some(ts) = &params.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(a) = params.account {
            qp.push(("account", a.to_string()));
        }
        if let Some(d) = params.depth {
            if !(1..=250).contains(&d) {
                return Err(Error::InvalidParam(
                    "depth must be between 1 and 250 orders per side".to_string(),
                ));
            }
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(&format!("{}/l3orderbook/{}", self.prefix, symbol), &qp)
            .await
    }

    /// Get paginated L3 orderbook history for a symbol.
    pub async fn history(
        &self,
        symbol: &str,
        params: L3HistoryParams,
    ) -> Result<CursorResponse<Vec<serde_json::Value>>> {
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
        if let Some(a) = params.account {
            qp.push(("account", a.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/l3orderbook/{}/history", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
