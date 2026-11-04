//! The positioning cross-section for Hyperliquid core and HIP-3 perps.
//! Build, Pro, Scale and Enterprise plans.

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{MetaResponse, PositioningReading, PositioningWindow};

/// Every listed perp over a trailing window, as
/// `client.hyperliquid.positioning` (core perps) and
/// `client.hyperliquid.hip3.positioning` (HIP-3 perps): its open interest
/// change, taker buy and sell volume and liquidations, and its typical move
/// over windows of the same length, in one call.
///
/// Served for the Build, Pro, Scale and Enterprise plans; on Free the API
/// answers 403 and the error carries
/// [`PlanRequired`](crate::error::PlanRequired).
///
/// The current window only: there is no history, no paging and no symbol
/// filter (filter the records yourself). One record per listed perp, sorted
/// by symbol; delisted perps are not included. The window ends at the newest
/// 5-minute close, about 4 minutes behind now, which `meta.as_of` names.
/// Open interest and marks are `None` when a 5-minute bucket has no reading,
/// never 0. The typical moves are the median absolute move over windows of
/// the same length sliding by 5 minutes over about the last 6.9 days, and
/// are `None` under 288 counted windows.
///
/// A 503 `upstream_unavailable` ("still being prepared") is retryable.
#[derive(Debug, Clone)]
pub struct PositioningResource {
    http: HttpClient,
    prefix: String,
}

impl PositioningResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get every listed perp over the trailing `window` ending at the newest
    /// 5-minute close. `meta.as_of` is that close.
    pub async fn current(
        &self,
        window: PositioningWindow,
    ) -> Result<MetaResponse<Vec<PositioningReading>>> {
        self.http
            .get_with_cursor(
                &format!("{}/positioning", self.prefix),
                &[("window", window.as_str().to_string())],
            )
            .await
    }
}
