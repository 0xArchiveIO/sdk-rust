# oxarchive

Rust client for async services that need typed 0xArchive market data.

0xArchive is granular market data infrastructure for Hyperliquid and Lighter.xyz. HIP-3 builder perps live under the Hyperliquid namespace at `/v1/hyperliquid/hip3`.

Use this SDK when the integration belongs in an async Rust service, data system, backtest runner, or strongly typed market-data pipeline.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
oxarchive = "1.4"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

For WebSocket support (real-time streaming, replay, bulk download):

```toml
oxarchive = { version = "1.4", features = ["websocket"] }
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
| Claude Code, GPT Codex, and coding-agent workflows | [AI Clients](https://www.0xarchive.io/docs/ai-clients) |
| File-based historical pulls | [Data Catalog](https://www.0xarchive.io/data) |
| Route contract and machine context | [OpenAPI](https://www.0xarchive.io/openapi.json), [llms.txt](https://www.0xarchive.io/llms.txt) |

## Data Coverage

| Venue | Coverage | Notes |
| --- | --- | --- |
| Hyperliquid | April 2023+ | Perpetuals across the full venue |
| Hyperliquid HIP-3 | February 2026+ | Free tier: `km:US500`. Build+: all HIP-3 symbols. Pro+: orderbook history. |
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

| Resource | `client.hyperliquid` | `client.hyperliquid.hip3` | `client.lighter` |
|----------|---------------------|--------------------------|-------------------|
| `orderbook` | Yes | Yes | Yes |
| `trades` | Yes | Yes | Yes |
| `instruments` | Yes | Yes | Yes |
| `funding` | Yes | Yes | Yes |
| `open_interest` | Yes | Yes | Yes |
| `candles` | Yes | Yes | Yes |
| `liquidations` | Yes | Yes | -- |
| `orders` | Yes | Yes | -- |
| `l4_orderbook` | Yes | Yes | -- |
| `l2_orderbook` | Yes | Yes | -- |
| `l3_orderbook` | -- | -- | Yes |
| `freshness()` | Yes | Yes | Yes |
| `summary()` | Yes | Yes | Yes |
| `price_history()` | Yes | Yes | Yes |

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

#### Orderbook Depth Limits

| Tier | Max Depth |
|------|-----------|
| Free | 20 |
| Build | 200 |
| Pro | Full Depth |
| Enterprise | Full Depth |

**Note:** Hyperliquid L2 source data contains 20 levels. Full-depth L2 (derived from L4) and Lighter.xyz provide full depth on Pro+. Depth limits apply to L2 snapshot endpoints only — L4 and L2 diff endpoints return full data.

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

| Granularity | Interval | Tier Required | Credit Multiplier |
|-------------|----------|---------------|-------------------|
| `Checkpoint` | ~60s | Free+ | 1x |
| `ThirtySeconds` | 30s | Build+ | 2x |
| `TenSeconds` | 10s | Build+ | 3x |
| `OneSecond` | 1s | Pro+ | 10x |
| `Tick` | tick-level | Enterprise | 20x |

### Orderbook Reconstruction (Enterprise)

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

// Get current L2 full-depth orderbook (Build+ tier)
let l2 = client.hyperliquid.l2_orderbook.get("BTC", None).await?;

// Get L2 orderbook at a specific timestamp
let l2 = client.hyperliquid.l2_orderbook.get("BTC", Some(L2OrderBookParams {
    timestamp: Some(1704067200000_i64.into()),
    depth: Some(50),
})).await?;

// Get L2 orderbook history (Build+ tier)
let l2_history = client.hyperliquid.l2_orderbook.history("BTC", L2HistoryParams {
    start: 1704067200000_i64.into(),
    end: 1704153600000_i64.into(),
    cursor: None,
    limit: Some(1000),
    depth: Some(50),
}).await?;

// Get L2 tick-level diffs (Pro+ tier)
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

### Freshness

Check when each data type was last updated for a specific coin.

```rust
let freshness = client.hyperliquid.freshness("BTC").await?;
let lighter_freshness = client.lighter.freshness("BTC").await?;
let hip3_freshness = client.hyperliquid.hip3.freshness("km:US500").await?;
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
- **Real-time** — subscribe to live market data
- **Replay** — replay historical data with timing preserved

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
        ServerMsg::Data { channel, coin, data } => {
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

| Channel | Description | Historical |
|---------|-------------|-----------|
| `orderbook` | L2 order book (~1.2s resolution) | Yes |
| `trades` | Trade/fill updates | Yes |
| `candles` | OHLCV candle data | Yes |
| `liquidations` | Liquidation events (May 2025+) | Yes |
| `open_interest` | Open interest snapshots (May 2023+) | Yes |
| `funding` | Funding rate snapshots (May 2023+) | Yes |
| `ticker` | Price and 24h volume | Real-time only |
| `all_tickers` | All market tickers | Real-time only |
| `lighter_orderbook` | Lighter.xyz L2 order book | Yes |
| `lighter_trades` | Lighter.xyz trades | Yes |
| `lighter_candles` | Lighter.xyz candles | Yes |
| `lighter_open_interest` | Lighter.xyz open interest | Yes |
| `lighter_funding` | Lighter.xyz funding rates | Yes |
| `lighter_l3_orderbook` | Lighter.xyz L3 order-level orderbook (Pro+) | Yes |
| `hip3_orderbook` | HIP-3 L2 order book | Yes |
| `hip3_trades` | HIP-3 trades | Yes |
| `hip3_candles` | HIP-3 candles | Yes |
| `hip3_open_interest` | HIP-3 open interest | Yes |
| `hip3_funding` | HIP-3 funding rates | Yes |
| `hip3_liquidations` | HIP-3 liquidation events (Feb 2026+) | Yes |
| `l4_diffs` | Hyperliquid L4 orderbook diffs with user attribution (Pro+) | Real-time only |
| `l4_orders` | Hyperliquid order lifecycle events (Pro+) | Real-time only |
| `hip3_l4_diffs` | HIP-3 L4 orderbook diffs with user attribution (Pro+) | Real-time only |
| `hip3_l4_orders` | HIP-3 order lifecycle events (Pro+) | Real-time only |

### Tier Limits

| Tier | Max Subscriptions | Max Replay Speed | Max Batch Size |
|------|------------------|------------------|----------------|
| Free | — | — | — |
| Build | 25 | 50x | 2,000 |
| Pro | 100 | 100x | 5,000 |
| Enterprise | 200 | 1000x | 10,000 |

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

# WebSocket (requires websocket feature)
cargo run --example websocket --features websocket
```

## Data Catalog

For large-scale data exports (full order books, complete trade history, etc.), use the [Data Catalog](https://www.0xarchive.io/data). It lets you choose markets, datasets, and date ranges, see a live quote, and export zstd-compressed Parquet.

## Requirements

- Rust 1.75+
- tokio runtime

## License

MIT
