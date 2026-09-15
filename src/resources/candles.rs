use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{Candle, CandleInterval, CursorResponse, Timestamp};

/// Parameters for paginated candle history.
#[derive(Debug)]
pub struct CandleHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    /// Max results per page (default 100). Hyperliquid, HIP-3, and Lighter
    /// accept at most 10,000 rows; HIP-4 and Spot accept at most 1,000.
    pub limit: Option<i64>,
    pub interval: Option<CandleInterval>,
}

/// Access to OHLCV candle endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct CandlesResource {
    http: HttpClient,
    prefix: String,
    symbol_transform: fn(&str) -> String,
    max_limit: Option<i64>,
}

impl CandlesResource {
    pub(crate) fn new_with_transform_and_limit(
        http: HttpClient,
        prefix: &str,
        symbol_transform: fn(&str) -> String,
        max_limit: Option<i64>,
    ) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
            symbol_transform,
            max_limit,
        }
    }

    /// Get paginated historical candles.
    ///
    /// Returns an error if the `start` timestamp is more than 5 minutes in the
    /// future (the API silently returns empty data for future ranges), or if a
    /// route-specific page limit is exceeded.
    pub async fn history(
        &self,
        symbol: &str,
        params: CandleHistoryParams,
    ) -> Result<CursorResponse<Vec<Candle>>> {
        let start_ms = params.start.to_millis();
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Allow 5 minutes of clock skew tolerance
        if start_ms > now_ms + 5 * 60 * 1000 {
            return Err(Error::InvalidParam(
                "start timestamp is in the future — no candle data can exist yet".into(),
            ));
        }

        let mut qp = vec![
            ("start", start_ms.to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            if let Some(max_limit) = self.max_limit {
                if !(1..=max_limit).contains(&l) {
                    return Err(Error::InvalidParam(format!(
                        "candle limit must be between 1 and {max_limit} for this route"
                    )));
                }
            }
            qp.push(("limit", l.to_string()));
        }
        if let Some(i) = params.interval {
            qp.push(("interval", i.as_str().to_string()));
        }
        let symbol = (self.symbol_transform)(symbol);
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/candles/{}", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }
}
