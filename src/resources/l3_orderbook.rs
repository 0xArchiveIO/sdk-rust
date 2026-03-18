use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, Timestamp};

/// Parameters for paginated L3 orderbook history.
#[derive(Debug)]
pub struct L3HistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
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
    pub async fn get(
        &self,
        symbol: &str,
        depth: Option<i64>,
    ) -> Result<serde_json::Value> {
        let mut qp = vec![];
        if let Some(d) = depth {
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
