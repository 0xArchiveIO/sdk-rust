//! USDC entering and leaving HyperCore: every flow event, and totals per
//! bucket. Hyperliquid only; Build, Pro, Scale and Enterprise plans.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::resources::pages::{pin_window, CursorPages};
use crate::types::{
    CursorResponse, OiFundingInterval, Timestamp, UsdcFlowEvent, UsdcFlowKind, UsdcFlowVolumeBucket,
};

/// The largest `limit` the USDC flow routes accept.
pub const USDC_FLOWS_MAX_LIMIT: i64 = 1_000;

/// The largest `min_usd` the events route accepts.
pub const USDC_FLOWS_MAX_MIN_USD: f64 = 1e12;

const PREFIX: &str = "/v1/hyperliquid/usdc-flows";

fn check_limit(limit: Option<i64>) -> Result<()> {
    match limit {
        Some(l) if !(1..=USDC_FLOWS_MAX_LIMIT).contains(&l) => Err(Error::InvalidParam(format!(
            "USDC flows limit must be between 1 and {USDC_FLOWS_MAX_LIMIT}"
        ))),
        _ => Ok(()),
    }
}

/// Window, filters and page for [`UsdcFlowsResource::history`] and
/// [`UsdcFlowsResource::iterate`].
///
/// Pass `next_cursor` back as `cursor` with every other field unchanged, and
/// stop when it is `None`. A page can be empty and still be followed by
/// more: with a high `min_usd`, a page reads at most 31 days of history.
#[derive(Debug, Default, Clone)]
pub struct UsdcFlowsParams {
    /// Window start, inclusive (default: 24 hours before `end`).
    pub start: Option<Timestamp>,
    /// Window end, inclusive (default: now).
    pub end: Option<Timestamp>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Events per page, 1 to 1,000 (default 100).
    pub limit: Option<i64>,
    /// Keep only events of at least this many USD (0 to 1e12, default 0).
    pub min_usd: Option<f64>,
    /// Keep only one kind of flow (default: every kind).
    pub kind: Option<UsdcFlowKind>,
}

impl UsdcFlowsParams {
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        check_limit(self.limit)?;
        if let Some(m) = self.min_usd {
            if !(m.is_finite() && (0.0..=USDC_FLOWS_MAX_MIN_USD).contains(&m)) {
                return Err(Error::InvalidParam(
                    "USDC flows min_usd must be between 0 and 1e12".to_string(),
                ));
            }
        }
        let mut qp = vec![];
        if let Some(s) = &self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = &self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(m) = self.min_usd {
            qp.push(("min_usd", m.to_string()));
        }
        if let Some(k) = self.kind {
            qp.push(("kind", k.as_str().to_string()));
        }
        Ok(qp)
    }
}

/// Window and page for [`UsdcFlowsResource::volume`] and
/// [`UsdcFlowsResource::iterate_volume`].
///
/// Totals count every flow, whatever its size, so this route takes no
/// `min_usd` or `kind`.
#[derive(Debug, Default, Clone)]
pub struct UsdcFlowVolumeParams {
    /// Window start (default: 24 hours before `end`).
    pub start: Option<Timestamp>,
    /// Window end (default: now).
    pub end: Option<Timestamp>,
    /// Bucket width: `1m`, `5m`, `15m`, `30m`, `1h` (default), `4h` or `1d`,
    /// on UTC epoch boundaries (`1d` is the UTC day).
    pub interval: Option<OiFundingInterval>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Buckets per page, 1 to 1,000 (default 100).
    pub limit: Option<i64>,
}

impl UsdcFlowVolumeParams {
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        check_limit(self.limit)?;
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

/// USDC flows into and out of HyperCore, as `client.hyperliquid.usdc_flows`:
/// Arbitrum bridge deposits and withdrawals (`deposit`, `withdraw`) and
/// transfers between HyperCore and HyperEVM (`evm_in`, `evm_out`), which
/// also carry CCTP transfers. Each event names the HyperCore account
/// (`user`).
///
/// Hyperliquid only, from 2025-09-27 09:33 UTC (the first flow), for the
/// Build, Pro, Scale and Enterprise plans; on Free the API answers 403 and
/// the error carries [`PlanRequired`](crate::error::PlanRequired). A window
/// that ends before coverage is empty, with `meta.coverage_from` and
/// `meta.notice`.
///
/// Events come oldest first, in execution order. `id` (`block:index`) is
/// each event's identity: join pages and deduplicate on it. Events in the
/// last 3 hours can still be added after you read them, so re-read that span
/// and deduplicate on `id`.
#[derive(Debug, Clone)]
pub struct UsdcFlowsResource {
    http: HttpClient,
}

impl UsdcFlowsResource {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Get one page of flow events (`GET /v1/hyperliquid/usdc-flows`).
    pub async fn history(
        &self,
        params: UsdcFlowsParams,
    ) -> Result<CursorResponse<Vec<UsdcFlowEvent>>> {
        let mut qp = params.query()?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http.get_with_cursor(PREFIX, &qp).await
    }

    /// Walk every event of `[start, end]` that matches the filters.
    ///
    /// `start` is required. When `end` is `None` it is fixed to the current
    /// time now, before the first request, and every page sends that same
    /// `end`. A `cursor` in `params` resumes from that page. The pager stops
    /// only when a page carries no `next_cursor`, so empty pages in a sparse
    /// window are walked through.
    pub fn iterate(&self, params: UsdcFlowsParams) -> Result<CursorPages<UsdcFlowEvent>> {
        let params = pin_window(params.start.is_some(), params, |p| &mut p.end)?;
        let qp = params.query()?;
        Ok(CursorPages::new(
            self.http.clone(),
            PREFIX.to_string(),
            qp,
            params.cursor,
        ))
    }

    /// Get one page of flow totals per bucket
    /// (`GET /v1/hyperliquid/usdc-flows/volume`).
    ///
    /// Every bucket whose open is in `[start, end]` is returned, zeros
    /// included, never past the bucket still filling and never before the
    /// bucket of the first flow. A page whose first bucket is only partly
    /// covered says so in `meta.coverage_from` and `meta.notice`.
    pub async fn volume(
        &self,
        params: UsdcFlowVolumeParams,
    ) -> Result<CursorResponse<Vec<UsdcFlowVolumeBucket>>> {
        let mut qp = params.query()?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http
            .get_with_cursor(&format!("{PREFIX}/volume"), &qp)
            .await
    }

    /// Walk every bucket of `[start, end]`.
    ///
    /// `start` is required. When `end` is `None` it is fixed to the current
    /// time now, before the first request, and every page sends that same
    /// `end`. A `cursor` in `params` resumes from that page.
    pub fn iterate_volume(
        &self,
        params: UsdcFlowVolumeParams,
    ) -> Result<CursorPages<UsdcFlowVolumeBucket>> {
        let params = pin_window(params.start.is_some(), params, |p| &mut p.end)?;
        let qp = params.query()?;
        Ok(CursorPages::new(
            self.http.clone(),
            format!("{PREFIX}/volume"),
            qp,
            params.cursor,
        ))
    }
}
