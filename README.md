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
oxarchive = "1.11"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

For WebSocket support (real-time streaming and replay):

```toml
oxarchive = { version = "1.11", features = ["websocket"] }
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
| Hyperliquid | April 2023+ | Core perpetuals; coverage varies by schema and route. |
| Hyperliquid HIP-3 | February 2026+ for served history | Builder perps; funding and OI update at roughly 10 seconds. |
| Hyperliquid HIP-4 | May 2, 2026+ | Candles and outcome-side OI are served from 2026-05-02; OI updates at ~10s. No funding or liquidations. |
| Hyperliquid Spot | Trades March 2025+; candles from exactly 2025-03-22T10:50:22Z; orderbook, L4, TWAP, and freshness from May 2026 | 326 authenticated inventory rows using dashed symbols (`HYPE-USDC`, `PURR-USDC`, ...). No funding, OI, or liquidations. |
| Lighter.xyz | Candles from 2025-08-01; observed global per-fill trade floor January 17, 2025; exact starts vary by market. L3 from March 5, 2026+ | Maker/taker trade context; L3 caps at 250 orders per side; funding/OI update at ~10s. |

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
| `candles` | Yes | Yes | Yes | Yes |
| `breadth` (above session VWAP) | -- | Yes | -- | -- |
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

Depth is route-specific. Hyperliquid-family native L2 is capped at 20 levels per side. Lighter native L2 includes all served levels, and Lighter L3 is capped at 250 orders per side.

**Note:** Dedicated L2 routes derived from L4 return all served levels where supported. Lighter L3 exposes individual resting orders rather than price levels and begins March 5, 2026.

#### Lighter Orderbook Granularity

Lighter.xyz orderbook history supports a `granularity` parameter for different data resolutions:

```rust
use oxarchive::types::LighterGranularity;

let history = client.lighter.orderbook.history("BTC", OrderBookHistoryParams {
    start: 1769904000000_i64.into(), // 2026-02-01 00:00 UTC
    end: 1769990400000_i64.into(),   // 2026-02-02 00:00 UTC
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
    1769904000000_i64,  // start (2026-02-01 00:00 UTC)
    1769907600000_i64,  // end (2026-02-01 01:00 UTC)
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
    1769904000000_i64, // 2026-02-01 00:00 UTC
    1769990400000_i64, // 2026-02-02 00:00 UTC
    Some(20),
).await?;
println!("{} tick-level snapshots", all_snapshots.len());

// Option 3: Manual control — fetch raw tick data and reconstruct yourself
let tick_data = client.lighter.orderbook.history_tick(
    "BTC", 1769904000000_i64, 1769907600000_i64, None,
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
let mut cursor = 1769904000000_i64; // 2026-02-01 00:00 UTC
let end = 1769990400000_i64; // 2026-02-02 00:00 UTC;

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

### HIP-3 Breadth Above Session VWAP

HIP-3 breadth is the percentage of eligible instruments trading above their
current UTC-session VWAP. The session resets at 00:00 UTC, uses the close of
the most recently completed one-minute candle, and excludes instruments with
no session volume or a price older than five minutes. History begins on
**2026-08-28**. `value_pct` is unavailable (`None`) when no instrument is
eligible; do not render it as 0%, and do not average percentages across
snapshots because the eligible denominator varies.

```rust
use oxarchive::resources::breadth::BreadthHistoryParams;
use oxarchive::types::OiFundingInterval;

let current = client.hyperliquid.hip3.breadth.current().await?;
println!("HIP-3 above session VWAP: {:?}%", current.value_pct);

let history = client.hyperliquid.hip3.breadth.history(BreadthHistoryParams {
    start: Some(1787961600000_i64.into()),
    end: Some(1788048000000_i64.into()),
    interval: Some(OiFundingInterval::FiveMinutes),
    cursor: None,
    limit: Some(1000),
}).await?;
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

Lighter `funding_rate` values are decimal fractions, not percentages and not
annualized. For example, `0.0001` means `0.01%`. This is a breaking unit
normalization from the former raw percent representation; consumers that
applied a compensating conversion must update it.

#### Aggregation Intervals

| Interval | Description |
|----------|-------------|
| `5m` | 5 minutes |
| `15m` | 15 minutes |
| `30m` | 30 minutes |
| `1h` | 1 hour |
| `4h` | 4 hours |
| `1d` | 1 day |

Raw cadence is route-specific: Hyperliquid core funding is ~1 minute; HIP-3 and Lighter funding are ~10 seconds. HIP-4 has no funding. HIP-3, HIP-4 outcome-side OI, and Lighter OI are also ~10 seconds.

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
    start: 1772323200000_i64.into(), // 2026-03-01 00:00 UTC
    end: 1772409600000_i64.into(),   // 2026-03-02 00:00 UTC
    cursor: None,
    limit: None,
}).await?;

let hip3_vol = client.hyperliquid.hip3.liquidations.volume("km:US500", LiquidationVolumeParams {
    start: 1772323200000_i64.into(), // 2026-03-01 00:00 UTC
    end: 1772409600000_i64.into(),   // 2026-03-02 00:00 UTC
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
```

Projected forced-liquidation price levels refresh approximately every five
minutes. This is an observed cadence, not an exact five-minute guarantee.

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

Hyperliquid, HIP-3, and Lighter candle routes accept at most 10,000 rows per
page. HIP-4 and Spot accept at most 1,000. Lighter candle history is served
from **2025-08-01**; HIP-4 candle history is served from **2026-05-02**.

```rust
let lighter_candles = client.lighter.candles.history("BTC", CandleHistoryParams {
    start: 1754006400000_i64.into(), // 2025-08-01 00:00 UTC
    end: 1754092800000_i64.into(),   // 2025-08-02 00:00 UTC
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneHour),
}).await?;
```

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
    start: 1772323200000_i64.into(), // 2026-03-01 00:00 UTC
    end: 1772409600000_i64.into(),   // 2026-03-02 00:00 UTC
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
    start: 1772668800000_i64.into(), // 2026-03-05 00:00 UTC
    end: 1772755200000_i64.into(),   // 2026-03-06 00:00 UTC
    cursor: None,
    limit: Some(1000),
}).await?;
```

### HIP-4 Outcome Markets (Hyperliquid)

Binary outcome perps deployed under the Hyperliquid namespace. Responses use
`#<10*outcome_id + side>` symbols (`#0`, `#1`, `#55850`, ...). For path
inputs, use the bare numeric form (`"0"`, `"1"`, `"55850"`). Legacy `"#0"`
inputs remain supported and are percent-encoded for transport; do not
pre-encode them yourself.

