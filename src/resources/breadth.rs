use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, Hip3BreadthSnapshot, OiFundingInterval, Timestamp};

/// Parameters for breadth-above-session-VWAP history (Hyperliquid core and
/// HIP-3).
#[derive(Debug, Default, Clone)]
pub struct BreadthHistoryParams {
    /// Inclusive range start in epoch milliseconds.
    pub start: Option<Timestamp>,
    /// Inclusive range end in epoch milliseconds.
    pub end: Option<Timestamp>,
    /// Last-snapshot-per-bucket downsampling interval.
    pub interval: Option<OiFundingInterval>,
    /// Exclusive epoch-millisecond cursor from `next_cursor`.
    pub cursor: Option<String>,
    /// Number of snapshots to return, from 1 through 1,000.
    pub limit: Option<i64>,
}

impl BreadthHistoryParams {
    fn to_query(&self) -> Result<Vec<(&'static str, String)>> {
        if let Some(limit) = self.limit {
            if !(1..=1_000).contains(&limit) {
                return Err(Error::InvalidParam(
                    "breadth history limit must be between 1 and 1000".to_string(),
                ));
            }
        }

        let mut query = Vec::new();
        if let Some(start) = &self.start {
            query.push(("start", start.to_millis().to_string()));
        }
        if let Some(end) = &self.end {
            query.push(("end", end.to_millis().to_string()));
        }
        if let Some(interval) = self.interval {
            query.push(("interval", interval.as_str().to_string()));
        }
        if let Some(cursor) = &self.cursor {
            query.push(("cursor", cursor.clone()));
        }
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        Ok(query)
    }
}

/// Typed access to market breadth above the current UTC-session VWAP, as
/// `client.hyperliquid.breadth` (core perps, aggregate only) and
/// `client.hyperliquid.hip3.breadth` (HIP-3, with per-namespace counts).
#[derive(Debug, Clone)]
pub struct BreadthResource {
    http: HttpClient,
    prefix: String,
}

impl BreadthResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the latest validated breadth snapshot.
    ///
    /// `value_pct` is `None` when no instrument is eligible. The snapshot
    /// compares the close of the most recently completed one-minute candle
    /// with the instrument's current UTC-session VWAP. HIP-3 history begins
    /// on 2026-08-28 and core history on 2026-08-24; no earlier snapshots are
    /// synthesized. Core snapshots always have empty `namespaces` maps.
    pub async fn current(&self) -> Result<Hip3BreadthSnapshot> {
        self.http
            .get(&format!("{}/breadth/above-vwap/current", self.prefix), &[])
            .await
    }

    /// Get ascending breadth history with cursor pagination.
    ///
    /// The server performs last-snapshot-per-bucket downsampling for the
    /// optional `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, or `1d` interval. Do not average
    /// `value_pct` values across snapshots because the eligible denominator
    /// varies with session volume and five-minute freshness exclusions.
    pub async fn history(
        &self,
        params: BreadthHistoryParams,
    ) -> Result<CursorResponse<Vec<Hip3BreadthSnapshot>>> {
        let query = params.to_query()?;
        self.http
            .get_with_cursor(&format!("{}/breadth/above-vwap", self.prefix), &query)
            .await
    }
}
