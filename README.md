# oxarchive

[![Crates.io](https://img.shields.io/crates/v/oxarchive.svg)](https://crates.io/crates/oxarchive) [![Docs.rs](https://docs.rs/oxarchive/badge.svg)](https://docs.rs/oxarchive) [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Rust client for async services that need typed 0xArchive market data.

0xArchive is granular market data infrastructure for Hyperliquid and Lighter.xyz. Hyperliquid includes core perps (`/v1/hyperliquid`), HIP-3 builder perps (`/v1/hyperliquid/hip3`), HIP-4 outcome markets (`/v1/hyperliquid/hip4`), and Hyperliquid Spot (`/v1/hyperliquid/spot`). Lighter.xyz is the second top-level venue API at `/v1/lighter`.

Use this SDK when the integration belongs in an async Rust service, data system, backtest runner, or strongly typed market-data pipeline.

## Installation

```bash
cargo add oxarchive
```

Or add directly to your `Cargo.toml`:

```toml
[dependencies]
oxarchive = "1.9"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

For WebSocket support (real-time streaming, replay, bulk download):

```toml
oxarchive = { version = "1.9", features = ["websocket"] }
```

## Quick Start

```rust
use oxarchive::OxArchive;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let client = OxArchive::new("0xa_your_api_key")?;

    // First successful call: Hyperliquid BTC order book
    let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
    println!("Hyperliquid BTC mid price: {:?}", ob.mid_price);

    // Lighter.xyz uses its own venue client
    let lighter_ob = client.lighter.orderbook.get("BTC", None).await?;
    println!("Lighter BTC mid price: {:?}", lighter_ob.mid_price);

    // Hyperliquid HIP-3 builder perps stay under client.hyperliquid.hip3
    let hip3 = client.hyperliquid.hip3.instruments.list().await?;
    let hip3_ob = client.hyperliquid.hip3.orderbook.get("km:US500", None).await?;
    let hip3_funding = client.hyperliquid.hip3.funding.current("xyz:XYZ100").await?;

    // Hyperliquid Spot stays under client.hyperliquid.spot (dashed canonical pairs)
    let pairs = client.hyperliquid.spot.pairs.list().await?;
    let hype_ob = client.hyperliquid.spot.orderbook.get("HYPE-USDC", None).await?;
    println!("HYPE-USDC mid price: {:?}", hype_ob.mid_price);

    // Historical order book snapshots
    use oxarchive::resources::orderbook::OrderBookHistoryParams;
    let history = client.hyperliquid.orderbook.history("ETH", OrderBookHistoryParams {
        start: 1704067200000_i64.into(),
        end: 1704153600000_i64.into(),
        cursor: None,
        limit: Some(100),
        depth: None,
        granularity: None,
    }).await?;

    Ok(())
}
```

## Choose Your Next Path

| Need | Link |
| --- | --- |
| First authenticated route | [Quick Start](https://www.0xarchive.io/docs/quick-start) |
| SDK install and route docs | [SDK docs](https://www.0xarchive.io/docs/sdks) |
| Claude Code, ChatGPT Codex, and coding-agent workflows | [AI Clients](https://www.0xarchive.io/docs/ai-clients) |
| File-based historical pulls | [Data Catalog](https://www.0xarchive.io/data) |
| Route contract and machine context | [OpenAPI](https://www.0xarchive.io/openapi.json), [llms.txt](https://www.0xarchive.io/llms.txt) |

## Use From Coding Agents

When prototyping Rust services from Claude Code, ChatGPT Codex, or another coding agent, also install the [0xArchive skill](https://github.com/0xArchiveIO/0xarchive-skill) so the agent has typed API context and example patterns for every endpoint. The skill installs into `.claude/skills/0xarchive` (Claude Code) or `.agents/skills/0xarchive` (ChatGPT Codex). For shell-driven exploration alongside the SDK, the [CLI](https://npmjs.com/package/@0xarchive/cli) and the [hosted MCP](https://mcp.0xarchive.io) share the same API key.

## Data Coverage

| Venue | Coverage | Notes |
| --- | --- | --- |
| Hyperliquid | April 2023+ | Perpetuals across the full venue |
| Hyperliquid HIP-3 | February 2026+ | All HIP-3 symbols, orderbook, and history on every tier. |
| Hyperliquid HIP-4 | May 2026+ | Binary outcome markets (`#0`, `#1`, ...). No funding, no liquidations, no candles. |
| Hyperliquid Spot | Trades March 2025+; orderbook, L4, TWAP, freshness live-only from May 2026 | 294 pairs (`HYPE-USDC`, `PURR-USDC`, ...). No funding, no OI, no liquidations, no candles. |
| Lighter.xyz | August 2025+ for fills; January 2026+ for orderbooks, open interest, funding rates | Perpetuals |

## Configuration

```rust
use oxarchive::OxArchive;
use std::time::Duration;

let client = OxArchive::builder("0xa_your_api_key")
    .base_url("https://api.0xarchive.io")  // Optional
    .timeout(Duration::from_secs(60))       // Optional (default: 30s)
    .build()?;
```

## REST API Reference

The sections below show which resources are available on each exchange client:

| Resource | `client.hyperliquid` | `client.hyperliquid.hip3` | `client.hyperliquid.spot` | `client.lighter` |
|----------|---------------------|--------------------------|-------------------------|-------------------|
| `orderbook` | Yes | Yes | Yes | Yes |
| `trades` | Yes | Yes | Yes | Yes |
| `instruments` | Yes | Yes | -- (use `pairs`) | Yes |
| `pairs` | -- | -- | Yes | -- |
| `funding` | Yes | Yes | -- | Yes |
| `open_interest` | Yes | Yes | -- | Yes |
| `candles` | Yes | Yes | -- | Yes |
| `liquidations` | Yes | Yes | -- | -- |
| `orders` | Yes | Yes | Yes | -- |
| `l4_orderbook` | Yes | Yes | Yes | -- |
| `l2_orderbook` | Yes | Yes | -- | -- |
| `l3_orderbook` | -- | -- | -- | Yes |
| `twap` | -- | -- | Yes | -- |
| `freshness()` | Yes | Yes | Yes | Yes |
| `summary()` | Yes | Yes | -- | Yes |
| `price_history()` | Yes | Yes | -- | Yes |

### Order Book

```rust
use oxarchive::resources::orderbook::{GetOrderBookParams, OrderBookHistoryParams};

// Get current order book
let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
println!("Mid price: {:?}", ob.mid_price);
println!("Best bid: {:?}", ob.bids.first());
println!("Best ask: {:?}", ob.asks.first());

// Get with specific timestamp and depth
let ob = client.hyperliquid.orderbook.get("BTC", Some(GetOrderBookParams {
    timestamp: Some(1704067200000_i64.into()),
    depth: Some(20),
})).await?;

// Get historical snapshots
let history = client.hyperliquid.orderbook.history("BTC", OrderBookHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
    depth: None,
    granularity: None,
}).await?;
```

#### Orderbook Depth

Full orderbook depth is available on every tier.

**Note:** Hyperliquid L2 source data contains ~20 levels. Full-depth L2 (derived from L4) and Lighter.xyz provide full depth.

#### Lighter Orderbook Granularity

Lighter.xyz orderbook history supports a `granularity` parameter for different data resolutions:

```rust
use oxarchive::types::LighterGranularity;

let history = client.lighter.orderbook.history("BTC", OrderBookHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
    depth: None,
    granularity: Some(LighterGranularity::TenSeconds),
}).await?;
```

| Granularity | Interval | Credit Multiplier |
|-------------|----------|-------------------|
| `Checkpoint` | ~60s | 1x |
| `ThirtySeconds` | 30s | 2x |
| `TenSeconds` | 10s | 3x |
| `OneSecond` | 1s | 10x |
| `Tick` | tick-level | 20x |

### Orderbook Reconstruction

Tick-level orderbook history returns a full **checkpoint** plus **incremental deltas**, allowing you to reconstruct the exact state of the order book at every tick. This is the most granular data available and is ideal for backtesting, market microstructure research, and latency analysis.

```rust
use oxarchive::OxArchive;
use oxarchive::orderbook_reconstructor::OrderBookReconstructor;
use oxarchive::types::ReconstructOptions;

let client = OxArchive::new("your-api-key")?;

// Option 1: One-shot — fetch and reconstruct in one call
let snapshots = client.lighter.orderbook.history_reconstructed(
    "BTC",
    1704067200000_i64,  // start
    1704070800000_i64,  // end
    Some(20),           // depth (top 20 levels per side)
    true,               // emit_all: snapshot after every tick
).await?;

for snapshot in &snapshots {
    println!("{}: mid={:?}, spread={:?}bps, seq={:?}",
        snapshot.timestamp, snapshot.mid_price, snapshot.spread_bps, snapshot.sequence);
}

// Option 2: Auto-paginated — fetches all pages automatically
let all_snapshots = client.lighter.orderbook.collect_tick_history(
    "BTC",
    1704067200000_i64,
    1704153600000_i64,
    Some(20),
).await?;
println!("{} tick-level snapshots", all_snapshots.len());

// Option 3: Manual control — fetch raw tick data and reconstruct yourself
let tick_data = client.lighter.orderbook.history_tick(
    "BTC", 1704067200000_i64, 1704070800000_i64, None,
).await?;

// Check data integrity
let gaps = OrderBookReconstructor::detect_gaps(&tick_data.deltas);
if !gaps.is_empty() {
    eprintln!("Sequence gaps detected: {:?}", gaps);
}

// Reconstruct with full control
let mut reconstructor = OrderBookReconstructor::new();
reconstructor.initialize(&tick_data.checkpoint);

for delta in &tick_data.deltas {
    reconstructor.apply_delta(delta);
    let snapshot = reconstructor.get_snapshot(Some(10));
    // Process each snapshot...
}

// Or get just the final state (most efficient)
let final_state = reconstructor.reconstruct_final(
    &tick_data.checkpoint, &tick_data.deltas, Some(20),
);
println!("Final mid price: {:?}", final_state.mid_price);
```

#### Manual Pagination

For large time ranges where memory is a concern, paginate manually instead of using `collect_tick_history`:

```rust
let mut cursor = 1704067200000_i64;
let end = 1704153600000_i64;

while cursor < end {
    let tick_data = client.lighter.orderbook.history_tick(
        "BTC", cursor, end, Some(20),
    ).await?;

    if tick_data.deltas.is_empty() {
        break;
    }

    // Process this page...
    let mut reconstructor = OrderBookReconstructor::new();
    let final_state = reconstructor.reconstruct_final(
        &tick_data.checkpoint, &tick_data.deltas, Some(20),
    );
    println!("{}: {} bids, {} asks", final_state.timestamp, final_state.bids.len(), final_state.asks.len());

    // Advance cursor past the last delta
    let last_ts = tick_data.deltas.iter().map(|d| d.timestamp).max().unwrap();
    cursor = last_ts + 1;

    // Fewer than ~1000 deltas means end of range
    if tick_data.deltas.len() < 1000 {
        break;
    }
}
```

### Trades

Cursor-based pagination for efficient retrieval of large datasets.

```rust
use oxarchive::resources::trades::GetTradesParams;

// Get trades with pagination
let result = client.hyperliquid.trades.list("BTC", GetTradesParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    limit: Some(1000),
    cursor: None,
    side: None,
}).await?;

// Paginate through all results
let mut all_trades = result.data;
let mut cursor = result.next_cursor;
while let Some(c) = cursor {
    let page = client.hyperliquid.trades.list("BTC", GetTradesParams {
        start: 1704067200000_i64.into(),
        end: 1704153600000_i64.into(),
        limit: Some(1000),
        cursor: Some(c),
        side: None,
    }).await?;
    all_trades.extend(page.data);
    cursor = page.next_cursor;
}

// Get recent trades (Lighter and HIP-3 only)
let recent = client.lighter.trades.recent("BTC", Some(100)).await?;
let hip3_recent = client.hyperliquid.hip3.trades.recent("km:US500", Some(50)).await?;
```

**Note:** The `recent()` method is available for Lighter.xyz and HIP-3 only. Hyperliquid does not have a recent trades endpoint — use `list()` with a time range instead.

### Instruments

```rust
// List all Hyperliquid instruments
let instruments = client.hyperliquid.instruments.list().await?;
for inst in &instruments {
    println!("{}: {}x leverage", inst.name, inst.max_leverage.unwrap_or(0));
}

// Get specific instrument
let btc = client.hyperliquid.instruments.get("BTC").await?;

// Lighter.xyz instruments (different schema with fees, market IDs)
let lighter_instruments = client.lighter.instruments.list().await?;
for inst in &lighter_instruments {
    println!("{}: taker_fee={:?}, maker_fee={:?}", inst.symbol, inst.taker_fee, inst.maker_fee);
}

// HIP-3 instruments (derived from live data, includes mark price + OI)
let hip3_instruments = client.hyperliquid.hip3.instruments.list().await?;
for inst in &hip3_instruments {
    println!("{} ({}:{}): mark={:?}", inst.coin, inst.namespace, inst.ticker, inst.mark_price);
}
```

### Funding Rates

```rust
use oxarchive::resources::funding::FundingHistoryParams;
use oxarchive::types::OiFundingInterval;

// Get current funding rate
let current = client.hyperliquid.funding.current("BTC").await?;
println!("Funding rate: {}", current.funding_rate);

// Get history with aggregation interval
let history = client.hyperliquid.funding.history("ETH", FundingHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
    interval: Some(OiFundingInterval::OneHour),
}).await?;
```

#### Aggregation Intervals

| Interval | Description |
|----------|-------------|
| `5m` | 5 minutes |
| `15m` | 15 minutes |
| `30m` | 30 minutes |
| `1h` | 1 hour |
| `4h` | 4 hours |
| `1d` | 1 day |

When omitted, raw ~1 minute data is returned.

### Open Interest

```rust
use oxarchive::resources::open_interest::OpenInterestHistoryParams;
use oxarchive::types::OiFundingInterval;

// Get current open interest
let current = client.hyperliquid.open_interest.current("BTC").await?;
println!("Open interest: {}", current.open_interest);

// Get history with aggregation
let history = client.hyperliquid.open_interest.history("BTC", OpenInterestHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
    interval: Some(OiFundingInterval::OneHour),
}).await?;
```

### Liquidations (Hyperliquid and HIP-3)

Historical liquidation events from May 2025 onwards. Available on `client.hyperliquid.liquidations` and `client.hyperliquid.hip3.liquidations`.

```rust
use oxarchive::resources::liquidations::*;

// Get liquidation history
let liquidations = client.hyperliquid.liquidations.history("BTC", LiquidationHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
}).await?;

// Get liquidations for a specific user
let user_liq = client.hyperliquid.liquidations.by_user("0x1234...", LiquidationsByUserParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    coin: Some("BTC".to_string()),
    cursor: None,
    limit: None,
}).await?;

// Get pre-aggregated liquidation volume (100-1000x less data)
let volume = client.hyperliquid.liquidations.volume("BTC", LiquidationVolumeParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
for bucket in &volume.data {
    println!("total=${}, long=${}, short=${}", bucket.total_usd, bucket.long_usd, bucket.short_usd);
}

// HIP-3 liquidations (same API, different exchange prefix)
let hip3_liqs = client.hyperliquid.hip3.liquidations.history("km:US500", LiquidationHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
}).await?;

let hip3_vol = client.hyperliquid.hip3.liquidations.volume("km:US500", LiquidationVolumeParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
```

### Candles (OHLCV)

```rust
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::CandleInterval;

let candles = client.hyperliquid.candles.history("BTC", CandleHistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    interval: Some(CandleInterval::OneHour),
    cursor: None,
    limit: None,
}).await?;

for candle in &candles.data {
    println!("O={} H={} L={} C={} V={}", candle.open, candle.high, candle.low, candle.close, candle.volume);
}
```

#### Available Intervals

`1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`, `1w`

### Orders (Hyperliquid and HIP-3)

Order history, order flow aggregation, and TP/SL order queries. Available on `client.hyperliquid.orders` and `client.hyperliquid.hip3.orders`.

```rust
use oxarchive::resources::orders::*;

// Get order history for a symbol
let orders = client.hyperliquid.orders.history("BTC", OrderHistoryParams {
    start: Some(1704067200000_i64.into()),
    end: Some(1704153600000_i64.into()),
    user: None,
    status: None,
    order_type: None,
    cursor: None,
    limit: Some(1000),
}).await?;

// Filter by user address
let user_orders = client.hyperliquid.orders.history("BTC", OrderHistoryParams {
    start: Some(1704067200000_i64.into()),
    end: Some(1704153600000_i64.into()),
    user: Some("0x1234...".to_string()),
    status: Some("filled".to_string()),
    order_type: None,
    cursor: None,
    limit: None,
}).await?;

// Get aggregated order flow
let flow = client.hyperliquid.orders.flow("BTC", OrderFlowParams {
    start: Some(1704067200000_i64.into()),
    end: Some(1704153600000_i64.into()),
    interval: Some("1h".to_string()),
    limit: None,
}).await?;

// Get TP/SL (take-profit / stop-loss) orders
let tpsl = client.hyperliquid.orders.tpsl("BTC", TpslParams {
    start: Some(1704067200000_i64.into()),
    end: Some(1704153600000_i64.into()),
    user: None,
    triggered: Some(true),
    cursor: None,
    limit: None,
}).await?;

// HIP-3 orders
let hip3_orders = client.hyperliquid.hip3.orders.history("km:US500", OrderHistoryParams::default()).await?;
```

### L4 Orderbook (Hyperliquid and HIP-3)

Node-level L4 orderbook data with user attribution. Available on `client.hyperliquid.l4_orderbook` and `client.hyperliquid.hip3.l4_orderbook`.

```rust
use oxarchive::resources::l4_orderbook::*;

// Get current L4 orderbook snapshot
let l4 = client.hyperliquid.l4_orderbook.get("BTC", None).await?;

// Get with specific timestamp and depth
let l4 = client.hyperliquid.l4_orderbook.get("BTC", Some(L4OrderBookParams {
    timestamp: Some(1704067200000_i64.into()),
    depth: Some(20),
})).await?;

// Get paginated L4 orderbook diffs
let diffs = client.hyperliquid.l4_orderbook.diffs("BTC", L4DiffsParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;

// Get paginated L4 orderbook history
let history = client.hyperliquid.l4_orderbook.history("BTC", L4HistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
    depth: Some(20),
}).await?;

// HIP-3 L4 orderbook
let hip3_l4 = client.hyperliquid.hip3.l4_orderbook.get("km:US500", None).await?;
let hip3_diffs = client.hyperliquid.hip3.l4_orderbook.diffs("km:US500", L4DiffsParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: None,
}).await?;
```

### L2 Orderbook (Hyperliquid and HIP-3)

Full-depth L2 orderbook derived from L4 data. Available on `client.hyperliquid.l2_orderbook` and `client.hyperliquid.hip3.l2_orderbook`.

```rust
use oxarchive::resources::l2_orderbook::*;

// Get current L2 full-depth orderbook
let l2 = client.hyperliquid.l2_orderbook.get("BTC", None).await?;

// Get L2 orderbook at a specific timestamp
let l2 = client.hyperliquid.l2_orderbook.get("BTC", Some(L2OrderBookParams {
    timestamp: Some(1704067200000_i64.into()),
    depth: Some(50),
})).await?;

// Get L2 orderbook history
let l2_history = client.hyperliquid.l2_orderbook.history("BTC", L2HistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
    depth: Some(50),
}).await?;

// Get L2 tick-level diffs
let l2_diffs = client.hyperliquid.l2_orderbook.diffs("BTC", L2DiffsParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;

// HIP-3 L2 orderbook
let hip3_l2 = client.hyperliquid.hip3.l2_orderbook.get("km:US500", None).await?;
```

### L3 Orderbook (Lighter only)

Order-level L3 orderbook data showing individual orders. Available on `client.lighter.l3_orderbook`.

```rust
use oxarchive::resources::l3_orderbook::L3HistoryParams;

// Get current L3 orderbook
let l3 = client.lighter.l3_orderbook.get("BTC", None).await?;

// Get with depth limit
let l3 = client.lighter.l3_orderbook.get("BTC", Some(20)).await?;

// Get paginated L3 orderbook history
let history = client.lighter.l3_orderbook.history("BTC", L3HistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
```

### HIP-4 Outcome Markets (Hyperliquid)

Binary outcome perps deployed under the Hyperliquid namespace. Coin symbols
use the bare `#<10*outcome_id + side>` form (`#0`, `#1`, `#55850`, ...). The
SDK passes `symbol` straight through to the URL path. Use the bare form in
your code; do not pre-encode `#` to `%23`.

HIP-4 markets are fully collateralized: there are **no funding rates, no
liquidations, and no candles**. `mark_price` on HIP-4 is an implied
probability in `[0, 1]`, not a USD price.

```rust
use oxarchive::exchanges::{Hip4HistoryRange, Hip4ListOutcomesParams,
    Hip4OrderBookParams, Hip4TradesParams};

// Outcomes (per-outcome view, both sides combined)
let outcomes = client.hyperliquid.hip4.list_outcomes(None).await?;
for o in &outcomes.data {
    println!("{} . {:?}", o.outcome_id, o.display_title);
}

// Filter by settlement state OR by slug
let live = client.hyperliquid.hip4.list_outcomes(Some(Hip4ListOutcomesParams {
    is_settled: Some(false),
    slug: None,
    cursor: None,
    limit: Some(50),
})).await?;

let one = client.hyperliquid.hip4
    .get_outcome_by_slug("btc-above-78213-may-04-0600").await?;
println!("aggregated_oi: {:?}", one.aggregated_oi);

// Outcome detail (with aggregated_oi)
let detail = client.hyperliquid.hip4.get_outcome(5585).await?;

// Per-side instruments (`#0`, `#1`, ...)
let insts = client.hyperliquid.hip4.get_instruments().await?;
let inst = client.hyperliquid.hip4.get_instrument("#0").await?;

// L2 orderbook
let ob = client.hyperliquid.hip4.get_orderbook("#0", None).await?;
let ob_at = client.hyperliquid.hip4.get_orderbook("#0", Some(Hip4OrderBookParams {
    timestamp: Some(1714694400000_i64.into()),
    depth: Some(20),
})).await?;
let ob_history = client.hyperliquid.hip4.get_orderbook_history("#0", Hip4HistoryRange {
    start: 1714694400000_i64.into(),
    end:   1714780800000_i64.into(),
    cursor: None,
    limit: Some(100),
}).await?;

// Trades (history + recent)
let trades = client.hyperliquid.hip4.get_trades("#0", Hip4TradesParams {
    start: 1714694400000_i64.into(),
    end:   1714780800000_i64.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;
let recent = client.hyperliquid.hip4.get_trades_recent("#0", Some(50)).await?;

// Open interest (per-side history + latest)
let oi_hist = client.hyperliquid.hip4.get_open_interest("#0", Hip4HistoryRange {
    start: 1714694400000_i64.into(),
    end:   1714780800000_i64.into(),
    cursor: None,
    limit: None,
}).await?;
let oi_now = client.hyperliquid.hip4.get_open_interest_current("#0").await?;
// mark_price on HIP-4 is an implied probability in [0, 1].

// Summary, freshness, prices
let summary    = client.hyperliquid.hip4.get_summary("#0").await?;
let freshness  = client.hyperliquid.hip4.get_freshness("#0").await?;
let prices     = client.hyperliquid.hip4.get_prices("#0",
    1714694400000_i64, 1714780800000_i64, Some("1h"), Some(100), None).await?;

// L4 (current snapshot, diffs, checkpoint history)
let l4_now     = client.hyperliquid.hip4.get_l4_orderbook("#0", None).await?;
let l4_diffs   = client.hyperliquid.hip4.get_l4_diffs("#0", Hip4HistoryRange {
    start: 1714694400000_i64.into(),
    end:   1714780800000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
```

### Hyperliquid Spot

Spot trading pairs deployed under the Hyperliquid namespace. Symbols use the
dashed canonical form (`HYPE-USDC`, `PURR-USDC`, ...). The server resolves
the dashed form to the wire format (`PURR/USDC`, `@107`) internally.

Spot has **no funding, no open interest, no liquidations, and no candles**:
those are perp-only constructs. Trade history backfills to 2025-03-22; every
other dataset is live-only from 2026-05-05.

```rust
use oxarchive::resources::orderbook::{GetOrderBookParams, OrderBookHistoryParams};
use oxarchive::resources::l4_orderbook::{L4DiffsParams, L4HistoryParams, L4OrderBookParams};
use oxarchive::resources::orders::OrderHistoryParams;
use oxarchive::resources::spot::SpotTwapParams;
use oxarchive::resources::trades::GetTradesParams;

// Pair discovery (dashed canonical: HYPE-USDC, PURR-USDC, ...).
let pairs = client.hyperliquid.spot.pairs.list().await?;
let hype = client.hyperliquid.spot.pairs.get("HYPE-USDC").await?;

// Current L2 orderbook.
let ob = client.hyperliquid.spot.orderbook.get("HYPE-USDC", None).await?;
println!("HYPE-USDC mid: {:?}", ob.mid_price);

// Historical L2 orderbook.
let ob_history = client.hyperliquid.spot.orderbook.history("HYPE-USDC", OrderBookHistoryParams {
    start: 1746489600000_i64.into(), // 2026-05-06 UTC
    end:   1746576000000_i64.into(),
    cursor: None,
    limit: Some(1000),
    depth: None,
    granularity: None,
}).await?;

// Trades (history backfilled to 2025-03-22).
let trades = client.hyperliquid.spot.trades.list("PURR-USDC", GetTradesParams {
    start: 1742601600000_i64.into(), // 2025-03-22 UTC
    end:   1746576000000_i64.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;

// L4 reconstruction.
let l4_now = client.hyperliquid.spot.l4_orderbook.get("HYPE-USDC", None).await?;
let l4_diffs = client.hyperliquid.spot.l4_orderbook.diffs("HYPE-USDC", L4DiffsParams {
    start: 1746489600000_i64.into(),
    end:   1746576000000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
let l4_history = client.hyperliquid.spot.l4_orderbook.history("HYPE-USDC", L4HistoryParams {
    start: 1746489600000_i64.into(),
    end:   1746576000000_i64.into(),
    cursor: None,
    limit: Some(10),
    depth: Some(20),
}).await?;

// Order lifecycle events.
let orders = client.hyperliquid.spot.orders.history("HYPE-USDC", OrderHistoryParams::default()).await?;

// TWAP statuses by symbol or by user.
let twap_sym = client.hyperliquid.spot.twap
    .by_symbol("HYPE-USDC", SpotTwapParams::default()).await?;
let twap_user = client.hyperliquid.spot.twap
    .by_user("0x1234...", SpotTwapParams::default()).await?;

// Per-table freshness.
let freshness = client.hyperliquid.spot.freshness("HYPE-USDC").await?;
```

### Freshness

Check when each data type was last updated for a specific coin.

```rust
let freshness = client.hyperliquid.freshness("BTC").await?;
let lighter_freshness = client.lighter.freshness("BTC").await?;
let hip3_freshness = client.hyperliquid.hip3.freshness("km:US500").await?;
let spot_freshness = client.hyperliquid.spot.freshness("HYPE-USDC").await?;
```

### Summary

Combined market snapshot — mark/oracle price, funding rate, open interest, 24h volume, and liquidation volumes.

```rust
let summary = client.hyperliquid.summary("BTC").await?;
let lighter_summary = client.lighter.summary("BTC").await?;
let hip3_summary = client.hyperliquid.hip3.summary("km:US500").await?;
```

### Price History

Mark, oracle, and mid price history. Supports aggregation intervals.

```rust
let prices = client.hyperliquid.price_history(
    "BTC",
    1704067200000_i64,  // start
    1704153600000_i64,  // end
    Some("1h"),         // interval
    Some(100),          // limit
    None,               // cursor
).await?;
```

## Data Quality Monitoring

Monitor data coverage, incidents, latency, and SLA compliance.

```rust
// System health status
let status = client.data_quality.status().await?;
println!("System: {}", status.status);

// Data coverage
let coverage = client.data_quality.coverage().await?;

// Symbol-specific coverage with gap detection
let btc = client.data_quality.symbol_coverage("hyperliquid", "BTC").await?;

// Incidents
let incidents = client.data_quality.list_incidents(None).await?;
let incident = client.data_quality.get_incident("inc-123").await?;

// Latency and SLA
let latency = client.data_quality.latency().await?;
let sla = client.data_quality.sla(None).await?;
```

## Web3 Authentication

Wallet-based authentication using SIWE (Sign-In with Ethereum) and x402 USDC payments.

```rust
// Step 1: Get a SIWE challenge
let challenge = client.web3.challenge("0xYourWalletAddress").await?;

// Step 2: Sign the challenge and register
let result = client.web3.signup(
    &challenge.message,
    "0xYourSignature",
).await?;
println!("API Key: {}", result.api_key);

// Manage API keys (requires fresh SIWE signature)
let keys = client.web3.list_keys(&challenge.message, "0xSignature").await?;
client.web3.revoke_key(&challenge.message, "0xSignature", "key-id").await?;

// Subscribe with x402 USDC payment
let sub = client.web3.subscribe("build", "base64_payment_payload").await?;
```

## Webhooks

0xArchive pushes events to an HTTPS endpoint you own: fills and transfers on wallets you watch, liquidations and liquidation bursts, funding flips, oracle jumps, listings, ingest and chain health, export jobs finishing. `client.webhooks` covers the whole management surface, and `oxarchive::webhook_signature` verifies the deliveries that arrive.

### What each plan gets

| Plan | Endpoints | Subscriptions | Watched wallets | Deliveries a day |
| --- | --- | --- | --- | --- |
| Free | 0 | 0 | 0 | 0 |
| Build | 1 | 8 | 2 | 5,000 |
| Pro | 4 | 40 | 15 | 50,000 |
| Scale | 12 | 200 | 50 | 500,000 |
| Enterprise | negotiated | negotiated | negotiated | negotiated |

Free has no webhook delivery at all. Not one endpoint, not one subscription, not one watched wallet, not one delivery.

Free does keep both preview routes. `estimate` and `dry_run` answer on every plan, so you can design a rule, see how often it would have fired, and read the occurrences it would have sent before paying for anything. That is the intended way in: size the rule first, then buy the plan that fits it.

When an account exceeds its deliveries a day, the subscription responsible is paused and reports that it is paused, rather than events being quietly dropped. Nothing is buffered while it is paused, so recover the gap by querying the REST archive over that window. The field names for the pause state are still settling, so this SDK keeps them in `WebhookSubscription::extra` rather than binding them to typed fields that would have to change.

### Size a rule before you build it

```rust
use oxarchive::EstimateParams;
use serde_json::json;

let estimate = client.webhooks.estimate(
    EstimateParams::new("market.liquidation")
        .filters(json!({
            "venue": "hyperliquid",
            "conditions": [
                {"metric": "notional_usd", "op": "greater_than_or_equal", "value": 250_000}
            ]
        }))
        .lookback_days(30),
).await?;

println!("{} events over {} days", estimate.total, estimate.days);
println!("median {:.1}/day, busiest {}/day", estimate.per_day_p50, estimate.per_day_max);

// The ladder is the same rule at other thresholds, so you can pick one that
// fits your plan's deliveries a day instead of discovering the cap in prod.
for rung in &estimate.ladder {
    println!("  at {:>12.0}: {:.1}/day", rung.value, rung.per_day);
}
```

`dry_run` answers the narrower question: which occurrences in the last few hours would this rule actually have sent, payload and all.

```rust
use oxarchive::DryRunParams;

let preview = client.webhooks.dry_run(
    DryRunParams::new("market.liquidation")
        .filters(json!({"venue": "hyperliquid"}))
        .lookback_s(3_600)
        .limit(20),
).await?;

println!("{} matched, showing {}", preview.matched, preview.occurrences.len());
for occurrence in &preview.occurrences {
    println!("  {} {}", occurrence.observed_at_estimate, occurrence.data);
}
```

Both previews share a per-minute budget, and `dry_run` supports fewer event types than `estimate` does. A refusal names the types it does support.

### The event-type catalog

`event_types()` is the source of truth for what can be subscribed to and how. It declares, per type, the venues it covers, the filters it accepts, the parameters it takes with their defaults and bounds, and the metrics conditions can be written against. Subscribe-time validation reads the same declarations, so anything the catalog does not declare is refused rather than quietly ignored.

```rust
for event in client.webhooks.event_types().await? {
    if !event.live {
        continue; // published as coming soon; subscriptions are refused
    }
    println!("{} [{}] {}", event.event_type, event.scope, event.description);
    for (name, metric) in &event.metrics {
        println!("    metric {name}: {} {}", metric.value_type, metric.unit.clone().unwrap_or_default());
    }
}
```

`scope` tells you what a type reports on: `public` for market-wide events, `addresses` for events about wallets you watch, `user` for events about your own account such as an export finishing.

### Endpoints

```rust
use oxarchive::CreateEndpointParams;

let endpoint = client.webhooks.create_endpoint(
    CreateEndpointParams::new("https://example.com/hooks/0xarchive")
        .description("prod receiver"),
).await?;

// Shown exactly once, here and on rotate. No list or get call returns it again.
let secret = endpoint.secret.clone().expect("create returns the secret once");
store_somewhere_safe(&secret);
```

Private and loopback destinations are refused at create time and re-checked on every delivery attempt.

`test_endpoint(id)` queues a real `webhook.test` delivery through the identical dispatch path, signed the same way as any other event, so it is a genuine end-to-end check of your receiver rather than a simulation.

```rust
let fired = client.webhooks.test_endpoint(&endpoint.id).await?;
println!("queued delivery {} for event {}", fired.delivery_id, fired.event_id);
```

### Subscriptions

A subscription is one event type plus the rule that decides which of its occurrences reach an endpoint.

```rust
use oxarchive::{CreateSubscriptionParams, UpdateSubscriptionParams};

let sub = client.webhooks.create_subscription(
    CreateSubscriptionParams::new(&endpoint.id, "market.liquidation")
        .filters(json!({
            "venue": "hyperliquid",
            "symbols": ["BTC", "ETH"],
            "conditions": [
                {"metric": "notional_usd", "op": ">=", "value": 250_000}
            ]
        })),
).await?;

// Read the stored config back: operator spellings are canonicalised (">=" is
// stored as "greater_than_or_equal"), addresses are lowercased, and declared
// parameter defaults are filled in.
println!("{}", sub.filters);

// Edit in place; replacing a rule never needs delete and recreate.
client.webhooks.update_subscription(&sub.id, UpdateSubscriptionParams::default().enabled(false)).await?;
```

### Watched wallets

Address-scoped event types only report on wallets on your watched list, and a subscription cannot name an address that is not on it.

```rust
let wallet = "0x1111111111111111111111111111111111111111";
let watched = client.webhooks.add_address(wallet, Some("treasury")).await?;

let all = client.webhooks.list_addresses().await?;
println!("{} of {:?} watched", all.addresses.len(), all.limit);

client.webhooks.delete_address(&watched.id).await?;
```

Adding a wallet you already watch is idempotent: it updates the label and does not spend a second slot.

### Verifying a delivery

Every delivery carries these headers. Look them up case insensitively.

| Header | Value |
| --- | --- |
| `0xa-signature` | `t=<unix seconds>,v1=<hex>[,v1=<hex>]` |
| `0xa-event-id` | the event UUID, stable across retries and redelivery |
| `0xa-event-type` | the event type, for example `webhook.test` |
| `content-type` | `application/json` |

There is no separate timestamp header. The timestamp lives inside `0xa-signature` as `t`, and since it is the first component of the signed string it cannot be moved without breaking the signature.

The signed bytes are the timestamp, one ASCII full stop, and the raw body:

```text
signed_payload = <t> || "." || <raw request body>
signature      = lowercase_hex(HMAC_SHA256(key = the whole whsec_... string, signed_payload))
```

```rust
use oxarchive::webhook_signature::{SignatureError, WebhookVerifier};

let verifier = WebhookVerifier::new(secret);

match verifier.verify(raw_body_bytes, signature_header) {
    Ok(_) => { /* deduplicate on 0xa-event-id, answer 2xx, process out of band */ }
    Err(SignatureError::Stale { .. }) => { /* outside the replay window; refuse */ }
    Err(_) => { /* refuse with a 4xx */ }
}
```

Four things decide whether your verifier works:

**Hash the bytes you received.** Do not re-serialise the JSON first. The body is rendered by PostgreSQL's `jsonb` output, so its key order and spacing match neither the emitter nor any JSON library's default, and `serde_json::to_string(&value)` will produce different bytes and fail. Capture the raw body before a parser touches it. In axum that means `bytes::Bytes` or `String` as the extractor rather than `Json<T>`; anything behind a proxy that pretty-prints or re-encodes JSON will break verification.

**The key is the whole secret string.** `whsec_` included. Do not strip the prefix, do not hex-decode the 64 characters after it, do not base64-decode anything.

**Read every `v1`, not the first.** During a rotation overlap the header carries two, and nothing says which secret each belongs to. `header.split("v1=")` style parsing passes in steady state and fails intermittently the moment somebody rotates.

**Compare in constant time.** `WebhookVerifier` does, through the HMAC crate's own comparison. A plain `==` on hex strings leaks the position of the first differing byte.

The verifier also enforces a replay window, 300 seconds by default and configurable with `tolerance_secs`. `t` is generated per attempt rather than per event, so even an hour-late retry arrives with a fresh timestamp and nothing legitimate is ever stale. Keep the window tight: the destination URL is not part of the signed string, so an observed delivery could otherwise be replayed at a different path on the same host forever.

### Rotating the signing secret

```rust
let rotated = client.webhooks.rotate_secret(&endpoint.id).await?;
// The previous secret keeps verifying for 24 hours.
let verifier = WebhookVerifier::with_secrets([rotated.secret.clone(), old_secret]);
```

During the overlap every delivery carries two `v1` values over the same payload, one per secret, so a receiver that has not rolled yet keeps verifying. Store the new secret alongside the old one, deploy, then drop the old one before the window closes.

Only one previous secret is ever carried. Rotating twice inside the same window overwrites it, and the original stops verifying immediately, so do not rotate twice in a day unless you mean to.

### Deliveries, retries, and redelivery

```rust
for attempt in client.webhooks.deliveries(&endpoint.id, Some(50)).await? {
    println!("{} {} attempts={} status={:?}",
        attempt.created_at, attempt.state, attempt.attempts, attempt.last_status_code);
}

// Same event id on purpose, so a receiver that already handled it deduplicates.
client.webhooks.redeliver(&delivery_id).await?;
```

Your receiver has 10 seconds to answer. Acknowledge with a 2xx immediately and do the work out of band. Anything outside 200 to 299 counts as a failure, redirects included, and enters the retry ladder: 5s, 30s, 2m, 10m, 1h, then hourly, giving up after 24 hours. Ten consecutive failures spanning at least six hours auto disable the endpoint, which `enable_endpoint(id)` clears.

If a delivery fails to verify, answer 4xx and log it. A 5xx just replays the same bad delivery at you for a day.

Delivery is at least once. Deduplicate on `0xa-event-id`, never on the signature or on `t`, both of which change on every attempt.

### Route reference

| SDK method | Route |
| --- | --- |
| `event_types()` | `GET /v1/webhooks/event-types` |
| `list_endpoints()` | `GET /v1/webhooks/endpoints` |
| `create_endpoint(params)` | `POST /v1/webhooks/endpoints` |
| `delete_endpoint(id)` | `DELETE /v1/webhooks/endpoints/{id}` |
| `rotate_secret(id)` | `POST /v1/webhooks/endpoints/{id}/rotate` |
| `enable_endpoint(id)` | `POST /v1/webhooks/endpoints/{id}/enable` |
| `test_endpoint(id)` | `POST /v1/webhooks/endpoints/{id}/test` |
| `deliveries(id, limit)` | `GET /v1/webhooks/endpoints/{id}/deliveries` |
| `redeliver(delivery_id)` | `POST /v1/webhooks/deliveries/{id}/redeliver` |
| `list_subscriptions()` | `GET /v1/webhooks/subscriptions` |
| `create_subscription(params)` | `POST /v1/webhooks/subscriptions` |
| `update_subscription(id, params)` | `PATCH /v1/webhooks/subscriptions/{id}` |
| `delete_subscription(id)` | `DELETE /v1/webhooks/subscriptions/{id}` |
| `dry_run(params)` | `POST /v1/webhooks/subscriptions/dry-run` |
| `estimate(params)` | `POST /v1/webhooks/subscriptions/estimate` |
| `list_addresses()` | `GET /v1/webhooks/addresses` |
| `add_address(address, label)` | `POST /v1/webhooks/addresses` |
| `delete_address(id)` | `DELETE /v1/webhooks/addresses/{id}` |

## WebSocket Client

Requires the `websocket` feature. Supports two modes on a single connection:
- **Real-time**: subscribe to live market data
- **Replay**: replay historical data with timing preserved

For file-based historical exports, use the [Data Catalog](https://www.0xarchive.io/data).

### Real-time Streaming

```rust
use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions};

let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key"));
ws.connect().await?;

ws.subscribe("orderbook", Some("BTC")).await?;
ws.subscribe("trades", Some("ETH")).await?;

let mut rx = ws.rx.take().expect("receiver");
while let Some(msg) = rx.recv().await {
    match msg {
        ServerMsg::Data { channel, coin, data, .. } => {
            println!("{channel} {} update: {}", coin.unwrap_or_default(), data);
        }
        ServerMsg::Error { message } => eprintln!("Error: {message}"),
        _ => {}
    }
}
```

### Historical Replay

```rust
// Replay BTC orderbook at 100x speed
ws.replay("orderbook", "BTC", 1704067200000, Some(1704070800000), Some(100.0)).await?;

// Control playback
ws.replay_pause().await?;
ws.replay_resume().await?;
ws.replay_seek(1704069000000).await?;
ws.replay_stop().await?;
```

### Available Channels

| Channel | Description | Realtime | Replay |
|---------|-------------|----------|--------|
| `orderbook` | L2 order book (~1.2s resolution) | Yes | Yes |
| `trades` | Trade/fill updates | Yes | Yes |
| `candles` | OHLCV candle data | Yes | Yes |
| `liquidations` | Liquidation events. Each item is a fill row with `is_liquidation: true`. | Yes | Yes |
| `open_interest` | Open interest snapshots | Yes | Yes |
| `funding` | Funding rate snapshots | Yes | Yes |
| `ticker` | Price and 24h volume | Yes | No |
| `all_tickers` | All market tickers | Yes | No |
| `lighter_orderbook` | Lighter.xyz L2 order book | Yes | Yes |
| `lighter_trades` | Lighter.xyz trades | Yes | Yes |
| `lighter_candles` | Lighter.xyz candles | Yes | Yes |
| `lighter_open_interest` | Lighter.xyz open interest | Yes | Yes |
| `lighter_funding` | Lighter.xyz funding rates | Yes | Yes |
| `lighter_l3_orderbook` | Lighter.xyz L3 order-level orderbook | Yes | Yes |
| `hip3_orderbook` | HIP-3 L2 order book | Yes | Yes |
| `hip3_trades` | HIP-3 trades | Yes | Yes |
| `hip3_candles` | HIP-3 candles | Yes | Yes |
| `hip3_open_interest` | HIP-3 open interest | Yes | Yes |
| `hip3_funding` | HIP-3 funding rates | Yes | Yes |
| `hip3_liquidations` | HIP-3 liquidation events. Same wire shape as `liquidations`. | Yes | Yes |
| `hip4_orderbook` | HIP-4 outcome-market L2 order book | Yes | Yes |
| `hip4_trades` | HIP-4 trade/fill updates | Yes | Yes |
| `hip4_open_interest` | HIP-4 open interest snapshots | Yes | Yes |
| `l4_diffs` | Hyperliquid L4 orderbook diffs with user attribution | Yes | No |
| `l4_orders` | Hyperliquid order lifecycle events | Yes | No |
| `hip3_l4_diffs` | HIP-3 L4 orderbook diffs with user attribution | Yes | No |
| `hip3_l4_orders` | HIP-3 order lifecycle events | Yes | No |
| `hip4_l4_diffs` | HIP-4 L4 orderbook diffs with user attribution | Yes | No |
| `hip4_l4_orders` | HIP-4 order lifecycle events | Yes | No |
| `spot_orderbook` | Hyperliquid Spot L2 order book | Yes | No |
| `spot_trades` | Hyperliquid Spot trades | Yes | No |
| `spot_l4_diffs` | Hyperliquid Spot L4 orderbook diffs with user attribution | Yes | No |
| `spot_l4_orders` | Hyperliquid Spot order lifecycle events | Yes | No |
| `spot_twap` | Hyperliquid Spot TWAP execution updates | Yes | No |

HIP-4 outcome markets have **no funding, no liquidations, and no candles** by design (binary outcomes settle to 0/1 at expiry).

Hyperliquid Spot has **no funding, no open interest, no liquidations, and no candles** by design (perp-only constructs). Spot symbols use the dashed canonical form (`HYPE-USDC`, `PURR-USDC`).

### Settlement Frame

When a HIP-4 outcome settles, the server emits an `outcome_settled` frame
exactly once per `(outcome_id, side)` and proactively unsubscribes the
client from every `hip4_*` subscription on that coin. Other subscriptions
remain active.

```rust
use oxarchive::ws::ServerMsg;

while let Some(msg) = rx.recv().await {
    if let ServerMsg::OutcomeSettled { coin, settlement_value, .. } = msg {
        println!("{coin} settled to {:?}", settlement_value);
    }
}
```

### Tier Limits

Every tier has access to all markets, all schemas, and full history. Tiers differ only in capacity limits.

| Tier | Max Subscriptions | Max Connections | Max Replay Speed | Max Batch Size |
|------|------------------|-----------------|------------------|----------------|
| Free | 10 | 2 | 10x | 2,000 |
| Build | 500 | 3 | 50x | 2,000 |
| Pro | 3,000 | 5 | 100x | 5,000 |
| Scale | 20,000 | 16 | 300x | 10,000 |
| Enterprise | Custom | Custom | from 500x | 10,000 |

## Timestamp Formats

All time parameters accept the `Timestamp` enum:

```rust
use oxarchive::types::Timestamp;

// Unix milliseconds (i64)
let ts: Timestamp = 1704067200000_i64.into();

// ISO 8601 string
let ts: Timestamp = "2024-01-01T00:00:00Z".into();

// chrono::DateTime<Utc>
let ts: Timestamp = chrono::Utc::now().into();
```

## Error Handling

```rust
use oxarchive::Error;

match client.hyperliquid.orderbook.get("BTC", None).await {
    Ok(ob) => println!("Mid price: {:?}", ob.mid_price),
    Err(Error::Api { message, code, .. }) => {
        eprintln!("API error ({code}): {message}");
    }
    Err(Error::Timeout) => eprintln!("Request timed out"),
    Err(Error::Http(e)) => eprintln!("HTTP error: {e}"),
    Err(e) => eprintln!("Other error: {e}"),
}
```

## Examples

Run the included examples:

```bash
export OXARCHIVE_API_KEY="your-api-key"

# Basic usage
cargo run --example basic

# Cursor-based pagination
cargo run --example pagination

# Hyperliquid Spot
cargo run --example spot

# Webhooks: size a rule, create an endpoint, verify deliveries
cargo run --example webhooks

# WebSocket (requires websocket feature)
cargo run --example websocket --features websocket
```

## Data Catalog

For large-scale data exports (full order books, complete trade history, etc.), use the [Data Catalog](https://www.0xarchive.io/data). It lets you choose markets, datasets, and date ranges, see a live quote, and export zstd-compressed Parquet.

## Links

- [API Docs](https://www.0xarchive.io/docs)
- [Python SDK](https://pypi.org/project/oxarchive/)
- [TypeScript SDK](https://npmjs.com/package/@0xarchive/sdk)
- [CLI](https://npmjs.com/package/@0xarchive/cli)
- [MCP Server](https://mcp.0xarchive.io) (or [self-host](https://npmjs.com/package/@0xarchive/mcp-server))
- [0xArchive Skill](https://github.com/0xArchiveIO/0xarchive-skill)
- [Examples](https://github.com/0xArchiveIO/examples)

## Requirements

- Rust 1.75+
- tokio runtime

## License

MIT
