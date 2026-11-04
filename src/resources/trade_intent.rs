//! Trade intent for Hyperliquid core and HIP-3 symbols: why a market traded,
//! bucket by bucket. Build, Pro, Scale and Enterprise plans.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::resources::pages::{pin_window, CursorPages};
use crate::types::{CursorResponse, Timestamp, TradeIntentBucket, TradeIntentInterval};

/// The largest `limit` the trade intent routes accept.
pub const TRADE_INTENT_MAX_LIMIT: i64 = 2_000;

/// Window, bucket width and page for `client.hyperliquid.trade_intent` and
/// `client.hyperliquid.hip3.trade_intent`.
///
/// A page holds the buckets whose open is in `[start, end]`, oldest first,
/// never past the bucket holding now, and never splits a bucket. Pass
/// `next_cursor` back as `cursor` with every other field unchanged, and stop
/// when it is `None`: a page can hold fewer than `limit` buckets, or none,
/// and still be followed by more. Set `start` and `end` when paging; without
/// them each request re-derives the default window, the 24 hours before now.
#[derive(Debug, Default, Clone)]
pub struct TradeIntentParams {
    /// Window start (default: 24 hours before `end`).
    pub start: Option<Timestamp>,
    /// Window end (default: now). The bucket holding `end` is served whole.
    pub end: Option<Timestamp>,
    /// Bucket width (default `1h`).
    pub interval: Option<TradeIntentInterval>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Buckets per page, 1 to 2,000 (default 500).
    pub limit: Option<i64>,
}

impl TradeIntentParams {
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        if let Some(limit) = self.limit {
            if !(1..=TRADE_INTENT_MAX_LIMIT).contains(&limit) {
                return Err(Error::InvalidParam(format!(
                    "trade intent limit must be between 1 and {TRADE_INTENT_MAX_LIMIT}"
                )));
            }
        }
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
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        Ok(qp)
    }
}

/// Trade intent: per bucket, the USD volume of new longs and shorts
/// (`open_*`), profit taken (`tp_*`), positions stopped out (`stop_*`:
/// closes at a loss, liquidations and auto-deleveraging) and flips through
/// zero (`flip_ls`, `flip_sl`), with market makers' legs apart in `mm_usd`,
/// and the market's trailing-72h winners and losers as aggregates (no wallet
/// is named).
///
/// Served on Hyperliquid core (`client.hyperliquid.trade_intent`) and HIP-3
/// (`client.hyperliquid.hip3.trade_intent`) for the Build, Pro, Scale and
/// Enterprise plans; on Free the API answers 403 and the error carries
/// [`PlanRequired`](crate::error::PlanRequired).
///
/// Coverage (UTC): hourly widths from 2025-05-25 15:00 on Hyperliquid and
/// from 2025-10-13 12:00 on HIP-3; minute widths from 2026-07-14 00:00;
/// second widths for the last 3 days. A window that ends before its width's
/// coverage is empty, with `meta.coverage_from` and `meta.notice`.
///
/// `meta.built_through` says how far the series is built: a second without a
/// record had no trades up to it, and nothing is known after it.
/// `meta.preliminary_row_count` counts the buckets that hold preliminary data
/// from the hour in progress. A 503 `upstream_unavailable` for a read that
/// took too long is retryable: lower `limit` or narrow the window.
#[derive(Debug, Clone)]
pub struct TradeIntentResource {
    http: HttpClient,
    prefix: String,
}

impl TradeIntentResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    fn path(&self, symbol: &str) -> String {
        format!("{}/trade-intent/{}", self.prefix, symbol)
    }

    /// Get one page of trade intent buckets for a symbol (`BTC`; HIP-3
    /// symbols keep their builder prefix and case, `xyz:XYZ100`).
    ///
    /// The response's `meta` carries the canonical `symbol`, the `venue`,
    /// `built_through` and, while the window holds data from the hour in
    /// progress, `preliminary_row_count`.
    pub async fn history(
        &self,
        symbol: &str,
        params: TradeIntentParams,
    ) -> Result<CursorResponse<Vec<TradeIntentBucket>>> {
        let mut qp = params.query()?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http.get_with_cursor(&self.path(symbol), &qp).await
    }

    /// Walk every page of `[start, end]` for a symbol.
    ///
    /// `start` is required. When `end` is `None` it is fixed to the current
    /// time now, before the first request, and every page sends that same
    /// `end`. A `cursor` in `params` resumes from that page. The pager stops
    /// only when a page carries no `next_cursor`.
    pub fn iterate(
        &self,
        symbol: &str,
        params: TradeIntentParams,
    ) -> Result<CursorPages<TradeIntentBucket>> {
        let params = pin_window(params.start.is_some(), params, |p| &mut p.end)?;
        let qp = params.query()?;
        Ok(CursorPages::new(
            self.http.clone(),
            self.path(symbol),
            qp,
            params.cursor,
        ))
    }
}
