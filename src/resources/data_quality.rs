use std::time::Duration;

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{
    CoverageResponse, ExchangeCoverage, Incident, IncidentsResponse, LatencyResponse, SlaResponse,
    StatusResponse, SymbolCoverageResponse,
};

/// Timeout for data quality endpoints that aggregate across all exchanges/symbols.
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

    /// Get data coverage for all exchanges.
    ///
    /// This endpoint aggregates coverage data across all exchanges and data
    /// types, which can take longer than other queries. It uses a 120-second
    /// timeout instead of the default 30 seconds.
    pub async fn coverage(&self) -> Result<CoverageResponse> {
        self.http
            .get_with_timeout("/v1/data-quality/coverage", &[], Some(SLOW_ENDPOINT_TIMEOUT))
            .await
    }

    /// Get data coverage for a single exchange.
    pub async fn exchange_coverage(&self, exchange: &str) -> Result<ExchangeCoverage> {
        self.http
            .get(&format!("/v1/data-quality/coverage/{}", exchange), &[])
            .await
    }

    /// Get symbol-level coverage with gap detection.
    pub async fn symbol_coverage(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<SymbolCoverageResponse> {
        self.http
            .get(
                &format!("/v1/data-quality/coverage/{}/{}", exchange, symbol),
                &[],
            )
            .await
    }

    /// List data incidents.
    pub async fn list_incidents(
        &self,
        status: Option<&str>,
    ) -> Result<IncidentsResponse> {
        let mut qp = vec![];
        if let Some(s) = status {
            qp.push(("status", s.to_string()));
        }
        self.http.get("/v1/data-quality/incidents", &qp).await
    }

    /// Get a single incident by ID.
    pub async fn get_incident(&self, incident_id: &str) -> Result<Incident> {
        self.http
            .get(
                &format!("/v1/data-quality/incidents/{}", incident_id),
                &[],
            )
            .await
    }

    /// Get latency metrics across exchanges.
    pub async fn latency(&self) -> Result<LatencyResponse> {
        self.http.get("/v1/data-quality/latency", &[]).await
    }

    /// Get SLA compliance metrics.
    ///
    /// Uses a 120-second timeout (this endpoint can be slow when computing
    /// compliance across all data types).
    pub async fn sla(&self, month: Option<&str>) -> Result<SlaResponse> {
        let mut qp = vec![];
        if let Some(m) = month {
            qp.push(("month", m.to_string()));
        }
        self.http
            .get_with_timeout("/v1/data-quality/sla", &qp, Some(SLOW_ENDPOINT_TIMEOUT))
            .await
    }
}
