//! Cumulative volume delta for Hyperliquid core and HIP-3 symbols.

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{CandleInterval, CvdBucket, MetaResponse, Timestamp};

/// Window, bucket width and page for `client.hyperliquid.cvd()` and
/// `client.hyperliquid.hip3.cvd()`.
///
/// A page holds up to `limit` buckets. While the response's `next_cursor` is
/// set, pass it back as `cursor` with the same `start`, `end` and `interval`,
/// and stop when it is `None`. Below `1h` a page can hold fewer than `limit`
/// buckets and still carry a cursor, so stop on the cursor, not on a short
/// page. Without `start` or `cursor`, the response is the newest `limit`
/// buckets of the 24 hours before `end`, with no cursor.
#[derive(Debug, Default, Clone)]
pub struct CvdParams {
    /// Start of the window. Omit it, with no `cursor`, for the 24 hours
    /// before `end`.
    pub start: Option<Timestamp>,
    /// End of the window (default: now).
    pub end: Option<Timestamp>,
    /// Bucket width (default `1h`). `1h` and longer roll up hourly totals;
    /// `1m`, `5m`, `15m` and `30m` are summed from taker fills. `4h`, `1d` and
    /// `1w` buckets open on UTC epoch boundaries, so `1w` buckets open on
    /// Thursdays.
    pub interval: Option<CandleInterval>,
    /// The previous page's `next_cursor`: the page starts at the bucket that
    /// opens after it.
    pub cursor: Option<String>,
    /// Buckets per page (default 500, max 10,000).
    pub limit: Option<i64>,
}

impl CvdParams {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut qp = vec![];
        if let Some(s) = &self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = &self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(i) = self.interval {
            qp.push(("interval", i.as_str().to_string()));
        }
        if let Some(c) = &self.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        qp
    }
}

pub(crate) async fn fetch_cvd(
    http: &HttpClient,
    prefix: &str,
    symbol: &str,
    params: CvdParams,
) -> Result<MetaResponse<Vec<CvdBucket>>> {
    let (data, meta) = http
        .get_with_meta(&format!("{}/cvd/{}", prefix, symbol), &params.to_query())
        .await?;
    Ok(MetaResponse::new(data, meta))
}
