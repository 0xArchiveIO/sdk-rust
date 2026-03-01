/// Tick-level order book reconstructor for Lighter.xyz.
///
/// Maintains an in-memory representation of the order book and applies
/// incremental deltas to produce full L2 snapshots at each tick.
///
/// # Example
///
/// ```no_run
/// use oxarchive::OxArchive;
/// use oxarchive::orderbook_reconstructor::OrderBookReconstructor;
///
/// # async fn example() -> oxarchive::Result<()> {
/// let client = OxArchive::new("your-api-key")?;
/// let tick_data = client.lighter.orderbook.history_tick(
///     "BTC", 1704067200000_i64, 1704153600000_i64, None,
/// ).await?;
///
/// let mut reconstructor = OrderBookReconstructor::new();
/// let snapshots = reconstructor.reconstruct_all(
///     &tick_data.checkpoint, &tick_data.deltas, None,
/// );
/// for snapshot in &snapshots {
///     println!("{}: mid={:?}", snapshot.timestamp, snapshot.mid_price);
/// }
/// # Ok(())
/// # }
/// ```

use std::collections::HashMap;

use chrono::{TimeZone, Utc};

use crate::types::{
    OrderBook, OrderbookDelta, PriceLevel, ReconstructedOrderBook, ReconstructOptions,
};

#[derive(Debug, Clone)]
struct InternalLevel {
    price: f64,
    size: f64,
    orders: i64,
}

/// Convert an `f64` price to a stable `HashMap` key using its bit pattern.
fn price_key(price: f64) -> u64 {
    price.to_bits()
}

fn to_price_level(l: &InternalLevel) -> PriceLevel {
    PriceLevel {
        px: l.price.to_string(),
        sz: l.size.to_string(),
        n: l.orders,
    }
}

/// Reconstructs L2 order books from a checkpoint and incremental deltas.
///
/// The reconstructor uses `HashMap<u64, InternalLevel>` keyed by the bit
/// pattern of each price for O(1) insertions and deletions. Sorted output
/// is produced only when a snapshot is requested.
///
/// # Usage patterns
///
/// | Method | When to use |
/// |--------|-------------|
/// | [`reconstruct_all`](Self::reconstruct_all) | Need every intermediate state |
/// | [`reconstruct_final`](Self::reconstruct_final) | Only need the final state |
/// | [`initialize`](Self::initialize) + [`apply_delta`](Self::apply_delta) loop | Custom streaming logic |
/// | [`detect_gaps`](Self::detect_gaps) | Validate data integrity before reconstruction |
#[derive(Debug)]
pub struct OrderBookReconstructor {
    bids: HashMap<u64, InternalLevel>,
    asks: HashMap<u64, InternalLevel>,
    coin: String,
    last_timestamp: String,
    last_sequence: i64,
}

impl OrderBookReconstructor {
    /// Create a new empty reconstructor.
    pub fn new() -> Self {
        Self {
            bids: HashMap::new(),
            asks: HashMap::new(),
            coin: String::new(),
            last_timestamp: String::new(),
            last_sequence: 0,
        }
    }

    /// Initialize (or reset) the reconstructor from a full order book checkpoint.
    ///
    /// This clears any existing state and loads all price levels from the
    /// checkpoint snapshot.
    pub fn initialize(&mut self, checkpoint: &OrderBook) {
        self.bids.clear();
        self.asks.clear();
        self.coin = checkpoint.coin.clone();
        self.last_timestamp = checkpoint.timestamp.clone();
        self.last_sequence = 0;

        for level in &checkpoint.bids {
            if let (Ok(px), Ok(sz)) = (level.px.parse::<f64>(), level.sz.parse::<f64>()) {
                self.bids.insert(
                    price_key(px),
                    InternalLevel {
                        price: px,
                        size: sz,
                        orders: level.n,
                    },
                );
            }
        }

        for level in &checkpoint.asks {
            if let (Ok(px), Ok(sz)) = (level.px.parse::<f64>(), level.sz.parse::<f64>()) {
                self.asks.insert(
                    price_key(px),
                    InternalLevel {
                        price: px,
                        size: sz,
                        orders: level.n,
                    },
                );
            }
        }
    }

    /// Apply a single delta to the internal order book state.
    ///
    /// - `size == 0.0` removes the price level.
    /// - `size > 0.0` inserts or updates the level.
    pub fn apply_delta(&mut self, delta: &OrderbookDelta) {
        let book = if delta.side == "bid" {
            &mut self.bids
        } else {
            &mut self.asks
        };

        let key = price_key(delta.price);
        if delta.size == 0.0 {
            book.remove(&key);
        } else {
            book.insert(
                key,
                InternalLevel {
                    price: delta.price,
                    size: delta.size,
                    orders: 1,
                },
            );
        }

        // Convert ms timestamp to ISO 8601.
        if let Some(dt) = Utc.timestamp_millis_opt(delta.timestamp).single() {
            self.last_timestamp = dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        }
        self.last_sequence = delta.sequence;
    }

