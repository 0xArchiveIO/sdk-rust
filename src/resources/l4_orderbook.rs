use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, Timestamp};

/// Parameters for fetching a single L4 orderbook snapshot.
#[derive(Debug, Default)]
pub struct L4OrderBookParams {
    /// Optional Unix-ms timestamp to fetch a historical snapshot.
    pub timestamp: Option<Timestamp>,
    /// Number of price levels per side.
    pub depth: Option<i32>,
}

/// Parameters for paginated L4 orderbook diffs.
#[derive(Debug)]
pub struct L4DiffsParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for paginated L4 orderbook history.
#[derive(Debug)]
pub struct L4HistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub depth: Option<i32>,
}

/// Access to L4 (node-level) orderbook endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct L4OrderBookResource {
    http: HttpClient,
    prefix: String,
}

impl L4OrderBookResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the current (or point-in-time) L4 orderbook for a symbol.
    pub async fn get(
        &self,
        symbol: &str,
        params: Option<L4OrderBookParams>,
    ) -> Result<serde_json::Value> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(ts) = p.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(d) = p.depth {
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(&format!("{}/orderbook/{}/l4", self.prefix, symbol), &qp)
            .await
    }

    /// Get paginated L4 orderbook diffs for a symbol.
    pub async fn diffs(
        &self,
        symbol: &str,
        params: L4DiffsParams,
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
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/orderbook/{}/l4/diffs", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get paginated L4 orderbook history for a symbol.
    pub async fn history(
        &self,
        symbol: &str,
        params: L4HistoryParams,
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
        if let Some(d) = params.depth {
            qp.push(("depth", d.to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(
                &format!("{}/orderbook/{}/l4/history", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
