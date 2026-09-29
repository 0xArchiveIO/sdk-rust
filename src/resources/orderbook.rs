use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::orderbook_reconstructor::OrderBookReconstructor;
use crate::types::{
    CursorResponse, LighterGranularity, OrderBook, OrderbookDelta, ReconstructOptions,
    ReconstructedOrderBook, ResponseMeta, TickData, Timestamp,
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
    /// Price levels per side in each snapshot, on every venue (Hyperliquid,
    /// HIP-3, Spot and both Lighter deployments).
    pub depth: Option<i32>,
    /// Lighter only: snapshot granularity.
    pub granularity: Option<LighterGranularity>,
}

/// Range, depth and position for
/// [`OrderBookResource::history_tick_page`].
#[derive(Debug, Clone)]
pub struct TickPageParams {
    /// Start of the range. The first page opens at the checkpoint at or
    /// before it. Keep it unchanged while paging.
    pub start: Timestamp,
    /// End of the range. Keep it unchanged while paging.
    pub end: Timestamp,
    /// Price levels per side on the checkpoint.
    pub depth: Option<i32>,
    /// The previous page's `next_cursor`; `None` for the first page.
    pub cursor: Option<String>,
    /// The previous page's `next_cursor_seq`, sent with `cursor`.
    pub cursor_seq: Option<i64>,
}

impl TickPageParams {
    /// The first page of a range, with no depth limit.
    pub fn new(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            depth: None,
            cursor: None,
            cursor_seq: None,
        }
    }

    /// The same range and depth, positioned after `page`.
    pub fn after(self, page: &TickPage) -> Self {
        Self {
            cursor: page.next_cursor.clone(),
            cursor_seq: page.next_cursor_seq,
            ..self
        }
    }
}

/// One page of tick-level order book data, from
/// [`OrderBookResource::history_tick_page`].
#[derive(Debug, Clone)]
pub struct TickPage {
    /// The full book at the checkpoint the range opens at. Present on the
    /// first page only; later pages carry deltas to apply on top of the
    /// running book.
    pub checkpoint: Option<OrderBook>,
    /// Incremental changes, in `(timestamp, sequence)` order.
    pub deltas: Vec<OrderbookDelta>,
    /// Pass back as `cursor`, with `next_cursor_seq`, for the next page.
    pub next_cursor: Option<String>,
    /// Pass back as `cursor_seq` for the next page.
    pub next_cursor_seq: Option<i64>,
    /// `true` while more deltas follow in the range.
    pub has_more: bool,
    /// The full `meta` block.
    pub meta: ResponseMeta,
}

