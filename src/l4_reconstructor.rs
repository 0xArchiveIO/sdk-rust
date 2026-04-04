//! L4 order book reconstructor with matching engine.
//!
//! Reconstructs Hyperliquid and HIP-3 L4 order books from checkpoints and diffs.
//! The same struct works for both exchanges — the diff format is identical.
//!
//! When a new order crosses the spread, the matching engine filled opposite-side
//! orders at crossing prices. Without removing them, the reconstructed book will
//! be "crossed" (best bid > best ask).
//!
//! Ref: <https://github.com/hyperliquid-dex/order_book_server>

use std::collections::{HashMap, HashSet};

/// A single L4 order in the reconstructed book.
#[derive(Debug, Clone)]
pub struct L4Order {
    pub oid: u64,
    pub user_address: String,
    pub side: String, // "B" or "A"
    pub price: f64,
    pub size: f64,
}

/// An aggregated L2 price level.
#[derive(Debug, Clone)]
pub struct L2Level {
    pub px: f64,
    pub sz: f64,
    pub n: usize,
}

/// An L4 diff to apply to the order book.
#[derive(Debug, Clone)]
pub struct L4Diff {
    pub diff_type: String, // "new", "update", "remove"
    pub oid: u64,
    pub side: String,
    pub price: f64,
    pub new_size: Option<f64>,
    pub user_address: String,
    pub block_number: u64,
}

/// L4 orderbook reconstructor with matching engine.
///
/// Works identically for Hyperliquid and HIP-3.
///
/// # Example
/// ```no_run
/// use oxarchive::L4OrderBookReconstructor;
///
/// let mut book = L4OrderBookReconstructor::new();
/// // book.load_checkpoint(&checkpoint);
/// // for diff in &diffs { book.apply_diff(diff, non_resting.get(&diff.block_number)); }
/// // assert!(!book.is_crossed());
/// ```
pub struct L4OrderBookReconstructor {
    orders: HashMap<u64, L4Order>,
    bid_prices: HashMap<u64, HashSet<u64>>, // price_bits -> oid set
    ask_prices: HashMap<u64, HashSet<u64>>,
}

fn price_key(p: f64) -> u64 {
    p.to_bits()
}