HIP-4 candles and outcome-side OI are served from **2026-05-02**, with raw OI
updates at ~10s. HIP-4 has **no funding rates and no liquidations**.
`mark_price` on HIP-4 is an implied probability in `[0, 1]`, not a USD price.

```rust
use oxarchive::exchanges::{Hip4HistoryRange, Hip4ListOutcomesParams,
    Hip4OrderBookParams, Hip4TradesParams};
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::CandleInterval;

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
let inst = client.hyperliquid.hip4.get_instrument("0").await?;

// L2 orderbook
let ob = client.hyperliquid.hip4.get_orderbook("0", None).await?;
let ob_at = client.hyperliquid.hip4.get_orderbook("0", Some(Hip4OrderBookParams {
    timestamp: Some(1777680000000_i64.into()),
    depth: Some(20),
})).await?;
let ob_history = client.hyperliquid.hip4.get_orderbook_history("0", Hip4HistoryRange {
    start: 1777680000000_i64.into(),
    end:   1777766400000_i64.into(),
    cursor: None,
    limit: Some(100),
}).await?;

// Trades (history + recent)
let trades = client.hyperliquid.hip4.get_trades("0", Hip4TradesParams {
    start: 1777680000000_i64.into(),
    end:   1777766400000_i64.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;
let recent = client.hyperliquid.hip4.get_trades_recent("0", Some(50)).await?;

// Implied-probability OHLCV candles
let candles = client.hyperliquid.hip4.candles.history("0", CandleHistoryParams {
    start: 1777680000000_i64.into(),
    end: 1777766400000_i64.into(),
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneHour),
}).await?;

// Open interest (per-side history + latest)
let oi_hist = client.hyperliquid.hip4.get_open_interest("0", Hip4HistoryRange {
    start: 1777680000000_i64.into(),
    end:   1777766400000_i64.into(),
    cursor: None,
    limit: None,
}).await?;
let oi_now = client.hyperliquid.hip4.get_open_interest_current("0").await?;
// mark_price on HIP-4 is an implied probability in [0, 1].

// Summary, freshness, prices
let summary    = client.hyperliquid.hip4.get_summary("0").await?;
let freshness  = client.hyperliquid.hip4.get_freshness("0").await?;
let prices     = client.hyperliquid.hip4.get_prices("0",
    1777680000000_i64, 1777766400000_i64, Some("1h"), Some(100), None).await?;

// L4 (current snapshot, diffs, checkpoint history)
let l4_now     = client.hyperliquid.hip4.get_l4_orderbook("0", None).await?;
let l4_diffs   = client.hyperliquid.hip4.get_l4_diffs("0", Hip4HistoryRange {
    start: 1777680000000_i64.into(),
    end:   1777766400000_i64.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
```