    /// Build a snapshot of the current order book state.
    ///
    /// Bids are sorted descending (best bid first), asks ascending (best ask
    /// first). If `depth` is specified, only the top N levels per side are
    /// included.
    pub fn get_snapshot(&self, depth: Option<usize>) -> ReconstructedOrderBook {
        let mut sorted_bids: Vec<&InternalLevel> = self.bids.values().collect();
        sorted_bids
            .sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));

        let mut sorted_asks: Vec<&InternalLevel> = self.asks.values().collect();
        sorted_asks
            .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(d) = depth {
            sorted_bids.truncate(d);
            sorted_asks.truncate(d);
        }

        let bids: Vec<PriceLevel> = sorted_bids.iter().map(|l| to_price_level(l)).collect();
        let asks: Vec<PriceLevel> = sorted_asks.iter().map(|l| to_price_level(l)).collect();

        let best_bid = sorted_bids.first().map(|l| l.price);
        let best_ask = sorted_asks.first().map(|l| l.price);

        let (mid_price, spread, spread_bps) = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => {
                let mid = (bid + ask) / 2.0;
                let sp = ask - bid;
                let bps = if mid > 0.0 {
                    (sp / mid) * 10000.0
                } else {
                    0.0
                };
                (
                    Some(mid.to_string()),
                    Some(sp.to_string()),
                    Some(format!("{bps:.2}")),
                )
            }
            _ => (None, None, None),
        };

        ReconstructedOrderBook {
            coin: self.coin.clone(),
            timestamp: self.last_timestamp.clone(),
            bids,
            asks,
            mid_price,
            spread,
            spread_bps,
            sequence: if self.last_sequence > 0 {
                Some(self.last_sequence)
            } else {
                None
            },
        }
    }

    /// Reconstruct all snapshots from a checkpoint and deltas.
    ///
    /// - `emit_all = true` (default): returns 1 + N snapshots (initial + one
    ///   per delta).
    /// - `emit_all = false`: returns only the final state (1 snapshot).
    pub fn reconstruct_all(
        &mut self,
        checkpoint: &OrderBook,
        deltas: &[OrderbookDelta],
        options: Option<ReconstructOptions>,
    ) -> Vec<ReconstructedOrderBook> {
        let opts = options.unwrap_or_default();
        let mut snapshots = Vec::new();

        self.initialize(checkpoint);

        let mut sorted_deltas: Vec<&OrderbookDelta> = deltas.iter().collect();
        sorted_deltas.sort_by_key(|d| d.sequence);

        if opts.emit_all {
            snapshots.push(self.get_snapshot(opts.depth));
        }

        for delta in sorted_deltas {
            self.apply_delta(delta);
            if opts.emit_all {
                snapshots.push(self.get_snapshot(opts.depth));
            }
        }

        if !opts.emit_all {
            snapshots.push(self.get_snapshot(opts.depth));
        }

        snapshots
    }

    /// Reconstruct only the final state after applying all deltas.
    ///
    /// More efficient than [`reconstruct_all`](Self::reconstruct_all) when
    /// intermediate snapshots are not needed — avoids sorting price levels
    /// for every delta.
    pub fn reconstruct_final(
        &mut self,
        checkpoint: &OrderBook,
        deltas: &[OrderbookDelta],
        depth: Option<usize>,
    ) -> ReconstructedOrderBook {
        self.initialize(checkpoint);

        let mut sorted_deltas: Vec<&OrderbookDelta> = deltas.iter().collect();
        sorted_deltas.sort_by_key(|d| d.sequence);

        for delta in sorted_deltas {
            self.apply_delta(delta);
        }

        self.get_snapshot(depth)
    }

    /// Check for sequence gaps in a set of deltas.
    ///
    /// Returns a list of `(expected_sequence, actual_sequence)` pairs. An
    /// empty vec means there are no gaps and the data is contiguous.
    ///
    /// # Example
    ///
    /// ```
    /// use oxarchive::orderbook_reconstructor::OrderBookReconstructor;
    /// use oxarchive::types::OrderbookDelta;
    ///
    /// let deltas = vec![
    ///     OrderbookDelta { timestamp: 1, side: "bid".into(), price: 100.0, size: 1.0, sequence: 1 },
    ///     OrderbookDelta { timestamp: 2, side: "bid".into(), price: 100.0, size: 2.0, sequence: 5 },
    /// ];
    /// let gaps = OrderBookReconstructor::detect_gaps(&deltas);
    /// assert_eq!(gaps, vec![(2, 5)]);
    /// ```
    pub fn detect_gaps(deltas: &[OrderbookDelta]) -> Vec<(i64, i64)> {
        if deltas.len() < 2 {
            return vec![];
        }

        let mut sorted: Vec<&OrderbookDelta> = deltas.iter().collect();
        sorted.sort_by_key(|d| d.sequence);

        let mut gaps = Vec::new();
        for i in 1..sorted.len() {
            let expected = sorted[i - 1].sequence + 1;
            let actual = sorted[i].sequence;
            if actual != expected {
                gaps.push((expected, actual));
            }
        }

        gaps
    }
}

impl Default for OrderBookReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function: reconstruct all snapshots from tick data.
///
/// Equivalent to creating an [`OrderBookReconstructor`] and calling
/// [`reconstruct_all`](OrderBookReconstructor::reconstruct_all).
pub fn reconstruct_orderbook(
    checkpoint: &OrderBook,
    deltas: &[OrderbookDelta],
    options: Option<ReconstructOptions>,
) -> Vec<ReconstructedOrderBook> {
    let mut r = OrderBookReconstructor::new();
    r.reconstruct_all(checkpoint, deltas, options)
}

/// Convenience function: reconstruct only the final state from tick data.
///
/// Equivalent to creating an [`OrderBookReconstructor`] and calling
/// [`reconstruct_final`](OrderBookReconstructor::reconstruct_final).
pub fn reconstruct_final(
    checkpoint: &OrderBook,
    deltas: &[OrderbookDelta],
    depth: Option<usize>,
) -> ReconstructedOrderBook {
    let mut r = OrderBookReconstructor::new();
    r.reconstruct_final(checkpoint, deltas, depth)
}