fn no_tick_data() -> Error {
    Error::InvalidParam(
        "Tick-level orderbook data was not returned for this request. \
         Check the symbol and time range, or use a different granularity."
            .into(),
    )
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
        self.http
            .get_with_cursor(&format!("{}/orderbook/{}/history", self.prefix, symbol), &qp)
            .await
    }

    /// Fetch tick-level orderbook data (checkpoint + deltas): the first page
    /// of the range.
    ///
    /// Returns the full L2 checkpoint at or before `start` plus the first
    /// page of incremental deltas after it (100 by default). Use this with
    /// [`OrderBookReconstructor`] for maximum control, call
    /// [`history_tick_page`](Self::history_tick_page) to page through the
    /// rest of the range, or [`collect_tick_history`](Self::collect_tick_history)
    /// to fetch and reconstruct all of it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParam`] if the API returns snapshot-level data
    /// instead of tick data for this request.
    pub async fn history_tick(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        depth: Option<i32>,
    ) -> Result<TickData> {
        let page = self
            .history_tick_page(
                symbol,
                TickPageParams {
                    start: start.into(),
                    end: end.into(),
                    depth,
                    cursor: None,
                    cursor_seq: None,
                },
            )
            .await?;
        let checkpoint = page.checkpoint.ok_or_else(no_tick_data)?;
        Ok(TickData {
            checkpoint,
            deltas: page.deltas,
        })
    }

    /// Fetch one page of tick-level orderbook data with its paging state.
    ///
    /// The first page (no `cursor`) carries the checkpoint at or before
    /// `start` and the first deltas after it. While `has_more` is `true`,
    /// request the next page with the same `start`, `end` and `depth` and the
    /// page's `next_cursor` and `next_cursor_seq`; continuation pages carry
    /// deltas only, to apply on top of the running book. A page holds a
    /// bounded number of deltas (100 by default), so a short page is not the
    /// end of the range; `has_more` is.
    ///
    /// ```no_run
    /// # use oxarchive::OxArchive;
    /// # use oxarchive::resources::orderbook::TickPageParams;
    /// # async fn example() -> oxarchive::Result<()> {
    /// # let client = OxArchive::new("key")?;
    /// let mut params = TickPageParams::new(1790553600000_i64, 1790553660000_i64);
    /// loop {
    ///     let page = client.lighter.orderbook.history_tick_page("BTC", params.clone()).await?;
    ///     println!("{} deltas", page.deltas.len());
    ///     if !page.has_more {
    ///         break;
    ///     }
    ///     params = params.after(&page);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn history_tick_page(
        &self,
        symbol: &str,
        params: TickPageParams,
    ) -> Result<TickPage> {
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
            ("granularity", "tick".to_string()),
        ];
        if let Some(d) = params.depth {
            qp.push(("depth", d.to_string()));
        }
        if let Some(c) = params.cursor {
            qp.push(("cursor", c));
        }
        if let Some(seq) = params.cursor_seq {
            qp.push(("cursor_seq", seq.to_string()));
        }

        let (value, meta): (serde_json::Value, ResponseMeta) = self
            .http
            .get_with_meta(&format!("{}/orderbook/{}/history", self.prefix, symbol), &qp)
            .await?;

        // Tick-level responses are objects with "checkpoint" + "deltas".
        // Some requests receive an array of snapshots instead.
        let obj = value.as_object().ok_or_else(no_tick_data)?;
        if !obj.contains_key("checkpoint") && !obj.contains_key("deltas") {
            return Err(no_tick_data());
        }

        let checkpoint: Option<OrderBook> = match obj.get("checkpoint") {
            None | Some(serde_json::Value::Null) => None,
            Some(c) => Some(
                serde_json::from_value(c.clone())
                    .map_err(|e| Error::Deserialize(format!("Failed to parse checkpoint: {e}")))?,
            ),
        };

        let deltas: Vec<OrderbookDelta> = match obj.get("deltas") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(d) => serde_json::from_value(d.clone())
                .map_err(|e| Error::Deserialize(format!("Failed to parse deltas: {e}")))?,
        };

        let next_cursor = obj
            .get("next_cursor")
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .or_else(|| meta.next_cursor.clone());
        let next_cursor_seq = obj.get("next_cursor_seq").and_then(|v| v.as_i64());
        let has_more = meta.has_more.unwrap_or(next_cursor.is_some());

        Ok(TickPage {
            checkpoint,
            deltas,
            next_cursor,
            next_cursor_seq,
            has_more,
            meta,
        })
    }

    /// Fetch tick-level data and reconstruct orderbook snapshots (single page).
    ///
    /// This is a convenience wrapper that calls
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
    /// Starts from the checkpoint at or before `start` and follows the
    /// response's cursor until `has_more` is `false`, applying every delta in
    /// order. Returns all reconstructed snapshots (the checkpoint, then one
    /// per delta) as a single `Vec`.
    ///
    /// For very large time ranges this may use significant memory. Consider
    /// using [`history_tick_page`](Self::history_tick_page) in a manual loop
    /// for streaming-style processing.
    pub async fn collect_tick_history(
        &self,
        symbol: &str,
        start: impl Into<Timestamp>,
        end: impl Into<Timestamp>,
        depth: Option<i32>,
    ) -> Result<Vec<ReconstructedOrderBook>> {
        let depth_usize = depth.map(|d| d as usize);
        let mut params = TickPageParams {
            depth,
            ..TickPageParams::new(start, end)
        };

        let mut reconstructor = OrderBookReconstructor::new();
        let mut all_snapshots = Vec::new();
        let mut is_first_page = true;

        loop {
            let page = self.history_tick_page(symbol, params.clone()).await?;

            if is_first_page {
                let checkpoint = page.checkpoint.as_ref().ok_or_else(no_tick_data)?;
                reconstructor.initialize(checkpoint);
                all_snapshots.push(reconstructor.get_snapshot(depth_usize));
                is_first_page = false;
            }

            let mut sorted_deltas: Vec<&OrderbookDelta> = page.deltas.iter().collect();
            sorted_deltas.sort_by_key(|d| (d.timestamp, d.sequence));
            for delta in sorted_deltas {
                reconstructor.apply_delta(delta);
                all_snapshots.push(reconstructor.get_snapshot(depth_usize));
            }

            // Stop on the server's `has_more`, and on a page that would not
            // move the cursor forward.
            let next = params.clone().after(&page);
            if !page.has_more
                || page.deltas.is_empty()
                || page.next_cursor.is_none()
                || (next.cursor == params.cursor && next.cursor_seq == params.cursor_seq)
            {
                break;
            }
            params = next;
        }

        Ok(all_snapshots)
    }
}
