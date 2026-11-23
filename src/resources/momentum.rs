//! The momentum factor for Hyperliquid core perps. Build, Pro, Scale and
//! Enterprise plans.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::resources::pages::{pin_window, CursorPages};
use crate::types::{
    CursorResponse, MetaResponse, MomentumInterval, MomentumLookback, MomentumReturn,
    MomentumScore, MomentumUniverse, Timestamp,
};

/// The largest `limit` the momentum factor's returns accept.
pub const MOMENTUM_MAX_LIMIT: i64 = 1_000;

/// Universe, window, bucket width and page for
/// [`MomentumResource::returns`] and [`MomentumResource::iterate_returns`].
/// The lookback is a separate, required argument.
///
/// A page holds the buckets whose open is in `[start, end]`, oldest first.
/// Pass `next_cursor` back as `cursor` with every other argument unchanged,
/// and stop when it is `None`. Set `end` when paging; without it each
/// request re-derives the default window, the 30 days before now.
#[derive(Debug, Default, Clone)]
pub struct MomentumReturnsParams {
    /// `Top25` (the API's default) or `Top50`.
    pub universe: Option<MomentumUniverse>,
    /// Window start (default: 30 days before `end`).
    pub start: Option<Timestamp>,
    /// Window end (default: now).
    pub end: Option<Timestamp>,
    /// Bucket width: `1d` (default) or `1h`, on UTC boundaries.
    pub interval: Option<MomentumInterval>,
    /// `Some(true)` adds each bucket's `holdings`: the weights of the
    /// rebalance in force at its last hour.
    pub holdings: Option<bool>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Buckets per page, 1 to 1,000 (default 100).
    pub limit: Option<i64>,
}

impl MomentumReturnsParams {
    fn query(&self, lookback: MomentumLookback) -> Result<Vec<(&'static str, String)>> {
        if let Some(limit) = self.limit {
            if !(1..=MOMENTUM_MAX_LIMIT).contains(&limit) {
                return Err(Error::InvalidParam(format!(
                    "momentum factor limit must be between 1 and {MOMENTUM_MAX_LIMIT}"
                )));
            }
        }
        let mut qp = vec![("lookback", lookback.as_str().to_string())];
        if let Some(u) = self.universe {
            qp.push(("universe", u.as_str().to_string()));
        }
        if let Some(s) = &self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = &self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(i) = self.interval {
            qp.push(("interval", i.as_str().to_string()));
        }
        if let Some(h) = self.holdings {
            qp.push(("holdings", h.to_string()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        Ok(qp)
    }
}

/// Every Hyperliquid core perp's momentum score at an hour, and the returns
/// of the long-short momentum factor built from those scores, as
/// `client.hyperliquid.factors.momentum`. Method `momentum/1`.
///
/// Served for the Build, Pro, Scale and Enterprise plans; on Free the API
/// answers 403 and the error carries
/// [`PlanRequired`](crate::error::PlanRequired).
///
/// The lookback is required on every call: 7 or 14 days. A perp's score is
/// its close-to-close return over the lookback divided by the standard
/// deviation of its last 30 days of hourly returns.
///
/// The factor's universe is the top 25 (the default) or top 50 perps by
/// 30-day dollar volume, re-picked every 30 days, with delisted perps
/// included while they traded. Every 168 hours (Saturdays 00:00 UTC, earning
/// from 01:00) it goes long the top fifth of the universe on the score and
/// short the bottom fifth, each leg weighted by inverse volatility and
/// summing to 1. Returns are gross of fees and funding.
///
/// Scores and returns start 2025-05-24 01:00 UTC. A request that ends before
/// then is empty, with `meta.coverage_from` and `meta.notice`. A 503
/// `upstream_unavailable` ("still being prepared") on the first request
/// after the service restarts is retryable.
#[derive(Debug, Clone)]
pub struct MomentumResource {
    http: HttpClient,
    prefix: String,
}

impl MomentumResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get every eligible perp's reading as of the close of one hour,
    /// strongest first: the hour at or before `at` (default now; a later
    /// time gets the newest reading). The response is never paged.
    pub async fn get(
        &self,
        lookback: MomentumLookback,
        at: Option<Timestamp>,
    ) -> Result<MetaResponse<Vec<MomentumScore>>> {
        let mut qp = vec![("lookback", lookback.as_str().to_string())];
        if let Some(at) = &at {
            qp.push(("at", at.to_millis().to_string()));
        }
        self.http
            .get_with_cursor(&format!("{}/factors/momentum", self.prefix), &qp)
            .await
    }

    /// Get the newest reading: every eligible perp, strongest first.
    pub async fn current(
        &self,
        lookback: MomentumLookback,
    ) -> Result<MetaResponse<Vec<MomentumScore>>> {
        self.http
            .get_with_cursor(
                &format!("{}/factors/momentum/current", self.prefix),
                &[("lookback", lookback.as_str().to_string())],
            )
            .await
    }

    /// Get one page of the factor's returns, one bucket per interval, oldest
    /// first.
    ///
    /// A bucket's `ret` is the sum of the factor's hourly returns over the
    /// `hours` it holds (simple, not compounded), with each leg's return and
    /// the legs' sizes in force at its last hour.
    pub async fn returns(
        &self,
        lookback: MomentumLookback,
        params: MomentumReturnsParams,
    ) -> Result<CursorResponse<Vec<MomentumReturn>>> {
        let mut qp = params.query(lookback)?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http
            .get_with_cursor(&format!("{}/factors/momentum/returns", self.prefix), &qp)
            .await
    }

    /// Walk every bucket of `[start, end]`.
    ///
    /// `start` is required. When `end` is `None` it is fixed to the current
    /// time now, before the first request, and every page sends that same
    /// `end`. A `cursor` in `params` resumes from that page.
    pub fn iterate_returns(
        &self,
        lookback: MomentumLookback,
        params: MomentumReturnsParams,
    ) -> Result<CursorPages<MomentumReturn>> {
        let params = pin_window(params.start.is_some(), params, |p| &mut p.end)?;
        let qp = params.query(lookback)?;
        Ok(CursorPages::new(
            self.http.clone(),
            format!("{}/factors/momentum/returns", self.prefix),
            qp,
            params.cursor,
        ))
    }
}

/// Factors on Hyperliquid core perps, as `client.hyperliquid.factors`.
/// Build, Pro, Scale and Enterprise plans.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FactorsResource {
    /// The momentum factor: every perp's score at an hour, and the
    /// long-short factor's returns (from 2025-05-24 01:00 UTC).
    pub momentum: MomentumResource,
}

impl FactorsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            momentum: MomentumResource::new(http, prefix),
        }
    }
}