### Hyperliquid Spot

Spot trading pairs deployed under the Hyperliquid namespace. Symbols use the
dashed canonical form (`HYPE-USDC`, `PURR-USDC`, ...). The server resolves
the dashed form to the wire format (`PURR/USDC`, `@107`) internally.

Spot has **no funding, no open interest, and no liquidations**. Trade history
backfills to 2025-03-22. Spot candle history starts exactly at
**2025-03-22T10:50:22Z**, supports `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`,
and `1w`, and accepts at most 1,000 rows per page. Orderbook, L4, TWAP, and
freshness data are live-only from 2026-05-05.

```rust
use oxarchive::resources::orderbook::{GetOrderBookParams, OrderBookHistoryParams};
use oxarchive::resources::l4_orderbook::{L4DiffsParams, L4HistoryParams, L4OrderBookParams};
use oxarchive::resources::orders::OrderHistoryParams;
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::spot::SpotTwapParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::types::CandleInterval;

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

// Candles (history starts exactly at 2025-03-22T10:50:22Z; max 1,000 rows).
let candles = client.hyperliquid.spot.candles.history("HYPE-USDC", CandleHistoryParams {
    start: 1742640622000_i64.into(), // 2025-03-22T10:50:22Z
    end:   1742644222000_i64.into(),
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneMinute),
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

## WebSocket Client

Requires the `websocket` feature. Supports two modes on a single connection:
- **Live subscriptions**: supported Hyperliquid and Lighter.xyz live market channels
- **Replay**: bounded historical data with timing preserved

For large historical downloads, use the S3 Parquet bulk export in the [Data Catalog](https://www.0xarchive.io/data). Bulk streaming over WebSocket has been discontinued, and the deprecated `stream()` and `stream_stop()` methods now receive an error from the server.

> Lighter.xyz live subscriptions are available for `lighter_orderbook`, `lighter_trades`, `lighter_open_interest`, and `lighter_funding`. `lighter_candles` and `lighter_l3_orderbook` remain replay-only. All six Lighter channels support historical replay.

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

### Lighter Live Streaming

Live Lighter data uses the same envelope as Hyperliquid live data and is served on
`wss://api.0xarchive.io/ws`, the default `ws_url` (not `wss://stream.0xarchive.io/ws`).
Symbols are the same as `client.lighter.instruments.list()`; they are case-insensitive
on subscribe and echoed uppercase. Live Lighter channels are available on every tier and
are metered per message like Hyperliquid live data; the tier subscription and connection
limits below apply, and each connection accepts at most 10 subscribe operations per second.

```rust
use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions};
use oxarchive::LighterLiveData;

let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key"));
ws.connect().await?;
let mut rx = ws.rx.take().expect("receiver");

// One full book per second by default...
ws.subscribe("lighter_orderbook", Some("BTC")).await?;
// ...or set interval_ms (100 to 5000) on lighter_orderbook only.
ws.subscribe_with_interval("lighter_orderbook", "ETH", 250).await?;
ws.subscribe("lighter_trades", Some("BTC")).await?;
ws.subscribe("lighter_funding", Some("BTC")).await?;

while let Some(msg) = rx.recv().await {
    match msg.lighter_live_data() {
        Some(Ok(LighterLiveData::OrderBook(book))) => {
            let best_bid = book.bids().first().map(|l| l.px.as_str());
            let best_ask = book.asks().first().map(|l| l.px.as_str());
            println!("{} {best_bid:?} / {best_ask:?} at {}", book.coin, book.time);
        }
        Some(Ok(LighterLiveData::Trades(fills))) => {
            // Two fills per trade (one per side) share a tid.
            for fill in fills.iter().filter(|f| f.crossed) {
                println!("{} trade {}: {} @ {}", fill.coin, fill.tid, fill.sz, fill.px);
            }
        }
        Some(Ok(LighterLiveData::OpenInterest(stats) | LighterLiveData::Funding(stats))) => {
            println!("{} funding {:?} OI {:?}", stats.coin, stats.ctx.funding, stats.ctx.open_interest);
        }
        Some(Err(e)) => eprintln!("Unexpected Lighter payload: {e}"),
        None => {
            if let ServerMsg::Error { message } = msg {
                // Lag notices do not always end the subscription (see below).
                eprintln!("{message}");
            }
        }
    }
}
```

