use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, L2DiffEntry, L2OrderBookSnapshot, Timestamp};

/// Parameters for fetching a single L2 full-depth orderbook snapshot.
#[derive(Debug, Default)]
pub struct L2OrderBookParams {
    /// Optional Unix-ms timestamp for historical state (omit for current).
    pub timestamp: Option<Timestamp>,
    /// Number of price levels per side (omit for full depth).
    pub depth: Option<i32>,
}

/// Parameters for paginated L2 orderbook history.
#[derive(Debug)]
pub struct L2HistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub depth: Option<i32>,
}

/// Parameters for paginated L2 orderbook diffs.
#[derive(Debug)]
pub struct L2DiffsParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// Access to L2 full-depth orderbook endpoints (derived from L4 data).
#[derive(Debug, Clone)]
pub struct L2OrderBookResource {
    http: HttpClient,
    prefix: String,
}

impl L2OrderBookResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the current (or point-in-time) full-depth L2 orderbook.
    pub async fn get(
        &self,
        symbol: &str,
        params: Option<L2OrderBookParams>,
    ) -> Result<L2OrderBookSnapshot> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(ts) = p.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(d) = p.depth {
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(&format!("{}/orderbook/{}/l2", self.prefix, symbol), &qp)
            .await
    }

    /// Get paginated L2 full-depth history.
    pub async fn history(
        &self,
        symbol: &str,
        params: L2HistoryParams,
    ) -> Result<CursorResponse<Vec<L2OrderBookSnapshot>>> {
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
                &format!("{}/orderbook/{}/l2/history", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get paginated L2 tick-level diffs.
    pub async fn diffs(
        &self,
        symbol: &str,
        params: L2DiffsParams,
    ) -> Result<CursorResponse<Vec<L2DiffEntry>>> {
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
                &format!("{}/orderbook/{}/l2/diffs", self.prefix, symbol),
                &qp,
            )
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
