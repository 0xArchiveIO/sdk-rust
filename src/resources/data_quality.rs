use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{
    CoverageResponse, ExchangeCoverage, Incident, IncidentsResponse, LatencyResponse, SlaResponse,
    StatusResponse, SymbolCoverageResponse,
};

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
    pub async fn coverage(&self) -> Result<CoverageResponse> {
        self.http.get("/v1/data-quality/coverage", &[]).await
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
    pub async fn sla(&self, month: Option<&str>) -> Result<SlaResponse> {
        let mut qp = vec![];
        if let Some(m) = month {
            qp.push(("month", m.to_string()));
        }
        self.http.get("/v1/data-quality/sla", &qp).await
    }
}
