use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::orderbook_reconstructor::OrderBookReconstructor;
use crate::types::{
    CursorResponse, LighterGranularity, OrderBook, OrderbookDelta, ReconstructOptions,
    ReconstructedOrderBook, TickData, Timestamp,
};

/// Parameters for fetching a single orderbook snapshot.
#[derive(Debug, Default)]
pub struct GetOrderBookParams {
    /// Optional Unix-ms timestamp to fetch a historical snapshot.
    pub timestamp: Option<Timestamp>,
    /// Number of price levels per side.
    pub depth: Option<i32>,
}

/// Parameters for paginated orderbook history.
#[derive(Debug)]
pub struct OrderBookHistoryParams {
    pub start: Timestamp,
    pub end: Timestamp,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub depth: Option<i32>,
    /// Lighter.xyz only: snapshot granularity.
    pub granularity: Option<LighterGranularity>,
}

/// Access to order book endpoints for a specific exchange.
#[derive(Debug, Clone)]
pub struct OrderBookResource {
    http: HttpClient,
    prefix: String,
}

impl OrderBookResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get the current (or point-in-time) orderbook for a symbol.
    pub async fn get(&self, symbol: &str, params: Option<GetOrderBookParams>) -> Result<OrderBook> {
        let p = params.unwrap_or_default();
        let mut qp = vec![];
        if let Some(ts) = p.timestamp {
            qp.push(("timestamp", ts.to_millis().to_string()));
        }
        if let Some(d) = p.depth {
            qp.push(("depth", d.to_string()));
        }
        self.http
            .get(&format!("{}/orderbook/{}", self.prefix, symbol), &qp)
            .await
    }

    /// Get paginated historical orderbook snapshots.
    pub async fn history(
        &self,
        symbol: &str,
        params: OrderBookHistoryParams,
    ) -> Result<CursorResponse<Vec<OrderBook>>> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        if let Some(l) = params.limit {
            qp.push(("limit", l.to_string()));
        }
        if let Some(d) = params.depth {
            qp.push(("depth", d.to_string()));
        }
        if let Some(g) = params.granularity {
            qp.push(("granularity", g.as_str().to_string()));
        }
        let (data, next_cursor) = self
            .http
            .get_with_cursor(&format!("{}/orderbook/{}/history", self.prefix, symbol), &qp)
            .await?;
        Ok(CursorResponse { data, next_cursor })
    }

    /// Fetch tick-level orderbook data (checkpoint + deltas).
    ///
    /// **Requires Enterprise tier.** Returns a full L2 checkpoint at the start
    /// of the range plus every incremental delta within the range (up to ~1000
    /// deltas per request). Use this with [`OrderBookReconstructor`] for
    /// maximum control, or call [`history_reconstructed`](Self::history_reconstructed)
    /// for a one-shot convenience method.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParam`] if the account is not on the Enterprise
    /// tier (the API will return snapshot-level data instead of tick data).
    pub async fn history_tick(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        depth: Option<i32>,
    ) -> Result<TickData> {
        let mut qp = vec![
            ("start", start.into().to_millis().to_string()),
            ("end", end.into().to_millis().to_string()),
            ("granularity", "tick".to_string()),
        ];
        if let Some(d) = depth {
            qp.push(("depth", d.to_string()));
        }

        let value: serde_json::Value = self
            .http
            .get(&format!("{}/orderbook/{}/history", self.prefix, symbol), &qp)
            .await?;

        // Tick-level responses are objects with "checkpoint" + "deltas".
        // Non-Enterprise tiers receive an array of snapshots instead.
        let obj = value.as_object().ok_or_else(|| {
            Error::InvalidParam(
                "Tick-level orderbook data requires Enterprise tier. \
                 See https://0xarchive.io/pricing for details."
                    .into(),
            )
        })?;

        if !obj.contains_key("checkpoint") {
            return Err(Error::InvalidParam(
                "Tick-level orderbook data requires Enterprise tier. \
                 See https://0xarchive.io/pricing for details."
                    .into(),
            ));
        }

        let checkpoint: OrderBook = serde_json::from_value(obj["checkpoint"].clone())
            .map_err(|e| Error::Deserialize(format!("Failed to parse checkpoint: {e}")))?;

        let deltas: Vec<OrderbookDelta> = obj
            .get("deltas")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .unwrap_or_default();

        Ok(TickData {
            checkpoint,
            deltas,
        })
    }

    /// Fetch tick-level data and reconstruct orderbook snapshots (single page).
    ///
    /// **Requires Enterprise tier.** This is a convenience wrapper that calls
    /// [`history_tick`](Self::history_tick) and runs reconstruction in one step.
    ///
    /// - `emit_all = true` (default): returns one snapshot per delta plus the
    ///   initial checkpoint.
    /// - `emit_all = false`: returns only the final state.
    pub async fn history_reconstructed(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        depth: Option<i32>,
        emit_all: bool,
    ) -> Result<Vec<ReconstructedOrderBook>> {
        let tick_data = self.history_tick(symbol, start, end, depth).await?;
        let mut reconstructor = OrderBookReconstructor::new();
        let options = ReconstructOptions {
            depth: depth.map(|d| d as usize),
            emit_all,
        };
        Ok(reconstructor.reconstruct_all(&tick_data.checkpoint, &tick_data.deltas, Some(options)))
    }

    /// Fetch and reconstruct tick-level orderbook history with automatic
    /// pagination.
    ///
    /// **Requires Enterprise tier.** Fetches up to ~1000 deltas per API call,
    /// automatically advancing the cursor until the entire time range is
    /// covered. Returns all reconstructed snapshots (one per tick) as a
    /// single `Vec`.
    ///
    /// For very large time ranges this may use significant memory. Consider
    /// using [`history_tick`](Self::history_tick) in a manual loop for
    /// streaming-style processing.
    pub async fn collect_tick_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        depth: Option<i32>,
    ) -> Result<Vec<ReconstructedOrderBook>> {
        let start_ts = start.into().to_millis();
        let end_ts = end.into().to_millis();
        let depth_usize = depth.map(|d| d as usize);
        let max_deltas_per_page = 1000;

        let mut cursor = start_ts;
        let mut reconstructor = OrderBookReconstructor::new();
        let mut all_snapshots = Vec::new();
        let mut is_first_page = true;

        while cursor < end_ts {
            let tick_data = self.history_tick(symbol, cursor, end_ts, depth).await?;

            if tick_data.deltas.is_empty() {
                if is_first_page {
                    reconstructor.initialize(&tick_data.checkpoint);
                    all_snapshots.push(reconstructor.get_snapshot(depth_usize));
                }
                break;
            }

            // Re-initialize from this page's checkpoint.
            reconstructor.initialize(&tick_data.checkpoint);

            let mut sorted_deltas: Vec<&OrderbookDelta> = tick_data.deltas.iter().collect();
            sorted_deltas.sort_by_key(|d| d.sequence);

            // On the first page, include the initial checkpoint snapshot.
            // On subsequent pages, skip it to avoid duplicates.
            if is_first_page {
                all_snapshots.push(reconstructor.get_snapshot(depth_usize));
            }

            for delta in &sorted_deltas {
                reconstructor.apply_delta(delta);
                all_snapshots.push(reconstructor.get_snapshot(depth_usize));
            }

            is_first_page = false;

            // Advance cursor past the last delta's timestamp.
            let last_delta = sorted_deltas.last().unwrap();
            cursor = last_delta.timestamp + 1;

            // Fewer than max deltas means we've reached the end of the range.
            if tick_data.deltas.len() < max_deltas_per_page {
                break;
            }
        }

        Ok(all_snapshots)
    }
}
