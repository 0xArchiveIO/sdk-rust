use std::time::Duration;

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{
    CoverageResponse, ExchangeCoverage, Incident, IncidentsResponse, LatencyResponse,
    PositionsFreshness, SlaResponse, StatusResponse, SymbolCoverageResponse, Timestamp,
};

/// Filters and paging for [`DataQualityResource::list_incidents_with`].
#[derive(Debug, Default, Clone)]
pub struct ListIncidentsParams {
    /// `open`, `investigating`, `identified`, `monitoring` or `resolved`.
    pub status: Option<String>,
    /// Venue scope, for example `hyperliquid` or `lighter`.
    pub exchange: Option<String>,
    /// Only incidents that started after this time.
    pub since: Option<Timestamp>,
    /// Incidents per page (default 20, max 100).
    pub limit: Option<i64>,
    /// Incidents to skip, for paging.
    pub offset: Option<i64>,
}

/// Gap-detection window for [`DataQualityResource::symbol_coverage_with`].
#[derive(Debug, Default, Clone)]
pub struct SymbolCoverageParams {
    /// Start of the window (default: 30 days before now).
    pub from: Option<Timestamp>,
    /// End of the window (default: now).
    pub to: Option<Timestamp>,
}

/// Percent-encode one path segment (`km:US500`, `#0`), keeping its case.
fn esc(segment: &str) -> String {
    urlencoding::encode(segment).into_owned()
}

/// Timeout for data quality endpoints that aggregate across venue APIs/symbols.
/// These can be significantly slower than per-instrument queries.
const SLOW_ENDPOINT_TIMEOUT: Duration = Duration::from_secs(120);

/// Access to data quality monitoring endpoints.
#[derive(Debug, Clone)]
pub struct DataQualityResource {
    http: HttpClient,
}

impl DataQualityResource {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Get overall system status.
    pub async fn status(&self) -> Result<StatusResponse> {
        self.http.get("/v1/data-quality/status", &[]).await
    }

    /// Get data coverage across venue APIs.
    ///
    /// This endpoint aggregates coverage data across supported venue APIs and data
    /// types, which can take longer than other queries. It uses a 120-second
    /// timeout instead of the default 30 seconds.
    pub async fn coverage(&self) -> Result<CoverageResponse> {
        self.http
            .get_with_timeout(
                "/v1/data-quality/coverage",
                &[],
                Some(SLOW_ENDPOINT_TIMEOUT),
            )
            .await
    }

    /// Get data coverage for a single venue scope.
    pub async fn exchange_coverage(&self, exchange: &str) -> Result<ExchangeCoverage> {
        self.http
            .get(&format!("/v1/data-quality/coverage/{}", esc(exchange)), &[])
            .await
    }

    /// Get symbol-level coverage with gap detection over the last 30 days.
    ///
    /// The symbol is sent as given, case preserved and percent-encoded, so
    /// `km:US500`, `HYPE-USDC` and `#0` all work.
    pub async fn symbol_coverage(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<SymbolCoverageResponse> {
        self.symbol_coverage_with(exchange, symbol, SymbolCoverageParams::default())
            .await
    }

    /// Get symbol-level coverage with gap detection over a chosen window.
    pub async fn symbol_coverage_with(
        &self,
        exchange: &str,
        symbol: &str,
        params: SymbolCoverageParams,
    ) -> Result<SymbolCoverageResponse> {
        let mut qp = vec![];
        if let Some(f) = &params.from {
            qp.push(("from", f.to_millis().to_string()));
        }
        if let Some(t) = &params.to {
            qp.push(("to", t.to_millis().to_string()));
        }
        self.http
            .get(
                &format!(
                    "/v1/data-quality/coverage/{}/{}",
                    esc(exchange),
                    esc(symbol)
                ),
                &qp,
            )
            .await
    }

    /// List data incidents, optionally filtered by status.
    pub async fn list_incidents(&self, status: Option<&str>) -> Result<IncidentsResponse> {
        self.list_incidents_with(ListIncidentsParams {
            status: status.map(str::to_string),
            ..Default::default()
        })
        .await
    }

    /// List data incidents with filters and offset paging.
    pub async fn list_incidents_with(
        &self,
        params: ListIncidentsParams,
    ) -> Result<IncidentsResponse> {
        let mut qp = vec![];
        if let Some(s) = params.status {
            qp.push(("status", s));
        }
        if let Some(e) = params.exchange {
            qp.push(("exchange", e));
        }
        if let Some(s) = &params.since {
            qp.push(("since", s.to_millis().to_string()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(o) = params.offset {
            qp.push(("offset", o.to_string()));
        }
        self.http.get("/v1/data-quality/incidents", &qp).await
    }

    /// Get a single incident by ID.
    pub async fn get_incident(&self, incident_id: &str) -> Result<Incident> {
        self.http
            .get(&format!("/v1/data-quality/incidents/{}", esc(incident_id)), &[])
            .await
    }

    /// Get latency metrics across exchanges.
    pub async fn latency(&self) -> Result<LatencyResponse> {
        self.http.get("/v1/data-quality/latency", &[]).await
    }

    /// Get SLA compliance metrics.
    ///
    /// Pass `year` + `month` (1-12) together to pin a period; omit both for
    /// the current month. Uses a 120-second timeout (this endpoint can be
    /// slow when computing compliance across all data types).
    pub async fn sla(&self, year: Option<i32>, month: Option<u8>) -> Result<SlaResponse> {
        let mut qp = vec![];
        if let Some(y) = year {
            qp.push(("year", y.to_string()));
        }
        if let Some(m) = month {
            qp.push(("month", m.to_string()));
        }
        self.http
            .get_with_timeout("/v1/data-quality/sla", &qp, Some(SLOW_ENDPOINT_TIMEOUT))
            .await
    }

    /// Get the freshness of the account positions data, one row per venue
    /// (Hyperliquid core, HIP-3, Lighter and Lighter on Robinhood Chain): the
    /// latest live snapshot and its age, whether it is stale, the latest
    /// hourly snapshot, and the `built_through` and `finalized_through`
    /// boundaries.
    pub async fn positions_freshness(&self) -> Result<Vec<PositionsFreshness>> {
        self.http.get("/v1/data-quality/positions", &[]).await
    }
}