impl L4OrderBookReconstructor {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            bid_prices: HashMap::new(),
            ask_prices: HashMap::new(),
        }
    }

    /// Initialize from an L4 checkpoint (JSON parsed into bids/asks arrays).
    pub fn load_checkpoint_raw(&mut self, bids: &[L4Order], asks: &[L4Order]) {
        self.orders.clear();
        self.bid_prices.clear();
        self.ask_prices.clear();

        for order in bids.iter().chain(asks.iter()) {
            self.orders.insert(order.oid, order.clone());
            let pk = price_key(order.price);
            if order.side == "B" {
                self.bid_prices.entry(pk).or_default().insert(order.oid);
            } else {
                self.ask_prices.entry(pk).or_default().insert(order.oid);
            }
        }
    }

    /// Apply a single L4 diff with matching engine.
    pub fn apply_diff(&mut self, diff: &L4Diff, non_resting_oids: Option<&HashSet<u64>>) {
        match diff.diff_type.as_str() {
            "new" => {
                if let Some(nr) = non_resting_oids {
                    if nr.contains(&diff.oid) {
                        return;
                    }
                }
                let sz = match diff.new_size {
                    Some(s) if s > 0.0 => s,
                    _ => return,
                };

                // Matching engine: remove crossing opposite-side orders
                if diff.side == "B" {
                    let to_remove: Vec<u64> = self
                        .ask_prices
                        .keys()
                        .filter(|&&pk| f64::from_bits(pk) <= diff.price)
                        .copied()
                        .collect();
                    for pk in to_remove {
                        if let Some(oids) = self.ask_prices.remove(&pk) {
                            for oid in oids {
                                self.orders.remove(&oid);
                            }
                        }
                    }
                } else {
                    let to_remove: Vec<u64> = self
                        .bid_prices
                        .keys()
                        .filter(|&&pk| f64::from_bits(pk) >= diff.price)
                        .copied()
                        .collect();
                    for pk in to_remove {
                        if let Some(oids) = self.bid_prices.remove(&pk) {
                            for oid in oids {
                                self.orders.remove(&oid);
                            }
                        }
                    }
                }

                self.orders.insert(
                    diff.oid,
                    L4Order {
                        oid: diff.oid,
                        user_address: diff.user_address.clone(),
                        side: diff.side.clone(),
                        price: diff.price,
                        size: sz,
                    },
                );
                let pk = price_key(diff.price);
                if diff.side == "B" {
                    self.bid_prices.entry(pk).or_default().insert(diff.oid);
                } else {
                    self.ask_prices.entry(pk).or_default().insert(diff.oid);
                }
            }
            "update" => {
                if let Some(order) = self.orders.get_mut(&diff.oid) {
                    if let Some(sz) = diff.new_size {
                        order.size = sz;
                    }
                }
            }
            "remove" => {
                if let Some(order) = self.orders.remove(&diff.oid) {
                    let pk = price_key(order.price);
                    let map = if order.side == "B" {
                        &mut self.bid_prices
                    } else {
                        &mut self.ask_prices
                    };
                    if let Some(oids) = map.get_mut(&pk) {
                        oids.remove(&diff.oid);
                        if oids.is_empty() {
                            map.remove(&pk);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Return bids sorted by price descending.
    pub fn bids(&self) -> Vec<&L4Order> {
        let mut v: Vec<&L4Order> = self
            .orders
            .values()
            .filter(|o| o.side == "B" && o.size > 0.0)
            .collect();
        v.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    /// Return asks sorted by price ascending.
    pub fn asks(&self) -> Vec<&L4Order> {
        let mut v: Vec<&L4Order> = self
            .orders
            .values()
            .filter(|o| o.side == "A" && o.size > 0.0)
            .collect();
        v.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids().first().map(|o| o.price)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks().first().map(|o| o.price)
    }

    /// Check if the book is crossed. Should be false after correct reconstruction.
    pub fn is_crossed(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some(bb), Some(ba)) => bb >= ba,
            _ => false,
        }
    }

    pub fn bid_count(&self) -> usize {
        self.orders.values().filter(|o| o.side == "B" && o.size > 0.0).count()
    }

    pub fn ask_count(&self) -> usize {
        self.orders.values().filter(|o| o.side == "A" && o.size > 0.0).count()
    }

    /// Aggregate L4 orders into L2 price levels.
    pub fn derive_l2(&self) -> (Vec<L2Level>, Vec<L2Level>) {
        let mut bid_agg: HashMap<u64, (f64, usize)> = HashMap::new();
        let mut ask_agg: HashMap<u64, (f64, usize)> = HashMap::new();

        for o in self.orders.values() {
            if o.size <= 0.0 {
                continue;
            }
            let pk = price_key(o.price);
            let agg = if o.side == "B" { &mut bid_agg } else { &mut ask_agg };
            let entry = agg.entry(pk).or_insert((0.0, 0));
            entry.0 += o.size;
            entry.1 += 1;
        }

        let mut l2_bids: Vec<L2Level> = bid_agg
            .iter()
            .map(|(&pk, &(sz, n))| L2Level { px: f64::from_bits(pk), sz, n })
            .collect();
        l2_bids.sort_by(|a, b| b.px.partial_cmp(&a.px).unwrap_or(std::cmp::Ordering::Equal));

        let mut l2_asks: Vec<L2Level> = ask_agg
            .iter()
            .map(|(&pk, &(sz, n))| L2Level { px: f64::from_bits(pk), sz, n })
            .collect();
        l2_asks.sort_by(|a, b| a.px.partial_cmp(&b.px).unwrap_or(std::cmp::Ordering::Equal));

        (l2_bids, l2_asks)
    }
}

impl Default for L4OrderBookReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_checkpoint() -> (Vec<L4Order>, Vec<L4Order>) {
        let bids = vec![
            L4Order { oid: 1, user_address: "0xAAA".into(), side: "B".into(), price: 100.0, size: 5.0 },
            L4Order { oid: 2, user_address: "0xBBB".into(), side: "B".into(), price: 99.0, size: 3.0 },
            L4Order { oid: 3, user_address: "0xCCC".into(), side: "B".into(), price: 98.0, size: 2.0 },
        ];
        let asks = vec![
            L4Order { oid: 4, user_address: "0xDDD".into(), side: "A".into(), price: 101.0, size: 4.0 },
            L4Order { oid: 5, user_address: "0xEEE".into(), side: "A".into(), price: 102.0, size: 6.0 },
            L4Order { oid: 6, user_address: "0xFFF".into(), side: "A".into(), price: 103.0, size: 1.0 },
        ];
        (bids, asks)
    }

    fn new_diff(oid: u64, side: &str, price: f64, new_size: Option<f64>, dt: &str) -> L4Diff {
        L4Diff {
            diff_type: dt.to_string(),
            oid,
            side: side.to_string(),
            price,
            new_size,
            user_address: String::new(),
            block_number: 1,
        }
    }

    #[test]
    fn test_load_checkpoint() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);
        assert_eq!(book.bid_count(), 3);
        assert_eq!(book.ask_count(), 3);
        assert_eq!(book.best_bid(), Some(100.0));
        assert_eq!(book.best_ask(), Some(101.0));
        assert!(!book.is_crossed());
    }

    #[test]
    fn test_bids_descending_asks_ascending() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        let bid_prices: Vec<f64> = book.bids().iter().map(|o| o.price).collect();
        assert_eq!(bid_prices, vec![100.0, 99.0, 98.0]);

        let ask_prices: Vec<f64> = book.asks().iter().map(|o| o.price).collect();
        assert_eq!(ask_prices, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn test_new_non_crossing() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        book.apply_diff(&new_diff(10, "B", 99.5, Some(1.0), "new"), None);
        assert_eq!(book.bid_count(), 4);
        assert_eq!(book.ask_count(), 3);
        assert!(!book.is_crossed());
    }

    #[test]
    fn test_crossing_bid_removes_asks() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        // Bid at 102 crosses asks at 101 and 102
        book.apply_diff(&new_diff(10, "B", 102.0, Some(0.5), "new"), None);
        assert_eq!(book.best_ask(), Some(103.0));
        assert_eq!(book.ask_count(), 1);
        assert!(!book.is_crossed());
    }

    #[test]
    fn test_crossing_ask_removes_bids() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        // Ask at 99 crosses bids at 100 and 99
        book.apply_diff(&new_diff(10, "A", 99.0, Some(0.5), "new"), None);
        assert_eq!(book.best_bid(), Some(98.0));
        assert_eq!(book.bid_count(), 1);
        assert!(!book.is_crossed());
    }

    #[test]
    fn test_update_order() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        book.apply_diff(&new_diff(1, "B", 100.0, Some(10.0), "update"), None);
        let b = book.bids();
        let updated = b.iter().find(|o| o.oid == 1).unwrap();
        assert_eq!(updated.size, 10.0);
    }

    #[test]
    fn test_remove_order() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        book.apply_diff(&new_diff(1, "B", 100.0, None, "remove"), None);
        assert_eq!(book.bid_count(), 2);
        assert_eq!(book.best_bid(), Some(99.0));
    }

    #[test]
    fn test_non_resting_filter() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        let mut nr = HashSet::new();
        nr.insert(10u64);
        book.apply_diff(&new_diff(10, "B", 100.5, Some(1.0), "new"), Some(&nr));
        assert_eq!(book.bid_count(), 3); // unchanged
    }

    #[test]
    fn test_multiple_crossings_never_crossed() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        for (i, px) in [101.0, 102.0, 103.0, 105.0].iter().enumerate() {
            book.apply_diff(&new_diff(100 + i as u64, "B", *px, Some(0.1), "new"), None);
            assert!(!book.is_crossed(), "Crossed after buy at {}", px);
        }

        for (i, px) in [99.0, 98.0, 95.0].iter().enumerate() {
            book.apply_diff(&new_diff(200 + i as u64, "A", *px, Some(0.1), "new"), None);
            assert!(!book.is_crossed(), "Crossed after sell at {}", px);
        }
    }

    #[test]
    fn test_exact_price_crossing() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        book.apply_diff(&new_diff(10, "B", 101.0, Some(0.5), "new"), None);
        assert!(!book.is_crossed());
        assert_eq!(book.best_ask(), Some(102.0));
    }

    #[test]
    fn test_derive_l2_basic() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        let (l2_bids, l2_asks) = book.derive_l2();
        assert_eq!(l2_bids.len(), 3);
        assert_eq!(l2_asks.len(), 3);
        assert_eq!(l2_bids[0].px, 100.0);
        assert_eq!(l2_bids[0].n, 1);
    }

    #[test]
    fn test_derive_l2_aggregation() {
        let mut book = L4OrderBookReconstructor::new();
        let bids = vec![
            L4Order { oid: 1, user_address: String::new(), side: "B".into(), price: 100.0, size: 2.0 },
            L4Order { oid: 2, user_address: String::new(), side: "B".into(), price: 100.0, size: 3.0 },
        ];
        let asks = vec![
            L4Order { oid: 3, user_address: String::new(), side: "A".into(), price: 101.0, size: 1.0 },
        ];
        book.load_checkpoint_raw(&bids, &asks);

        let (l2_bids, _) = book.derive_l2();
        assert_eq!(l2_bids.len(), 1);
        assert_eq!(l2_bids[0].sz, 5.0);
        assert_eq!(l2_bids[0].n, 2);
    }

    #[test]
    fn test_derive_l2_not_crossed_after_matching() {
        let mut book = L4OrderBookReconstructor::new();
        let (bids, asks) = make_checkpoint();
        book.load_checkpoint_raw(&bids, &asks);

        book.apply_diff(&new_diff(10, "B", 102.0, Some(1.0), "new"), None);
        let (l2_bids, l2_asks) = book.derive_l2();
        assert!(l2_bids[0].px < l2_asks[0].px, "L2 should not be crossed");
    }
}