| Channel | `data` payload | Rate |
|---------|----------------|------|
| `lighter_orderbook` | `LighterLiveOrderBook`: `coin`, `time` (ms), `levels` = `[bids, asks]`, best first, up to 20 levels per side. Each level has `px` and `sz` as decimal strings exactly as Lighter publishes them, and `n`, which is always `1` (Lighter does not publish per-level order counts). Every message is a full book, not a diff. | The newest book at most once per interval (default 1000 ms, `interval_ms` 100 to 5000). The current book is sent right after subscribing when one is available. |
| `lighter_trades` | `Vec<LighterLiveTrade>`: two fills per trade, one per side, with the same `tid`. `side` is `"A"` (ask side) or `"B"` (bid side), `crossed: true` is the taker fill, `users` holds the Lighter account index as a string, `oid` is that side's order id, `start_position` is that account's signed position before the trade, and `hash` is the Lighter transaction hash. `fee`, `fee_token`, `closed_pnl`, and `dir` are always `None`. | As trades happen, typically batched within about 100 ms. |
| `lighter_open_interest`, `lighter_funding` | `LighterLiveMarketStats`: both channels carry the same message, `coin` plus `ctx` with `openInterest`, `funding`, `premium`, `markPx`, `oraclePx` (Lighter's index price), `midPx`, `dayNtlVlm` (24h quote volume), `dayBaseVlm` (24h base volume), `prevDayPx`, and `impactPxs` (always `null`). `funding` and `premium` are decimal fractions, the same unit as REST `funding_rate`. | As Lighter publishes them, about once per second per market. The latest values are sent right after subscribing when available. |

Count trades by distinct `tid`, not by array length, and compute volume by summing
`sz` over one fill per `tid`. Live trades are preliminary. The finalized record,
including fields the live stream does not carry such as fees, is served by
`client.lighter.trades.list(...)` (`GET /v1/lighter/trades/{symbol}`), which returns
reconciled trades only; `client.lighter.trades.recent(...)` serves the preliminary tier.

If your connection falls behind `lighter_trades`, `lighter_open_interest`, or
`lighter_funding`, the server sends an `error` notice such as
`Dropped ~N live lighter_trades messages for BTC: ...` and the subscription continues.
If the lag persists, a notice such as `Stopped the lighter_trades stream for BTC: ...`
ends that subscription; subscribe again to resume. `lighter_orderbook` never sends an
older book in place of a newer one.

### Historical Replay

Replay a bounded historical window with original timing preserved. Every replay
ends with a `replay_completed` server message.

```rust
use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions};

let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key"));
ws.connect().await?;
let mut rx = ws.rx.take().expect("receiver");

// All six Lighter channels support replay. Replay rows keep their stored
// shapes, which differ from the live Lighter payloads above.
ws.replay(
    "lighter_orderbook",
    "BTC",
    1788048000000,       // 2026-08-30 00:00 UTC
    Some(1788051600000), // 2026-08-30 01:00 UTC
    Some(100.0),
).await?;

while let Some(msg) = rx.recv().await {
    match msg {
        ServerMsg::ReplayCompleted { channel, snapshots_sent, .. } => {
            println!("Replay complete: {channel}, {snapshots_sent:?} records");
            break;
        }
        ServerMsg::HistoricalData { channel, .. } => {
            println!("Historical {channel} record");
        }
        _ => {}
    }
}

// Control playback for the active bounded replay
ws.replay_pause().await?;
ws.replay_resume().await?;
ws.replay_seek(1788049800000).await?;
ws.replay_stop().await?;
```

### Available Channels

| Channel | Description | Live Subscription | Historical Replay |
|---------|-------------|-------------------|-------------------|
| `orderbook` | L2 order book (~1.2s resolution) | Yes | Yes |
| `trades` | Trade/fill updates | Yes | Yes |
| `candles` | OHLCV candle data | No | Yes |
| `liquidations` | Liquidation events. Each item is a fill row with `is_liquidation: true`. | Yes | Yes |
| `open_interest` | Open interest snapshots | Yes | Yes |
| `funding` | Funding rate snapshots | Yes | Yes |
| `ticker` | Price and 24h volume | Yes | No |
| `all_tickers` | All market tickers | Yes | No |
| `lighter_orderbook` | Lighter.xyz L2 order book | Yes | Yes |
| `lighter_trades` | Lighter.xyz trades | Yes | Yes |
| `lighter_candles` | Lighter.xyz candles | No | Yes |
| `lighter_open_interest` | Lighter.xyz open interest | Yes | Yes |
| `lighter_funding` | Lighter.xyz funding rates | Yes | Yes |
| `lighter_l3_orderbook` | Lighter.xyz L3 order-level orderbook | No | Yes |
| `hip3_orderbook` | HIP-3 L2 order book | Yes | Yes |
| `hip3_trades` | HIP-3 trades | Yes | Yes |
| `hip3_candles` | HIP-3 candles | Yes | Yes |
| `hip3_open_interest` | HIP-3 open interest | No | Yes |
| `hip3_funding` | HIP-3 funding rates | No | Yes |
| `hip3_liquidations` | HIP-3 liquidation events. Same wire shape as `liquidations`. | Yes | Yes |
| `hip4_orderbook` | HIP-4 outcome-market L2 order book | No | Yes |
| `hip4_trades` | HIP-4 trade/fill updates | Yes | Yes |
| `hip4_open_interest` | HIP-4 open interest snapshots | No | Yes |
| `l4_diffs` | Hyperliquid core L4 orderbook diffs with user attribution | Yes | Yes, `l4_snapshot` then ordered `l4_batch` |
| `l4_orders` | Hyperliquid core L4 order lifecycle events | Yes | Yes, `l4_snapshot` then ordered `l4_batch` |
| `hip3_l4_diffs` | HIP-3 L4 orderbook diffs with user attribution | Yes | No, live-only |
| `hip3_l4_orders` | HIP-3 order lifecycle events | Yes | No, live-only |
| `hip4_l4_diffs` | HIP-4 L4 orderbook diffs with user attribution | Yes | No, live-only |
| `hip4_l4_orders` | HIP-4 order lifecycle events | Yes | No, live-only |
| `spot_orderbook` | Hyperliquid Spot L2 order book | Yes | No |
| `spot_trades` | Hyperliquid Spot trades | Yes | No |
| `spot_l4_diffs` | Hyperliquid Spot L4 orderbook diffs with user attribution | Yes | No, live-only |
| `spot_l4_orders` | Hyperliquid Spot order lifecycle events | Yes | No, live-only |
| `spot_twap` | Hyperliquid Spot TWAP execution updates | Yes | No |

Current Lighter order books, trades, open interest, and funding are available as live subscriptions and through the Lighter REST resources; current Lighter candles and L3 order books are available through REST. Historical Lighter data is available through REST, WebSocket replay, or exports.

HIP-4 has no funding or liquidation channels. HIP-4 candles and current outcome-side OI are available over REST from 2026-05-02; the live HIP-4 order-book and OI bridges are paused, while stored replay remains available. This HIP-4 channel set has no dedicated candle channel.

Hyperliquid Spot has no funding, open-interest, or liquidation resources. Spot candle history is REST-served from 2025-03-22T10:50:22Z and has no dedicated WebSocket channel. Spot symbols use the dashed canonical form (`HYPE-USDC`, `PURR-USDC`).

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

All self-serve tiers reach the published route families; Free covers the most recent rolling 30 days of history (30-day span per request or replay), and Build and above keep the retained archive. Schema availability remains family-specific; plans gate capacity and Free's 30-day history window, not route families, schemas, or served depth.

| Tier | Max Subscriptions | Max Connections | Max Replay Speed |
|------|------------------|-----------------|------------------|
| Free | 10 | 2 | 10x |
| Build | 500 | 3 | 50x |
| Pro | 3,000 | 5 | 100x |
| Scale | 20,000 | 16 | 300x |
| Enterprise | Custom | Custom | from 500x |

On every tier, each WebSocket connection accepts at most 10 subscribe operations per second;
faster bursts are rejected with a "Subscription rate limit exceeded" error.

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

# WebSocket (requires websocket feature)
cargo run --example websocket --features websocket
```

## Data Catalog

For large-scale data exports (route-specific order books, fill-level trade history, and other retained datasets), use the [Data Catalog](https://www.0xarchive.io/data). It lets you choose markets, datasets, and date ranges, see a live quote, and export zstd-compressed Parquet.

## Links

- [API Docs](https://www.0xarchive.io/docs)
- [Python SDK](https://pypi.org/project/oxarchive/)
- [TypeScript SDK](https://npmjs.com/package/@0xarchive/sdk)
- [CLI](https://npmjs.com/package/@0xarchive/cli)
- [MCP Server](https://mcp.0xarchive.io)
- [0xArchive Skill](https://github.com/0xArchiveIO/0xarchive-skill)
- [Examples](https://github.com/0xArchiveIO/examples)

## Requirements

- Rust 1.75+
- tokio runtime

## License

MIT
