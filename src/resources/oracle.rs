//! HIP-3 oracle reads: the deployer-pushed external price and the
//! instantaneous discovery bounds.

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{Hip3OracleDiscoveryBounds, Hip3OracleExternalPrice};

/// Access to the HIP-3 oracle routes (`/v1/hyperliquid/hip3/oracle`), as
/// `client.hyperliquid.hip3.oracle`.
///
/// Symbols keep their builder prefix and case (`xyz:XYZ100`, `km:US500`).
#[derive(Debug, Clone)]
pub struct OracleResource {
    http: HttpClient,
    prefix: String,
}

impl OracleResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the latest deployer-pushed external reference price and mark
    /// price of a HIP-3 market
    /// (`GET /v1/hyperliquid/hip3/oracle/external-price/{symbol}`).
    pub async fn external_price(&self, symbol: &str) -> Result<Hip3OracleExternalPrice> {
        self.http
            .get(
                &format!("{}/oracle/external-price/{}", self.prefix, symbol),
                &[],
            )
            .await
    }

    /// Get the instantaneous discovery bounds of a HIP-3 market, from the
    /// current reference price and the market's maximum leverage
    /// (`GET /v1/hyperliquid/hip3/oracle/discovery-bounds/{symbol}`).
    ///
    /// The full ratcheted range can be wider when deployer-specific reset
    /// configuration applies.
    pub async fn discovery_bounds(&self, symbol: &str) -> Result<Hip3OracleDiscoveryBounds> {
        self.http
            .get(
                &format!("{}/oracle/discovery-bounds/{}", self.prefix, symbol),
                &[],
            )
            .await
    }
}
