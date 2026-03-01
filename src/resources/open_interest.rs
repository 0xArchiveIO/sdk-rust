use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, OiFundingInterval, OpenInterest, Timestamp};

/// Parameters for paginated open interest history.
#[derive(Debug)]
pub struct OpenInterestHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub interval: Option<OiFundingInterval>,
}

/// Access to open interest endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct OpenInterestResource {
    http: HttpClient,
    prefix: String,
}

impl OpenInterestResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated historical open interest snapshots.
    pub async fn history(
        &self,
        coin: &str,
        params: OpenInterestHistoryParams,
    ) -> Result<CursorResponse<Vec<OpenInterest>>> {
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
        if let Some(i) = params.interval {
            qp.push(("interval", i.as_str().to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/openinterest/{}", self.prefix, coin), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get the current open interest.
    pub async fn current(&self, coin: &str) -> Result<OpenInterest> {
        self.http
            .get(
                &format!("{}/openinterest/{}/current", self.prefix, coin),
                &[],
            )
            .await
    }
}
