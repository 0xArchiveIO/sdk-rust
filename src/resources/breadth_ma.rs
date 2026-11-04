//! Breadth above the n-day moving average for Hyperliquid core and HIP-3
//! perps. Build, Pro, Scale and Enterprise plans.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::resources::pages::{pin_window, CursorPages};
use crate::types::{
    BreadthMaLength, BreadthMaReading, CursorResponse, OiFundingInterval, Timestamp,
};

/// The largest `limit` the moving-average breadth history accepts.
pub const BREADTH_MA_MAX_LIMIT: i64 = 1_000;

/// Window, bucket width and page for [`BreadthMaResource::history`] and
/// [`BreadthMaResource::iterate`]. The average's length is a separate,
/// required argument.
///
/// A page holds the buckets whose open is in `[start, end]`, oldest first.
/// Pass `next_cursor` back as `cursor` with every other argument unchanged,
/// and stop when it is `None`. The cursor is opaque: do not derive it from a
/// reading. Set `start` and `end` when paging; without them each request
/// re-derives the default window, the 24 hours before now.
#[derive(Debug, Default, Clone)]
pub struct BreadthMaHistoryParams {
    /// Window start (default: 24 hours before `end`).
    pub start: Option<Timestamp>,
    /// Window end (default: now).
    pub end: Option<Timestamp>,
    /// Bucket width: `1m` (default), `5m`, `15m`, `30m`, `1h`, `4h` or `1d`,
    /// on UTC epoch boundaries (`1d` is the UTC day).
    pub interval: Option<OiFundingInterval>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Buckets per page, 1 to 1,000 (default 100).
    pub limit: Option<i64>,
}

impl BreadthMaHistoryParams {
    fn query(&self, n: BreadthMaLength) -> Result<Vec<(&'static str, String)>> {
        if let Some(limit) = self.limit {
            if !(1..=BREADTH_MA_MAX_LIMIT).contains(&limit) {
                return Err(Error::InvalidParam(format!(
                    "moving-average breadth limit must be between 1 and {BREADTH_MA_MAX_LIMIT}"
                )));
            }
        }
        let mut qp = vec![("n", n.days().to_string())];
        if let Some(s) = &self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = &self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(i) = self.interval {
            qp.push(("interval", i.as_str().to_string()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        Ok(qp)
    }
}

/// The share of perps trading above their n-day moving average, with the
/// quartiles of how far above or below, as `client.hyperliquid.breadth_ma`
/// (core perps) and `client.hyperliquid.hip3.breadth_ma` (HIP-3 perps).
/// Method `ma-breadth/1`.
///
/// Served for the Build, Pro, Scale and Enterprise plans; on Free the API
/// answers 403 and the error carries
/// [`PlanRequired`](crate::error::PlanRequired).
///
/// Each length starts on its own date (UTC). Hyperliquid: n = 5 from
/// 2025-03-27, n = 20 from 2025-04-11, n = 50 from 2025-05-11, n = 200 from
/// 2025-10-08. HIP-3: n = 5 from 2025-12-27, n = 20 from 2026-01-11, n = 50
/// from 2026-02-10, n = 200 from 2026-07-10. A window that ends before its
/// length's start is empty, with `meta.coverage_from` and `meta.notice`.
///
/// Candidates are every perp the venue ever listed, from its first 1-minute
/// bar. A delisted perp stays a candidate, counted in `excluded_stale_price`
/// from 24 hours after its last trade, and a perp younger than n days counts
/// in `excluded_short_history`, so `coverage_ratio` depends on n, is not a
/// data-quality ratio, and is not comparable with above-VWAP breadth. A
/// perp's price is its last 1-minute close before the reading; its average is
/// the mean of its daily closes over the n UTC days before the reading's
/// day. `value_pct` and `distance` are `None` when nothing is eligible.
///
/// A 503 `upstream_unavailable` right after the service restarts is
/// retryable.
#[derive(Debug, Clone)]
pub struct BreadthMaResource {
    http: HttpClient,
    prefix: String,
}

impl BreadthMaResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the newest 1-minute reading for length `n`, as of about 4 minutes
    /// before now, on the minute.
    pub async fn current(&self, n: BreadthMaLength) -> Result<BreadthMaReading> {
        self.http
            .get(
                &format!("{}/breadth/above-ma/current", self.prefix),
                &[("n", n.days().to_string())],
            )
            .await
    }

    /// Get one page of readings for length `n`.
    ///
    /// Each bucket carries its last reading, as of its end
    /// (`calculated_at`); the bucket still forming is as of about 4 minutes
    /// before now. Never average `value_pct` across buckets.
    pub async fn history(
        &self,
        n: BreadthMaLength,
        params: BreadthMaHistoryParams,
    ) -> Result<CursorResponse<Vec<BreadthMaReading>>> {
        let mut qp = params.query(n)?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http
            .get_with_cursor(&format!("{}/breadth/above-ma", self.prefix), &qp)
            .await
    }

    /// Walk every bucket of `[start, end]` for length `n`.
    ///
    /// `start` is required. When `end` is `None` it is fixed to the current
    /// time now, before the first request, and every page sends that same
    /// `end`. A `cursor` in `params` resumes from that page.
    pub fn iterate(
        &self,
        n: BreadthMaLength,
        params: BreadthMaHistoryParams,
    ) -> Result<CursorPages<BreadthMaReading>> {
        let params = pin_window(params.start.is_some(), params, |p| &mut p.end)?;
        let qp = params.query(n)?;
        Ok(CursorPages::new(
            self.http.clone(),
            format!("{}/breadth/above-ma", self.prefix),
            qp,
            params.cursor,
        ))
    }
}
