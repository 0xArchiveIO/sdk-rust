use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{Candle, CandleInterval, CursorResponse, Timestamp};

/// Parameters for paginated candle history.
#[derive(Debug)]
pub struct CandleHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    /// Max results per page (default 100, max 10,000 for candles).
    pub limit: Option<i64>,
    pub interval: Option<CandleInterval>,
}

/// Access to OHLCV candle endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct CandlesResource {
    http: HttpClient,
    prefix: String,
}

impl CandlesResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get paginated historical candles.
    pub async fn history(
        &self,
        coin: &str,
        params: CandleHistoryParams,
    ) -> Result<CursorResponse<Vec<Candle>>> {
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
            .get_with_cursor(&format!("{}/candles/{}", self.prefix, coin), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
