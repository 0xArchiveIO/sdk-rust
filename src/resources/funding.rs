use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CursorResponse, FundingRate, OiFundingInterval, Timestamp};

/// Parameters for paginated funding rate history.
#[derive(Debug)]
pub struct FundingHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub interval: Option<OiFundingInterval>,
}

/// Access to funding rate endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct FundingResource {
    http: HttpClient,
    prefix: String,
}

impl FundingResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated historical funding rates.
    pub async fn history(
        &self,
        coin: &str,
        params: FundingHistoryParams,
    ) -> Result<CursorResponse<Vec<FundingRate>>> {
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
            .get_with_cursor(&format!("{}/funding/{}", self.prefix, coin), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Get the current funding rate.
    pub async fn current(&self, coin: &str) -> Result<FundingRate> {
        self.http
            .get(&format!("{}/funding/{}/current", self.prefix, coin), &[])
            .await
    }
}
