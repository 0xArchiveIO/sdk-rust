# oxarchive

[![Crates.io](https://img.shields.io/crates/v/oxarchive.svg)](https://crates.io/crates/oxarchive) [![Docs.rs](https://docs.rs/oxarchive/badge.svg)](https://docs.rs/oxarchive) [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Rust client for async services that need typed 0xArchive market data.

0xArchive is granular market data infrastructure for two venues: Hyperliquid and Lighter. Hyperliquid includes core perps (`/v1/hyperliquid`), HIP-3 builder perps (`/v1/hyperliquid/hip3`), HIP-4 outcome markets (`/v1/hyperliquid/hip4`), and Hyperliquid Spot (`/v1/hyperliquid/spot`). Lighter has two deployments: mainnet (`/v1/lighter`, `client.lighter`) and Robinhood Chain (`/v1/rh-lighter`, `client.rh_lighter`).

Use this SDK when the integration belongs in an async Rust service, data system, backtest runner, or strongly typed market-data pipeline.

## Installation

```bash
cargo add oxarchive
```

Or add directly to your `Cargo.toml`:

```toml
[dependencies]
oxarchive = "1.13"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
chrono = "0.4" # the examples below use it for time windows
```

For WebSocket support (real-time streaming and replay):

```toml
oxarchive = { version = "1.13", features = ["websocket"] }
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

    // Lighter uses its own venue client
    let lighter_ob = client.lighter.orderbook.get("BTC", None).await?;
    println!("Lighter BTC mid price: {:?}", lighter_ob.mid_price);

    // Lighter on Robinhood Chain, the second Lighter deployment (USDG-quoted)
    let rh_ob = client.rh_lighter.orderbook.get("BTC", None).await?;
    println!("Lighter (Robinhood Chain) BTC mid price: {:?}", rh_ob.mid_price);

    // Open positions of a wallet, from the latest live snapshot
    let wallet = client.hyperliquid.positions
        .get("0x0000000000000000000000000000000000000001", None).await?;
    println!("{} open positions as of {:?}", wallet.data.positions.len(), wallet.meta.as_of);

    // Hyperliquid HIP-3 builder perps stay under client.hyperliquid.hip3
    let hip3 = client.hyperliquid.hip3.instruments.list().await?;
    let hip3_ob = client.hyperliquid.hip3.orderbook.get("xyz:XYZ100", None).await?;
    let hip3_funding = client.hyperliquid.hip3.funding.current("xyz:XYZ100").await?;
    println!(
        "{} HIP-3 markets; xyz:XYZ100 mid price {:?}, funding {}",
        hip3.len(), hip3_ob.mid_price, hip3_funding.funding_rate,
    );

    // Hyperliquid Spot stays under client.hyperliquid.spot (dashed canonical pairs)
    let pairs = client.hyperliquid.spot.pairs.list().await?;
    let hype_ob = client.hyperliquid.spot.orderbook.get("HYPE-USDC", None).await?;
    println!("{} spot pairs; HYPE-USDC mid price: {:?}", pairs.len(), hype_ob.mid_price);

    // Order book snapshots from the last hour
    use oxarchive::resources::orderbook::OrderBookHistoryParams;
    let end = chrono::Utc::now();
    let start = end - chrono::Duration::hours(1);
    let history = client.hyperliquid.orderbook.history("ETH", OrderBookHistoryParams {
        start: start.into(),
        end: end.into(),
        cursor: None,
        limit: Some(100),
        depth: None,
        granularity: None,
    }).await?;
    println!("{} ETH snapshots in the last hour", history.data.len());

    Ok(())
}
```

The examples in this README read the most recent minutes, hours or days, so
they work on every plan, including Free's rolling 30-day window. Each dataset's first
available instant is listed in [Data Coverage](#data-coverage) and returned by
[`client.capabilities()`](#capabilities).

## Choose Your Next Path

| Need | Link |
| --- | --- |
| First authenticated route | [Quick Start](https://docs.0xarchive.io/quickstart) |
| SDK install and route docs | [SDK docs](https://docs.0xarchive.io/sdks) |
| Claude Code, ChatGPT Codex, and coding-agent workflows | [AI Clients](https://docs.0xarchive.io/ai-clients) |
| File-based historical pulls | [Data Catalog](https://www.0xarchive.io/data) |
| Route contract and machine context | [OpenAPI](https://www.0xarchive.io/openapi.json), [llms.txt](https://www.0xarchive.io/llms.txt) |

## Use From Coding Agents

When prototyping Rust services from Claude Code, ChatGPT Codex, or another coding agent, also install the [0xArchive skill](https://github.com/0xArchiveIO/0xarchive-skill) so the agent has typed API context and example patterns for every endpoint. The skill installs into `.claude/skills/0xarchive` (Claude Code) or `.agents/skills/0xarchive` (ChatGPT Codex). For shell-driven exploration alongside the SDK, the [CLI](https://npmjs.com/package/@0xarchive/cli) and the [hosted MCP](https://docs.0xarchive.io/mcp-server) share the same API key.

## Data Coverage

Dates are UTC and give each dataset's first available instant, as returned
by `client.capabilities()`. Exact starts can be later for markets listed after
them.

| Venue | Coverage | Notes |
| --- | --- | --- |
| Hyperliquid | Order book from 2023-04-15; trades from 2023-04-15 03:31; funding, OI, and price history from 2023-05-20 02:50; candles from 2025-03-22 12:00; liquidations from 2025-12-22; L4, order history, and full-depth L2 from 2026-03-11 01:03 | Core perpetuals. |
| Hyperliquid HIP-3 | Price history from 2025-10-13 12:12; trades from 2025-10-13 12:24; candles and liquidations from 2025-12-22; order book from 2026-02-16 16:57; funding and OI from 2026-02-16 17:03; L4, order history, and full-depth L2 from 2026-03-11 01:03 | Builder perps; funding and OI update at roughly 10 seconds. |
| Hyperliquid HIP-4 | L4 and order history from 2026-05-02 07:47; trades and candles from 2026-05-02 08:00; order book, outcome-side OI, and price history from 2026-05-02 16:51 | Outcome markets; OI updates at ~10s. No funding or liquidations. |
| Hyperliquid Spot | Candles from 2025-03-22 10:50; trades from 2025-03-22 10:50:22; TWAP from 2026-05-05 13:05; order book from 2026-05-05 19:56; L4 and order history from 2026-05-05 22:57 | Dashed symbols (`HYPE-USDC`, `PURR-USDC`, ...); list them with `client.hyperliquid.spot.pairs.list()`. No funding, OI, or liquidations. |
| Lighter (mainnet) | Trades from 2025-01-17 08:43 (exact starts vary by market); candles from 2025-08-01; funding, OI, and price history from 2025-08-25 15:28; order book from 2026-01-29 02:13; L3 from 2026-03-05 03:33; liquidations from 2026-06-10 | Maker/taker trade context; L3 caps at 250 orders per side; funding/OI update at ~10s. |
| Lighter (Robinhood Chain) | Candles from 2026-06-26 20:10; trades and liquidations from 2026-06-26 20:10:26 (venue launch); order book, OI, funding, and price history from 2026-08-22 18:43 | Second Lighter deployment, quoted in USDG, with perps (`BTC`) and spot pairs (`AAPL-USDG`); list them with `client.rh_lighter.instruments.list()`. No L3. |
| Account positions | Hyperliquid core change log from 2025-05-25, HIP-3 from 2025-10-13, hourly snapshots from 2026-06-07, live every 5 minutes. Lighter mainnet from 2025-01-17, Robinhood Chain from 2026-06-26, hourly, live every 2 minutes | Perp positions per wallet or account, as of any time in coverage. See [Account Positions](#account-positions). |

## Configuration

```rust
use oxarchive::OxArchive;
use std::time::Duration;

let client = OxArchive::builder("0xa_your_api_key")
    .base_url("https://api.0xarchive.io")  // Optional
    .timeout(Duration::from_secs(60))       // Optional (default: 30s)
    .build()?;
```

Every request sends the `0xArchive-Version: 2026-10-01` header
(`oxarchive::API_VERSION`), and the WebSocket client connects with
`version=2026-10-01`. The version selects the response shapes the SDK types
describe: the standard `{success, data, meta}` envelope on every route,
RFC 3339 UTC times with an integer `*_ms` field alongside, and the stable
error codes in [Error Handling](#error-handling).

## Pagination

Paged methods return a `CursorResponse` (the same type as `MetaResponse`):
the page's `data`, `next_cursor`, `has_more`, and the full `meta` block. Pass
`next_cursor` back as `cursor`, with every other parameter unchanged, while
`has_more` is `true`. A last page can be empty when the page before it was
exactly full. Per-symbol routes also report the canonical symbol and the
venue they answered for in `meta.symbol` and `meta.venue` (a HIP-4 `"0"` is
answered as `#0`).

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::trades::GetTradesParams;

let end = Utc::now();
let start = end - Duration::minutes(10);
let mut cursor = None;
loop {
    let page = client.hyperliquid.trades.history("BTC", GetTradesParams {
        start: start.into(),
        end: end.into(),
        cursor,
        limit: Some(1000),
        side: None,
    }).await?;
    println!("{} rows for {:?} on {:?}", page.data.len(), page.meta.symbol, page.meta.venue);
    if !page.has_more {
        break;
    }
    cursor = page.next_cursor;
}
```

## Symbols

`client.symbols()` lists the public symbol universe across every venue family
in one response: each market's symbol, venue family (`hyperliquid`, `hip3`,
`hip4`, `spot`, `lighter`, `rh-lighter`), coverage start by data type, and
estimated size per day. HIP-4 entries also carry the slug, side pair, title,
and settlement state.

```rust
let symbols = client.symbols().await?;
let hip3: Vec<_> = symbols.iter().filter(|s| s.exchange == "hip3").collect();
println!("{} symbols, {} on HIP-3", symbols.len(), hip3.len());
```

`client.list_symbols()` is the same call.

## Capabilities

`client.capabilities()` returns what each venue serves (`GET /v1/capabilities`),
one `Capability` row per venue and datatype: its REST routes, its WebSocket
channels, whether those stream live (`live`) and replay history (`replay`),
the first instant served (`available_from`), the cadence, the largest page
(`page_limit`), and the accepted `interval` values. The `mempool` row is the one
that sets `ws_endpoint` (the endpoint that serves it) and `plans` (the plans that
include it); rows without them use the default endpoint, `wss://api.0xarchive.io/ws`,
and are on every plan. The route is public and uses no credits. Read replay availability from it rather than from a fixed
list:

```rust
use oxarchive::Capability;

let rows = client.capabilities().await?;
if let Some(row) = Capability::for_channel(&rows, "spot_l4_diffs") {
    println!("spot_l4_diffs: live={} replay={} from {:?}", row.live, row.replay, row.available_from);
}
```

## REST API Reference

The sections below show which resources are available on each exchange client:

| Resource | `client.hyperliquid` | `client.hyperliquid.hip3` | `client.hyperliquid.spot` | `client.lighter` | `client.rh_lighter` |
|----------|---------------------|--------------------------|-------------------------|-------------------|---------------------|
| `orderbook` | Yes | Yes | Yes | Yes | Yes |
| `trades` | Yes | Yes | Yes | Yes | Yes |
| `instruments` | Yes | Yes | -- (use `pairs`) | Yes | Yes |
| `pairs` | -- | -- | Yes | -- | -- |
| `funding` | Yes | Yes | -- | Yes | Yes |
| `open_interest` | Yes | Yes | -- | Yes | Yes |
| `candles` | Yes | Yes | Yes | Yes | Yes |
| `breadth` (above session VWAP) | Yes | Yes | -- | -- | -- |
| `liquidations` | Yes | Yes | -- | Yes | Yes |
| `positions` | Yes | Yes | -- | Yes | Yes |
| `wallets` (wallet classification) | Yes | Yes | -- | -- | -- |
| `oracle` (external price, discovery bounds) | -- | Yes | -- | -- | -- |
| `accounts` (L1 address lookup) | -- | -- | -- | Yes | -- |
| `orders` | Yes | Yes | History only | -- | -- |
| `l4_orderbook` | Yes | Yes | Yes | -- | -- |
| `l2_orderbook` | Yes | Yes | -- | -- | -- |
| `l3_orderbook` | -- | -- | -- | Yes | -- |
| `twap` | -- | -- | Yes | -- | -- |
| `freshness()` | Yes | Yes | Yes | Yes | Yes |
| `summary()` | Yes | Yes | -- | Yes | Yes |
| `price_history()` | Yes | Yes | -- | Yes | Yes |
| `cvd()` (cumulative volume delta) | Yes | Yes | -- | -- | -- |

HIP-4 outcome markets (`client.hyperliquid.hip4`) have their own methods, listed in [HIP-4 Outcome Markets](#hip-4-outcome-markets-hyperliquid). Webhook management is on `client.webhooks`; see [Webhooks](#webhooks).

### Order Book

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::orderbook::{GetOrderBookParams, OrderBookHistoryParams};

// Get current order book
let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
println!("Mid price: {:?}", ob.mid_price);
println!("Best bid: {:?}", ob.bids.first());
println!("Best ask: {:?}", ob.asks.first());

// Get the book as of a timestamp (one hour ago), 20 levels per side
let end = Utc::now();
let start = end - Duration::hours(1);
let ob = client.hyperliquid.orderbook.get("BTC", Some(GetOrderBookParams {
    timestamp: Some(start.into()),
    depth: Some(20),
})).await?;

// Get historical snapshots
let history = client.hyperliquid.orderbook.history("BTC", OrderBookHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
    depth: None,
    granularity: None,
}).await?;
```

#### Orderbook Depth

Depth is route-specific. Hyperliquid-family native L2 is capped at 20 levels per side. Lighter native L2 includes all served levels, and Lighter L3 is capped at 250 orders per side.

**Note:** Dedicated L2 routes derived from L4 return all served levels where supported. `depth` limits the levels per side on order book history for every venue (`OrderBookHistoryParams::depth`, `Hip4OrderBookHistoryParams::depth`) and on full-depth L2 history (`L2HistoryParams::depth`). Lighter L3 exposes individual resting orders rather than price levels and begins at 2026-03-05 03:33 UTC.

#### Lighter Orderbook Granularity

Lighter orderbook history supports a `granularity` parameter for different data resolutions, on both deployments (`client.lighter` and `client.rh_lighter`):

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::orderbook::OrderBookHistoryParams;
use oxarchive::types::LighterGranularity;

let end = Utc::now();
let history = client.lighter.orderbook.history("BTC", OrderBookHistoryParams {
    start: (end - Duration::hours(1)).into(),
    end: end.into(),
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
use chrono::{Duration, Utc};
use oxarchive::OxArchive;
use oxarchive::orderbook_reconstructor::OrderBookReconstructor;

let client = OxArchive::new("your-api-key")?;

// The last ten minutes, in Unix milliseconds
let end = Utc::now().timestamp_millis();
let start = end - Duration::minutes(10).num_milliseconds();

// Option 1: One-shot: fetch and reconstruct in one call
let snapshots = client.lighter.orderbook.history_reconstructed(
    "BTC",
    start,
    end,
    Some(20),           // depth (top 20 levels per side)
    true,               // emit_all: snapshot after every tick
).await?;

for snapshot in &snapshots {
    println!("{}: mid={:?}, spread={:?}bps, seq={:?}",
        snapshot.timestamp, snapshot.mid_price, snapshot.spread_bps, snapshot.sequence);
}

// Option 2: Auto-paginated: fetches all pages automatically
let all_snapshots = client.lighter.orderbook.collect_tick_history(
    "BTC",
    start,
    end,
    Some(20),
).await?;
println!("{} tick-level snapshots", all_snapshots.len());

// Option 3: Manual control: fetch raw tick data and reconstruct yourself
let tick_data = client.lighter.orderbook.history_tick(
    "BTC", start, end, None,
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

For large time ranges where memory is a concern, paginate manually instead of using `collect_tick_history`. The first page carries the checkpoint at or before `start`; each later page carries only deltas to apply on top of the running book. A page holds a bounded number of deltas (100 by default), so continue while `has_more` is `true`, passing the page's cursor back with the same `start`, `end`, and `depth`:

```rust
use chrono::{Duration, Utc};
use oxarchive::orderbook_reconstructor::OrderBookReconstructor;
use oxarchive::resources::orderbook::TickPageParams;

let end = Utc::now().timestamp_millis();
let start = end - Duration::minutes(10).num_milliseconds();
let mut params = TickPageParams {
    depth: Some(20),
    ..TickPageParams::new(start, end)
};
let mut reconstructor = OrderBookReconstructor::new();

loop {
    let page = client.lighter.orderbook.history_tick_page("BTC", params.clone()).await?;
    if let Some(checkpoint) = &page.checkpoint {
        reconstructor.initialize(checkpoint);
    }
    for delta in &page.deltas {
        reconstructor.apply_delta(delta);
    }
    let book = reconstructor.get_snapshot(Some(20));
    println!("{}: {} bids, {} asks", book.timestamp, book.bids.len(), book.asks.len());

    if !page.has_more {
        break;
    }
    params = params.after(&page);
}
```

### Trades

Cursor-based pagination for efficient retrieval of large datasets.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::trades::{GetTradesParams, RecentTradesParams};
use oxarchive::types::TradeSide;

let end = Utc::now();
let start = end - Duration::minutes(10);

// Get trades with pagination (`history()` and `list()` are the same call)
let result = client.hyperliquid.trades.history("BTC", GetTradesParams {
    start: start.into(),
    end: end.into(),
    limit: Some(1000),
    cursor: None,
    side: None,
}).await?;

// Paginate through all results
let mut all_trades = result.data;
let mut has_more = result.has_more;
let mut cursor = result.next_cursor;
while has_more {
    let page = client.hyperliquid.trades.history("BTC", GetTradesParams {
        start: start.into(),
        end: end.into(),
        limit: Some(1000),
        cursor: cursor.take(),
        side: None,
    }).await?;
    all_trades.extend(page.data);
    has_more = page.has_more;
    cursor = page.next_cursor;
}

// Only the buy side (`side` "B"); `TradeSide::Sell` keeps "A"
let buys = client.hyperliquid.hip3.trades.history("xyz:XYZ100", GetTradesParams {
    start: start.into(),
    end: end.into(),
    limit: Some(1000),
    cursor: None,
    side: Some(TradeSide::Buy),
}).await?;

// Get recent trades (HIP-3, HIP-4, Spot, Lighter and Robinhood Chain)
let recent = client.lighter.trades.recent("BTC", Some(100)).await?;
let rh_recent = client.rh_lighter.trades.recent("BTC", Some(100)).await?;
let hip3_recent = client.hyperliquid.hip3.trades.recent("xyz:XYZ100", Some(50)).await?;
let spot_sells = client.hyperliquid.spot.trades.recent_with("HYPE-USDC", RecentTradesParams {
    limit: Some(50),
    side: Some(TradeSide::Sell),
}).await?;
```

**Note:** Recent trades are available for HIP-3, Spot, Lighter, and Lighter on Robinhood Chain through `recent()`, and for HIP-4 through `hip4.get_trades_recent()`. Hyperliquid core perps do not have a recent trades endpoint; use `history()` with a time range instead.

`side` filters on each row's own side, before paging: where a trade is returned as one fill per side (maker and taker), `TradeSide::Buy` keeps the buying fill of each trade. Keep it unchanged while paging. HIP-4 takes the same filter on `Hip4TradesParams::side` and `get_trades_recent_with()`.

On both Lighter deployments, `list()` returns final trades only: `end` is clamped to the finalization boundary, which runs about a day behind, so a range that reaches past it ends early with `has_more` `false`. `recent()` serves the newer, preliminary tier. The page's `meta` block says where the boundary is; `list_with_meta()` and `recent_with_meta()` return the same rows as a `MetaResponse`:

- `meta.finalized_through`: every trade before it is final.
- `meta.requested_end` and `meta.clamped_to`: set when your `end` was past the boundary; `clamped_to` is where the range stopped.
- `meta.preliminary_row_count` (on `recent_with_meta()`): how many rows in the response are not final yet.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::trades::GetTradesParams;

// The last two days: the newest part is past the finalization boundary.
let end = Utc::now();
let page = client.rh_lighter.trades.list_with_meta("BTC", GetTradesParams {
    start: (end - Duration::days(2)).into(),
    end: end.into(),
    limit: Some(1000),
    cursor: None,
    side: None,
}).await?;
if let Some(boundary) = &page.meta.clamped_to {
    println!("range stopped at {boundary}; the rest is not final yet");
}

let recent = client.lighter.trades.recent_with_meta("BTC", Some(100)).await?;
println!(
    "{} rows, {:?} preliminary, final through {:?}",
    recent.data.len(),
    recent.meta.preliminary_row_count,
    recent.meta.finalized_through,
);
```

### Instruments

```rust
// List all Hyperliquid instruments
let instruments = client.hyperliquid.instruments.list().await?;
for inst in &instruments {
    println!("{}: {}x leverage", inst.name, inst.max_leverage.unwrap_or(0));
}

// Get specific instrument
let btc = client.hyperliquid.instruments.get("BTC").await?;

// Lighter instruments (different schema with fees, market IDs)
let lighter_instruments = client.lighter.instruments.list().await?;
for inst in &lighter_instruments {
    println!("{}: taker_fee={:?}, maker_fee={:?}", inst.symbol, inst.taker_fee, inst.maker_fee);
}

// HIP-3 instruments (derived from live data, includes mark price + OI)
let hip3_instruments = client.hyperliquid.hip3.instruments.list().await?;
for inst in &hip3_instruments {
    println!(
        "{} (namespace {:?}, ticker {:?}): mark={:?}",
        inst.coin, inst.namespace, inst.ticker, inst.mark_price,
    );
}
```

### Breadth Above Session VWAP (Hyperliquid and HIP-3)

Breadth is the percentage of eligible instruments trading above their
current UTC-session VWAP. The session resets at 00:00 UTC, uses the close of
the most recently completed one-minute candle, and excludes instruments with
no session volume or a price older than five minutes. History begins on
**2026-08-28** for HIP-3 and on **2026-08-24** for Hyperliquid core. Core snapshots
are aggregate only, so their `namespaces` maps are always empty. `value_pct`
is unavailable (`None`) when no instrument is eligible; do not render it as
0%, and do not average percentages across snapshots because the eligible
denominator varies.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::breadth::BreadthHistoryParams;
use oxarchive::types::OiFundingInterval;

let current = client.hyperliquid.hip3.breadth.current().await?;
println!("HIP-3 above session VWAP: {:?}%", current.value_pct);

let core = client.hyperliquid.breadth.current().await?;
println!("Core perps above session VWAP: {:?}%", core.value_pct);

// The last 24 hours at five-minute steps
let end = Utc::now();
let history = client.hyperliquid.hip3.breadth.history(BreadthHistoryParams {
    start: Some((end - Duration::days(1)).into()),
    end: Some(end.into()),
    interval: Some(OiFundingInterval::FiveMinutes),
    cursor: None,
    limit: Some(1000),
}).await?;
```

### HIP-3 Oracle

The latest deployer-pushed external reference price of a HIP-3 market, and its
instantaneous discovery bounds. The bounds apply `bound_fraction`, derived from
the market's maximum leverage, on each side of the reference price, which is
the external price when there is one and the mark price otherwise. The full
ratcheted range can be wider when deployer-specific reset configuration
applies. Keep the builder prefix and case in the symbol. `timestamp` is an
RFC 3339 UTC string and `timestamp_ms` the same instant in Unix milliseconds.

```rust
let price = client.hyperliquid.hip3.oracle.external_price("xyz:XYZ100").await?;
println!("external {:?}, mark {:?} at block {} ({})", price.external_price, price.mark_price, price.block_number, price.timestamp);

let bounds = client.hyperliquid.hip3.oracle.discovery_bounds("xyz:XYZ100").await?;
println!(
    "{} to {} around {} ({}), max leverage {}",
    bounds.lower_bound, bounds.upper_bound, bounds.reference_price,
    bounds.reference_source, bounds.max_leverage,
);
```

### Funding Rates

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::funding::FundingHistoryParams;
use oxarchive::types::OiFundingInterval;

// Get current funding rate
let current = client.hyperliquid.funding.current("BTC").await?;
println!("Funding rate: {}", current.funding_rate);

// Get the last 24 hours at an aggregation interval
let end = Utc::now();
let history = client.hyperliquid.funding.history("ETH", FundingHistoryParams {
    start: (end - Duration::days(1)).into(),
    end: end.into(),
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
use chrono::{Duration, Utc};
use oxarchive::resources::open_interest::OpenInterestHistoryParams;
use oxarchive::types::OiFundingInterval;

// Get current open interest
let current = client.hyperliquid.open_interest.current("BTC").await?;
println!("Open interest: {}", current.open_interest);

// Get the last 24 hours with aggregation
let end = Utc::now();
let history = client.hyperliquid.open_interest.history("BTC", OpenInterestHistoryParams {
    start: (end - Duration::days(1)).into(),
    end: end.into(),
    cursor: None,
    limit: None,
    interval: Some(OiFundingInterval::OneHour),
}).await?;
```

### Liquidations (Hyperliquid and HIP-3)

Historical liquidation events from 2025-12-22. Available on `client.hyperliquid.liquidations` and `client.hyperliquid.hip3.liquidations`. Liquidations by user (`by_user()`) are Hyperliquid core only.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::liquidations::*;

// The last seven days
let end = Utc::now();
let start = end - Duration::days(7);

// Get liquidation history
let liquidations = client.hyperliquid.liquidations.history("BTC", LiquidationHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: None,
}).await?;

// Get liquidations for a specific user
let user_liq = client.hyperliquid.liquidations.by_user("0x1234...", LiquidationsByUserParams {
    start: start.into(),
    end: end.into(),
    coin: Some("BTC".to_string()),
    cursor: None,
    limit: None,
}).await?;

// Get pre-aggregated liquidation volume (100-1000x less data)
let volume = client.hyperliquid.liquidations.volume("BTC", LiquidationVolumeParams {
    start: start.into(),
    end: end.into(),
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
for bucket in &volume.data {
    println!("total=${}, long=${}, short=${}", bucket.total_usd, bucket.long_usd, bucket.short_usd);
}

// HIP-3 liquidations (same API, different exchange prefix)
let hip3_liqs = client.hyperliquid.hip3.liquidations.history("xyz:XYZ100", LiquidationHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: None,
}).await?;

let hip3_vol = client.hyperliquid.hip3.liquidations.volume("xyz:XYZ100", LiquidationVolumeParams {
    start: start.into(),
    end: end.into(),
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
```

Projected forced-liquidation price levels refresh approximately every five
minutes. This is an observed cadence, not an exact five-minute guarantee.

### Lighter Liquidations

Liquidation trades and liquidation volume on both Lighter deployments, through
`client.lighter.liquidations` and `client.rh_lighter.liquidations`. Mainnet
history starts on 2026-06-10 and Robinhood Chain history at the venue launch,
2026-06-26 20:10:26 UTC; a `start` before that returns an error.

Lighter rows have their own shape, `LighterLiquidation`: both accounts of the
trade (`ask_account`, `bid_account`, as account indices), each side's
position before the trade, `usd_amount`, `tx_hash`, and `source`, which says
where the row came from. On Robinhood Chain, rows from before live capture
were backfilled from the venue's finalized export and have `source`
`"bucket"` and an empty `raw_json`; rows captured live have `source` `"ws"`
and the venue's raw JSON in `raw_json`. Volume buckets
(`LighterLiquidationVolume`) carry `total_usd` and `count`, with no
long/short split. `timestamp` is an RFC 3339 UTC string on both, with
`timestamp_ms` the same instant in Unix milliseconds.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::liquidations::{LiquidationHistoryParams, LiquidationVolumeParams};

// The last seven days
let end = Utc::now();
let start = end - Duration::days(7);

let liquidations = client.rh_lighter.liquidations.history("BTC", LiquidationHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
for liq in &liquidations.data {
    println!("{} {} @ {} ({:?})", liq.symbol, liq.size, liq.price, liq.source);
}

let volume = client.lighter.liquidations.volume("BTC", LiquidationVolumeParams {
    start: start.into(),
    end: end.into(),
    interval: Some("1h".to_string()),
    cursor: None,
    limit: None,
}).await?;
for bucket in &volume.data {
    println!("{}: {} liquidations, ${}", bucket.timestamp, bucket.count, bucket.total_usd);
}
```

### Candles (OHLCV)

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::CandleInterval;

// Hourly candles for the last 24 hours
let end = Utc::now();
let start = end - Duration::days(1);
let candles = client.hyperliquid.candles.history("BTC", CandleHistoryParams {
    start: start.into(),
    end: end.into(),
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
page. HIP-4 and Spot accept at most 1,000. Candle history is served from
**2025-03-22 12:00 UTC** on Hyperliquid, **2025-12-22** on HIP-3,
**2026-05-02 08:00 UTC** on HIP-4, **2025-03-22 10:50 UTC** on Spot,
**2025-08-01** on Lighter, and **2026-06-26 20:10 UTC** on Lighter on
Robinhood Chain.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::CandleInterval;

let end = Utc::now();
let lighter_candles = client.lighter.candles.history("BTC", CandleHistoryParams {
    start: (end - Duration::days(1)).into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneHour),
}).await?;
```

### Orders (Hyperliquid and HIP-3)

Order history, order flow aggregation, and TP/SL order queries. Available on `client.hyperliquid.orders` and `client.hyperliquid.hip3.orders`. `triggered: Some(true)` on order history keeps only orders whose trigger fired (status `triggered`) and `Some(false)` leaves them out; HIP-4 takes the same filter on `Hip4OrderHistoryParams`.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::orders::*;

// Order history is served from 2026-03-11 01:03 UTC; read the last ten minutes
let end = Utc::now();
let start = end - Duration::minutes(10);
let orders = client.hyperliquid.orders.history("BTC", OrderHistoryParams {
    start: Some(start.into()),
    end: Some(end.into()),
    limit: Some(1000),
    ..Default::default()
}).await?;

// Filter by user address
let user_orders = client.hyperliquid.orders.history("BTC", OrderHistoryParams {
    start: Some(start.into()),
    end: Some(end.into()),
    user: Some("0x1234...".to_string()),
    status: Some("filled".to_string()),
    ..Default::default()
}).await?;

// Only orders whose trigger fired
let triggered = client.hyperliquid.orders.history("BTC", OrderHistoryParams {
    start: Some(start.into()),
    end: Some(end.into()),
    triggered: Some(true),
    ..Default::default()
}).await?;

// Get aggregated order flow over the last 24 hours at 1m.
// A page holds up to `limit` buckets (default 1000, max 10000); follow
// next_cursor with the same start, end and interval while has_more is true.
let day_ago = end - Duration::days(1);
let flow = client.hyperliquid.orders.flow("BTC", OrderFlowParams {
    start: Some(day_ago.into()),
    end: Some(end.into()),
    interval: Some("1m".to_string()), // 1m (default), 5m, 15m, 1h
    cursor: None,
    limit: None,
}).await?;
let mut flow_buckets = flow.data;
let mut has_more = flow.has_more;
let mut cursor = flow.next_cursor;
while has_more {
    let page = client.hyperliquid.orders.flow("BTC", OrderFlowParams {
        start: Some(day_ago.into()),
        end: Some(end.into()),
        interval: Some("1m".to_string()),
        cursor: cursor.take(),
        limit: None,
    }).await?;
    flow_buckets.extend(page.data);
    has_more = page.has_more;
    cursor = page.next_cursor;
}

// Get TP/SL (take-profit / stop-loss) orders
let tpsl = client.hyperliquid.orders.tpsl("BTC", TpslParams {
    start: Some(day_ago.into()),
    end: Some(end.into()),
    user: None,
    triggered: Some(true),
    cursor: None,
    limit: None,
}).await?;

// HIP-3 orders
let hip3_orders = client.hyperliquid.hip3.orders.history("xyz:XYZ100", OrderHistoryParams {
    start: Some(start.into()),
    end: Some(end.into()),
    ..Default::default()
}).await?;
```

### L4 Orderbook (Hyperliquid and HIP-3)

Node-level L4 orderbook data with user attribution. Available on `client.hyperliquid.l4_orderbook` and `client.hyperliquid.hip3.l4_orderbook`. On a current or point-in-time snapshot each resting order carries its queue time: `timestamp` (RFC 3339 UTC) and `timestamp_ms`, both `None` when the queue time is unknown.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::l4_orderbook::*;

// L4 is served from 2026-03-11 01:03 UTC; read the last ten minutes
let end = Utc::now();
let start = end - Duration::minutes(10);

// Get current L4 orderbook snapshot
let l4 = client.hyperliquid.l4_orderbook.get("BTC", None).await?;

// Get the book as of a timestamp, 20 orders per side
let l4 = client.hyperliquid.l4_orderbook.get("BTC", Some(L4OrderBookParams {
    timestamp: Some(start.into()),
    depth: Some(20),
})).await?;

// Get paginated L4 orderbook diffs
let diffs = client.hyperliquid.l4_orderbook.diffs("BTC", L4DiffsParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
}).await?;

// Get paginated L4 checkpoint history (whole snapshots; keep `limit` small)
let history = client.hyperliquid.l4_orderbook.history("BTC", L4HistoryParams {
    start: (end - Duration::hours(1)).into(),
    end: end.into(),
    cursor: None,
    limit: Some(2),
}).await?;

// HIP-3 L4 orderbook
let hip3_l4 = client.hyperliquid.hip3.l4_orderbook.get("xyz:XYZ100", None).await?;
let hip3_diffs = client.hyperliquid.hip3.l4_orderbook.diffs("xyz:XYZ100", L4DiffsParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: None,
}).await?;
```

### L2 Orderbook (Hyperliquid and HIP-3)

Full-depth L2 orderbook derived from L4 data. Available on `client.hyperliquid.l2_orderbook` and `client.hyperliquid.hip3.l2_orderbook`.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::l2_orderbook::*;

// Full-depth L2 is served from 2026-03-11 01:03 UTC; read the last hour
let end = Utc::now();
let start = end - Duration::hours(1);

// Get current L2 full-depth orderbook
let l2 = client.hyperliquid.l2_orderbook.get("BTC", None).await?;

// Get L2 orderbook at a specific timestamp
let l2 = client.hyperliquid.l2_orderbook.get("BTC", Some(L2OrderBookParams {
    timestamp: Some(start.into()),
    depth: Some(50),
})).await?;

// Get L2 orderbook history, 50 levels per side
let l2_history = client.hyperliquid.l2_orderbook.history("BTC", L2HistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(100),
    depth: Some(50),
}).await?;

// Get L2 tick-level diffs from the last ten minutes
let l2_diffs = client.hyperliquid.l2_orderbook.diffs("BTC", L2DiffsParams {
    start: (end - Duration::minutes(10)).into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
}).await?;

// HIP-3 L2 orderbook
let hip3_l2 = client.hyperliquid.hip3.l2_orderbook.get("xyz:XYZ100", None).await?;
```

### L3 Orderbook (Lighter only)

Order-level L3 orderbook data showing individual orders. Available on `client.lighter.l3_orderbook`.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::l3_orderbook::{L3HistoryParams, L3OrderBookParams};

// L3 is served from 2026-03-05 03:33 UTC; read the last hour
let end = Utc::now();
let start = end - Duration::hours(1);

// Get current L3 orderbook
let l3 = client.lighter.l3_orderbook.get("BTC", None).await?;

// Get with depth limit
let l3 = client.lighter.l3_orderbook.get("BTC", Some(20)).await?;

// A point-in-time snapshot, filtered to one account's resting orders
let mine = client.lighter.l3_orderbook.get_with_params("BTC", L3OrderBookParams {
    timestamp: Some(start.into()),
    account: Some(726714),
    depth: None,
}).await?;

// Get paginated L3 orderbook history (optionally one account's orders)
let history = client.lighter.l3_orderbook.history("BTC", L3HistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
    account: None,
}).await?;
```

### HIP-4 Outcome Markets (Hyperliquid)

Binary outcome perps deployed under the Hyperliquid namespace. Responses use
`#<10*outcome_id + side>` symbols (`#0`, `#1`, `#55850`, ...). For path
inputs, use the bare numeric form (`"0"`, `"1"`, `"55850"`). Legacy `"#0"`
inputs remain supported and are percent-encoded for transport; do not
pre-encode them yourself.

HIP-4 trades and candles are served from **2026-05-02 08:00 UTC**, and the
order book and outcome-side OI from **2026-05-02 16:51 UTC**, with raw OI
updates at ~10s. HIP-4 has **no funding rates and no liquidations**.
`mark_price` on HIP-4 is an implied probability in `[0, 1]`, not a USD price.

Each side stops trading when its outcome settles, and new outcomes are listed
every day, so pick a side that is trading now. The example below takes the
Yes side of the open daily BTC outcome from the instrument list.

```rust
use chrono::{Duration, Utc};
use oxarchive::exchanges::{Hip4HistoryRange, Hip4ListOutcomesParams,
    Hip4ListQuestionsParams, Hip4OrderBookHistoryParams, Hip4OrderBookParams, Hip4TradesParams};
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::CandleInterval;

// Per-side instruments (`#0`, `#1`, ...); list_instruments() is the same call.
let insts = client.hyperliquid.hip4.get_instruments().await?;
let open = insts
    .iter()
    .find(|i| {
        i.is_settled == Some(false)
            && i.recurring_underlying.as_deref() == Some("BTC")
            && i.side == 0
    })
    .expect("an open BTC outcome");
// Path inputs use the bare numeric form: "#77440" becomes "77440".
let symbol = open.coin.trim_start_matches('#').to_string();
let inst = client.hyperliquid.hip4.get_instrument(&symbol).await?;
println!("{}: {:?}", inst.coin, inst.display_title);

// Outcomes (per-outcome view, both sides combined)
let outcomes = client.hyperliquid.hip4.list_outcomes(None).await?;
for o in &outcomes.data {
    println!("{} . {:?}", o.outcome_id, o.display_title);
}

// Filter by settlement state OR by slug
let unsettled = client.hyperliquid.hip4.list_outcomes(Some(Hip4ListOutcomesParams {
    is_settled: Some(false),
    slug: None,
    cursor: None,
    limit: Some(50),
})).await?;

if let Some(slug) = &open.slug {
    let one = client.hyperliquid.hip4.get_outcome_by_slug(slug).await?;
    println!("aggregated_oi: {:?}", one.aggregated_oi);
}

// Outcome detail (with aggregated_oi)
let detail = client.hyperliquid.hip4.get_outcome(open.outcome_id).await?;

// Questions: a question groups binary outcomes, one named outcome per choice
// plus a fallback outcome that resolves Yes when no named choice does.
let questions = client.hyperliquid.hip4.list_questions(Some(Hip4ListQuestionsParams {
    cursor: None,
    limit: Some(100),
})).await?;
for q in &questions.data {
    println!("{}: {} named outcomes, fallback {}", q.question_id, q.named_outcome_ids.len(), q.fallback_outcome_id);
}
// Page with questions.next_cursor until it is None.
let question = client.hyperliquid.hip4.get_question(1).await?;

// The last six hours
let end = Utc::now();
let start = end - Duration::hours(6);

// L2 orderbook: now, and as of a timestamp
let ob = client.hyperliquid.hip4.get_orderbook(&symbol, None).await?;
let ob_at = client.hyperliquid.hip4.get_orderbook(&symbol, Some(Hip4OrderBookParams {
    timestamp: Some(start.into()),
    depth: Some(20),
})).await?;
// History takes a Hip4HistoryRange, or Hip4OrderBookHistoryParams to set depth
let ob_history = client.hyperliquid.hip4.get_orderbook_history(&symbol, Hip4OrderBookHistoryParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(100),
    depth: Some(5),
}).await?;

// Trades (history + recent); get_trades_history() is the same call
let trades = client.hyperliquid.hip4.get_trades(&symbol, Hip4TradesParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;
let recent = client.hyperliquid.hip4.get_trades_recent(&symbol, Some(50)).await?;

// Implied-probability OHLCV candles
let candles = client.hyperliquid.hip4.candles.history(&symbol, CandleHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneHour),
}).await?;

// Open interest (per-side history + latest); get_open_interest_history() is the same call
let oi_hist = client.hyperliquid.hip4.get_open_interest(&symbol, Hip4HistoryRange {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: None,
}).await?;
let oi_now = client.hyperliquid.hip4.get_open_interest_current(&symbol).await?;
// mark_price on HIP-4 is an implied probability in [0, 1].

// Summary, freshness, prices (get_price_history() is the same call as get_prices())
let summary    = client.hyperliquid.hip4.get_summary(&symbol).await?;
let freshness  = client.hyperliquid.hip4.get_freshness(&symbol).await?;
let prices     = client.hyperliquid.hip4.get_prices(&symbol,
    start, end, Some("1h"), Some(100), None).await?;

// L4 (current snapshot, diffs, checkpoint history)
let l4_now     = client.hyperliquid.hip4.get_l4_orderbook(&symbol, None).await?;
let l4_diffs   = client.hyperliquid.hip4.get_l4_diffs(&symbol, Hip4HistoryRange {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
```

### Hyperliquid Spot

Spot trading pairs deployed under the Hyperliquid namespace. Symbols use the
dashed canonical form (`HYPE-USDC`, `PURR-USDC`, ...). The server resolves
the dashed form to the wire format (`PURR/USDC`, `@107`) internally.

Spot has **no funding, no open interest, and no liquidations**. Trade history
is served from **2025-03-22T10:50:22Z**. Spot candle history starts exactly at
**2025-03-22T10:50:00Z**, supports `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`,
and `1w`, and accepts at most 1,000 rows per page. The order book, L4, and TWAP have
no history before live capture began: TWAP from 2026-05-05 13:05 UTC, the
order book from 2026-05-05 19:56 UTC, and L4 and order history from
2026-05-05 22:57 UTC.

`SpotPair` carries the pair's `symbol`, `base` and `quote` tokens, the
Hyperliquid wire-format pair (`wire_symbol`, such as `PURR/USDC` or `@107`),
and its index in the spot universe (`spot_index`). The remaining registry
fields, such as `is_canonical` and the token decimals, are in `extra`.

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::orderbook::{GetOrderBookParams, OrderBookHistoryParams};
use oxarchive::resources::l4_orderbook::{L4DiffsParams, L4HistoryParams, L4OrderBookParams};
use oxarchive::resources::orders::SpotOrderHistoryParams;
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::spot::SpotTwapParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::types::CandleInterval;

// Pair discovery (dashed canonical: HYPE-USDC, PURR-USDC, ...).
let pairs = client.hyperliquid.spot.pairs.list().await?;
let hype = client.hyperliquid.spot.pairs.get("HYPE-USDC").await?;
println!("{:?}/{:?} trades as {:?}", hype.base, hype.quote, hype.wire_symbol);

// Current L2 orderbook.
let ob = client.hyperliquid.spot.orderbook.get("HYPE-USDC", None).await?;
println!("HYPE-USDC mid: {:?}", ob.mid_price);

// The last hour
let end = Utc::now();
let start = end - Duration::hours(1);

// Historical L2 orderbook.
let ob_history = client.hyperliquid.spot.orderbook.history("HYPE-USDC", OrderBookHistoryParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
    depth: None,
    granularity: None,
}).await?;

// One-minute candles (max 1,000 rows per page).
let candles = client.hyperliquid.spot.candles.history("HYPE-USDC", CandleHistoryParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
    interval: Some(CandleInterval::OneMinute),
}).await?;

// Trades.
let trades = client.hyperliquid.spot.trades.history("PURR-USDC", GetTradesParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;

// L4 reconstruction.
let l4_now = client.hyperliquid.spot.l4_orderbook.get("HYPE-USDC", None).await?;
let l4_diffs = client.hyperliquid.spot.l4_orderbook.diffs("HYPE-USDC", L4DiffsParams {
    start: (end - Duration::minutes(10)).into(),
    end:   end.into(),
    cursor: None,
    limit: Some(1000),
}).await?;
let l4_history = client.hyperliquid.spot.l4_orderbook.history("HYPE-USDC", L4HistoryParams {
    start: start.into(),
    end:   end.into(),
    cursor: None,
    limit: Some(10),
}).await?;

// Order lifecycle history (Spot serves history only: no flow, TP/SL, or trigger levels).
let orders = client.hyperliquid.spot.orders.history("HYPE-USDC", SpotOrderHistoryParams {
    start: Some((end - Duration::minutes(10)).into()),
    end: Some(end.into()),
    ..Default::default()
}).await?;

// TWAP statuses by symbol or by user.
let twap_sym = client.hyperliquid.spot.twap
    .by_symbol("HYPE-USDC", SpotTwapParams::default()).await?;
let twap_user = client.hyperliquid.spot.twap
    .by_user("0x1234...", SpotTwapParams::default()).await?;

// Per-table freshness.
let freshness = client.hyperliquid.spot.freshness("HYPE-USDC").await?;
```

### Lighter on Robinhood Chain

Robinhood Chain is the second Lighter deployment, served under
`/v1/rh-lighter` through `client.rh_lighter`. It has the same resources as the
mainnet `client.lighter` except the L3 order book, which is not captured on
this deployment.

- **Markets:** quoted in USDG: perps with uppercase symbols (`BTC`, `ETH`) and
  spot pairs with dashed symbols (`AAPL-USDG`). List them with
  `client.rh_lighter.instruments.list()`. Market ids and symbols are separate
  from mainnet, so query each deployment through its own client.
- **Coverage:** candles from 2026-06-26 20:10 UTC; trades and liquidations
  from 2026-06-26 20:10:26 UTC (the venue launch); order book, open interest,
  funding, and price history from 2026-08-22 18:43 UTC.
- **Trades:** as on mainnet, `trades.list()` returns final trades up to the
  finalization boundary (about a day behind), and `trades.recent()` serves
  the preliminary tier. `trades.list_with_meta()` and
  `trades.recent_with_meta()` also return the boundary and the clamp; see
  [Trades](#trades).

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::types::CandleInterval;

let instruments = client.rh_lighter.instruments.list().await?;
let book = client.rh_lighter.orderbook.get("BTC", None).await?;
let funding = client.rh_lighter.funding.current("BTC").await?;
let oi = client.rh_lighter.open_interest.current("BTC").await?;

// Final trades from the last seven days (the newest day is not final yet)
let end = Utc::now();
let start = end - Duration::days(7);
let trades = client.rh_lighter.trades.list("AAPL-USDG", GetTradesParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: Some(1000),
    side: None,
}).await?;
let candles = client.rh_lighter.candles.history("BTC", CandleHistoryParams {
    start: start.into(),
    end: end.into(),
    cursor: None,
    limit: None,
    interval: Some(CandleInterval::OneHour),
}).await?;
let summary = client.rh_lighter.summary("BTC").await?;
let freshness = client.rh_lighter.freshness("BTC").await?;
```

### Account Positions

Perp positions per wallet or account on Hyperliquid core
(`client.hyperliquid.positions`), HIP-3 (`client.hyperliquid.hip3.positions`),
Lighter mainnet (`client.lighter.positions`), and Lighter on Robinhood Chain
(`client.rh_lighter.positions`). Hyperliquid and HIP-3 are keyed by `0x`
wallet address; both Lighter deployments are keyed by integer account index.
Spot and HIP-4 positions are not included.

| Venue | Change log and as-of reads | Hourly snapshots | Live snapshot |
|-------|---------------------------|------------------|---------------|
| Hyperliquid core | From 2025-05-25 | From 2026-06-07 | Every 5 minutes |
| HIP-3 | From 2025-10-13 | From 2026-06-07 (some dexes start later) | Every 5 minutes |
| Lighter mainnet | From 2025-01-17 | From 2025-01-17 | Every 2 minutes |
| Lighter on Robinhood Chain | From 2026-06-26 | From 2026-06-26 | Every 2 minutes |

| Method | Returns |
|--------|---------|
| `get(key, params)` | Open positions now, or as of `params.timestamp`, plus the account summary when there is one (see `WalletPositions::account`) and `account_seen` |
| `history(key, params)` | Hourly position rows in `[start, end)` |
| `changes(key, params)` | Change-log rows (one per fill leg, with the position before and after) in `[start, end)` |
| `market(symbol, params)` | Every open position in one market, largest value first, now or at `hour`; totals in `meta.totals` |
| `market_summary(symbol, params)` | Long/short counts, sizes, values, average entries and top-10 share, now or as an hourly series |
| `all(params)` | Every open position across markets at one committed hour |
| `account(key, ...)` | Current account summary: the clearinghouse summary on Hyperliquid and HIP-3 (`account(address, dex)`), position aggregates on Lighter (`account(account_index)`) |
| `account_history(key, params)` | Hourly account summaries (on Lighter, one row per hour, at most 744 per page) |
| `client.lighter.accounts.by_l1(l1_address, cursor, limit)` | Lighter account indices owned by an L1 address (mainnet only) |

```rust
use chrono::{Duration, Utc};
use oxarchive::resources::positions::{
    AccountHistoryParams, GetPositionsParams, MarketPositionsParams, MarketSummaryParams,
    PositionRangeParams,
};

let wallet = "0x0000000000000000000000000000000000000001";
let end = Utc::now();
let day_ago = end - Duration::days(1);

// Current positions, from the latest live snapshot
let now = client.hyperliquid.positions.get(wallet, None).await?;
for p in &now.data.positions {
    println!("{} {} {} entry={:?} uPnL={:?}", p.symbol, p.side, p.size, p.entry_price, p.unrealized_pnl);
}
if now.meta.stale == Some(true) {
    eprintln!("{}", now.meta.notice.unwrap_or_default());
}

// As of an instant: the state after every event before it
let then = client.hyperliquid.positions.get(wallet, Some(GetPositionsParams {
    timestamp: Some(day_ago.into()),
    ..Default::default()
})).await?;
println!("source={:?} quality={:?}", then.meta.source, then.meta.quality);

// Change log for one market over the last day, paged with next_cursor
let mut params = PositionRangeParams {
    symbol: Some("BTC".to_string()),
    ..PositionRangeParams::new(day_ago, end)
};
loop {
    let page = client.hyperliquid.positions.changes(wallet, params.clone()).await?;
    for c in &page.data {
        println!("{} {} {:?} -> {:?}", c.timestamp, c.event_type, c.start_position, c.end_position);
    }
    match page.next_cursor {
        Some(cursor) => params.cursor = Some(cursor),
        None => break,
    }
}

// HIP-3: filter by dex
let hip3 = client.hyperliquid.hip3.positions.get(wallet, Some(GetPositionsParams {
    dex: Some("xyz".to_string()),
    ..Default::default()
})).await?;

// Every long position in a market worth at least $100k, with totals
let market = client.hyperliquid.positions.market("BTC", Some(MarketPositionsParams {
    side: Some("long".to_string()),
    min_value: Some(100_000.0),
    ..Default::default()
})).await?;
if let Some(totals) = market.meta.position_totals() {
    println!("{} longs, top-10 share {:?}", totals.long_count, totals.top10_value_share);
}

// Lighter accounts are keyed by account index
let accounts = client.lighter.accounts.by_l1(wallet, None, None).await?;
for a in &accounts.data.accounts {
    let index: u64 = a.account_index.parse().unwrap();
    let positions = client.lighter.positions.get(index, None).await?;
    println!("account {index}: {} positions", positions.data.positions.len());
}
let rh_summary = client.rh_lighter.positions
    .market_summary("BTC", Some(MarketSummaryParams::default())).await?;

// Lighter account summary (position aggregates): now, and one row per hour
let lighter_now = client.lighter.positions.account(4521).await?;
let rh_hourly = client.rh_lighter.positions
    .account_history(4521, AccountHistoryParams::new(day_ago, end)).await?;
```

Semantics:

- **As of `T`:** the state after every event before `T`. When `T` is an exact
  UTC hour with a committed hourly snapshot, you get that snapshot with every
  field. Any other instant is reconstructed (`meta.source` is
  `reconstructed`): size, entry price and `opened_at` are exact, mark fields
  are at `T`, and the snapshot-only fields are not stated. On Hyperliquid
  and HIP-3 that means `leverage.kind` (`type` on the wire) is `unknown`, and
  `leverage.value`, the `cum_funding` members, `margin_used`,
  `return_on_equity`, and `liquidation_price` are `None`, with
  `liquidation_price_status` `unavailable`. Lighter rows keep the margin mode
  in `leverage.kind`. `data.account` is `None` on a reconstructed response.
- **Boundaries:** `meta.built_through` is how far the change log is built;
  as-of reads and change-log ranges are clamped to it, with the original value
  in `meta.requested_end` and the clamp in `meta.clamped_to`.
  `meta.finalized_through` is how far the data is final, the same meaning it
  has on Lighter trades. A time before coverage returns an empty list with
  `meta.notice` and `meta.coverage_from`.
- **Quality:** every row carries its own `quality` (`complete`, `partial`,
  `degraded`; Lighter adds `preliminary`, `unreconciled`, and `incomplete`),
  and `meta.quality` describes the snapshot. `meta.stale` is `true` when the
  latest live snapshot is more than 12 minutes old.
- **Empty wallets:** when there are no open positions, `data.account_seen` is
  `flat`, `never_seen` (no recorded activity in the covered history), or
  `outside_coverage`.
- **Market routes** serve committed snapshots only (`hour` must be an exact
  UTC hour and is echoed as `meta.snapshot_ts`). Their cursors pin the
  snapshot; if it has been replaced or has expired, the server answers HTTP
  409 and you restart without a cursor. On Lighter, the insurance, settlement,
  and system accounts are excluded unless `include_system` is set, and every
  row carries `account_kind`.
- **Numbers** are decimal strings; a flat position is `"0"`.
- **Instants** are RFC 3339 UTC strings. Instants in `meta` always carry
  milliseconds (`2026-09-25T00:00:00.000Z`); instants in rows carry a
  fraction only when it is not zero (`2026-09-25T00:00:00Z`). Parse them, for
  example with `chrono::DateTime::parse_from_rfc3339`, before comparing a row
  to `meta`.

Limits and billing: wallet and account routes return up to 5,000 rows per
page (default 500), market routes up to 2,000 (default 100), summary series
up to 168 hours per page, and `all()` up to 2,000 rows per page. Position
routes cost one credit per 1,000 rows returned, with a minimum of one credit
per request, the same rate as trades. `account()`, `account_history()`, and
`accounts.by_l1()` cost one credit per request.

### Wallet Classification (Hyperliquid and HIP-3)

Precomputed daily behavior metrics for active wallets: order counts, cancel
and fill rates, maker share, order sizes, volume, fees, realized PnL, and TWAP,
priority gas, and builder usage. Filter, sort, and page with `offset` until it
reaches `total`. The snapshot date defaults to yesterday (UTC). Available on
`client.hyperliquid.wallets` and `client.hyperliquid.hip3.wallets`.

```rust
use chrono::{Duration, Utc};
use oxarchive::WalletClassifyParams;

// Two days ago (the default, yesterday, is the latest snapshot)
let date = Utc::now().date_naive() - Duration::days(2);
let page = client.hyperliquid.wallets.classify(WalletClassifyParams {
    min_orders: Some(1000),
    sort: Some("total_volume_usd".to_string()),
    order: Some("desc".to_string()),
    limit: Some(100),
    uses_twap: Some(true),
    date: Some(date),
    ..Default::default()
}).await?;
println!("{} wallets match on {}", page.total, page.date);
for wallet in &page.wallets {
    println!("{}: cancel rate {:?}, maker ratio {:?}", wallet.address, wallet.metrics.cancel_rate, wallet.metrics.maker_ratio);
}

let hip3 = client.hyperliquid.hip3.wallets.classify(WalletClassifyParams::default()).await?;
```

Every metric is an `Option`, and metrics added to the API later are kept in
`metrics.extra`.

### Freshness

Check when each data type was last updated for a specific coin.

```rust
let freshness = client.hyperliquid.freshness("BTC").await?;
let lighter_freshness = client.lighter.freshness("BTC").await?;
let rh_freshness = client.rh_lighter.freshness("BTC").await?;
let hip3_freshness = client.hyperliquid.hip3.freshness("xyz:XYZ100").await?;
let spot_freshness = client.hyperliquid.spot.freshness("HYPE-USDC").await?;
```

### Summary

Combined market snapshot: mark/oracle price, funding rate, open interest, 24h volume, and liquidation volumes.

```rust
let summary = client.hyperliquid.summary("BTC").await?;
let lighter_summary = client.lighter.summary("BTC").await?;
let rh_summary = client.rh_lighter.summary("BTC").await?;
let hip3_summary = client.hyperliquid.hip3.summary("xyz:XYZ100").await?;
```

### Price History

Mark, oracle, and mid price history. Supports aggregation intervals.

```rust
// The last 24 hours, in Unix milliseconds
let end = chrono::Utc::now().timestamp_millis();
let start = end - 24 * 60 * 60 * 1000;
let prices = client.hyperliquid.price_history(
    "BTC",
    start,
    end,
    Some("1h"),         // interval
    Some(100),          // limit
    None,               // cursor
).await?;
```

### Cumulative Volume Delta

Taker buy and sell notional per bucket, their difference (`delta`), and a
running total (`cumulative_delta`), for Hyperliquid core and HIP-3 symbols.
Each bucket's `timestamp` is its open time as an RFC 3339 UTC string, with
`timestamp_ms` in Unix milliseconds.
Intervals are `1m`, `5m`, `15m`, `30m`, `1h` (the default), `4h`, `1d`, and
`1w`. Buckets are labelled by their open time in UTC and omitted when they
hold no trades; `4h`, `1d`, and `1w` buckets open on UTC epoch boundaries, so
`1w` buckets open on Thursdays.

A page holds up to `limit` buckets (default 500, max 10,000). Follow
`next_cursor` with the same `start`, `end`, and `interval` while `has_more`
is `true`; below `1h` a page can be short and still have more. `cumulative_delta`
restarts on every page, so rebuild it from `delta` when joining pages, and
`meta.notice` says when a response is one page of several. Without `start` or
`cursor`, the response is the newest `limit` buckets of the 24 hours before
`end`.

```rust
use chrono::{Duration, Utc};
use oxarchive::CvdParams;
use oxarchive::types::CandleInterval;

// The last 24 hours at five-minute buckets
let end = Utc::now();
let start = end - Duration::days(1);
let mut buckets = Vec::new();
let mut cursor = None;
loop {
    let page = client.hyperliquid.cvd("BTC", CvdParams {
        start: Some(start.into()),
        end: Some(end.into()),
        interval: Some(CandleInterval::FiveMinutes),
        cursor,
        limit: None,
    }).await?;
    buckets.extend(page.data);
    if !page.has_more {
        break;
    }
    cursor = page.next_cursor;
}
// Rebuild the running total across pages from `delta`.
let mut running = 0.0;
for bucket in &buckets {
    running += bucket.delta;
    println!("{} delta {:.2} running {:.2}", bucket.timestamp, bucket.delta, running);
}

// HIP-3 keeps the builder prefix and case; the last 24 hours at 1h.
let hip3 = client.hyperliquid.hip3.cvd("xyz:XYZ100", CvdParams::default()).await?;
```

## Data Quality Monitoring

Monitor data coverage, incidents, latency, and SLA compliance.

```rust
// System health status
let status = client.data_quality.status().await?;
println!("System: {}", status.status);

// Data coverage (status_coverage() is the public summary the status page shows)
let coverage = client.data_quality.coverage().await?;
let public_coverage = client.data_quality.status_coverage().await?;

// Symbol-specific coverage with gap detection
let btc = client.data_quality.symbol_coverage("hyperliquid", "BTC").await?;
// Symbols keep their case and are percent-encoded (`xyz:XYZ100`, `HYPE-USDC`, `#0`);
// choose the gap-detection window with symbol_coverage_with().
use oxarchive::resources::data_quality::{ListIncidentsParams, SymbolCoverageParams};
let now = chrono::Utc::now();
let xyz100 = client.data_quality.symbol_coverage_with("hip3", "xyz:XYZ100", SymbolCoverageParams {
    from: Some((now - chrono::Duration::days(7)).into()),
    to: Some(now.into()),
}).await?;

// Incidents
let incidents = client.data_quality.list_incidents(None).await?;
let lighter_incidents = client.data_quality.list_incidents_with(ListIncidentsParams {
    exchange: Some("lighter".to_string()),
    limit: Some(20),
    offset: Some(0),
    ..Default::default()
}).await?;
println!("{:?} lighter incidents", lighter_incidents.pagination.map(|p| p.total));
let incident = client.data_quality.get_incident("inc-123").await?;

// Latency and SLA
let latency = client.data_quality.latency().await?;
let sla = client.data_quality.sla(None, None).await?;

// Account positions freshness, one row per venue
for venue in client.data_quality.positions_freshness().await? {
    println!("{} {}: {:?}s old, stale={}", venue.venue, venue.product, venue.live_age_seconds, venue.stale);
}
```

## Webhooks

Signed HTTP notifications for market, account, archive, export, and billing
events. `client.webhooks` covers the management routes under `/v1/webhooks`,
and `oxarchive::webhook_signature` verifies the deliveries your receiver gets.
Management calls cost no credits; the estimate and the dry-run are metered like
the market data they return.

Webhook delivery starts on the Build plan. Free keeps the estimate and the
dry-run, so a rule can be designed and sized before there is anywhere to
deliver it.

| Plan | Endpoints | Subscriptions | Watched wallets | Deliveries per day |
|------|-----------|---------------|-----------------|--------------------|
| Free | Not available | Not available | Not available | Not available |
| Build | 1 | 8 | 2 | 5,000 |
| Pro | 4 | 40 | 15 | 50,000 |
| Scale | 12 | 200 | 50 | 500,000 |
| Enterprise | Custom | Custom | Custom | Custom |

`client.webhooks.limits()` returns your own plan's caps with what is in use
against each.

### Design and Size a Rule

The event catalog declares each event type's scope, venues, filters,
parameters, and the metrics a condition can be written against. The estimate
replays a config over up to 30 days and returns the daily rate, a ladder of
rates at other thresholds, and a sample of matches. The dry-run returns the
individual occurrences a config would have delivered over up to 24 hours
(`account.fill`, `account.transfer`, and `market.liquidation`). Estimates and
dry-runs share a budget of six a minute per account.

```rust
use oxarchive::{DryRunParams, EstimateParams};
use oxarchive::types::{WebhookSubscriptionCondition, WebhookSubscriptionConfig};

let catalog = client.webhooks.event_types().await?;
for event_type in catalog.iter().filter(|t| t.live) {
    println!("{} ({}): {}", event_type.event_type, event_type.scope, event_type.description);
}

let config = WebhookSubscriptionConfig::default()
    .venue("hyperliquid")
    .symbols(["BTC", "ETH"])
    .condition(WebhookSubscriptionCondition::new(
        "notional_usd",
        "greater_than_or_equal",
        250_000,
    ));

// How often would this have fired over the last week?
let estimate = client.webhooks.estimate(
    EstimateParams::new("market.liquidation").config(config.clone()).lookback_days(7),
).await?;
println!("median {} a day, busiest day {}", estimate.per_day_p50, estimate.per_day_max);
for rung in &estimate.ladder {
    println!("at {}: {:.1} a day", rung.value, rung.per_day);
}

// Which occurrences would it have delivered in the last six hours?
let dry_run = client.webhooks.dry_run(
    DryRunParams::new("market.liquidation").config(config.clone()).lookback_s(21_600).limit(5),
).await?;
println!("{} matched", dry_run.matched);
```

### Endpoints and Subscriptions

```rust
use oxarchive::{CreateEndpointParams, CreateSubscriptionParams, UpdateSubscriptionParams};

// The signing secret is returned here and on rotation, and nowhere else.
let endpoint = client.webhooks.create_endpoint(
    CreateEndpointParams::new("https://example.com/hooks/0xarchive").description("liquidation alerts"),
).await?;
let secret = endpoint.secret.clone(); // store it now

// The response carries the stored, normalized config.
let subscription = client.webhooks.create_subscription(
    CreateSubscriptionParams::new(&endpoint.id, "market.liquidation").filters(config),
).await?;

// Queue a signed webhook.test through the delivery path, then read the log.
let queued = client.webhooks.test_endpoint(&endpoint.id).await?;
let log = client.webhooks.list_deliveries(&endpoint.id, Some(10)).await?;
for delivery in &log {
    println!("{} {} after {} attempts ({:?})", delivery.event_type, delivery.state, delivery.attempts, delivery.last_status_code);
}

// Switch a rule off without deleting it, or replace its config.
client.webhooks.update_subscription(
    &subscription.id,
    UpdateSubscriptionParams::default().enabled(false),
).await?;

// Watched wallets: address scoped event types (`account.*`) report only on these,
// and a subscription's `addresses` must already be on the list.
let added = client.webhooks.add_address("0x0000000000000000000000000000000000000001", Some("treasury")).await?;
let watched = client.webhooks.list_addresses().await?;
println!("{} watched, plan allows {:?}", watched.addresses.len(), watched.limit);
```

| Method | Route |
|--------|-------|
| `event_types()` | `GET /v1/webhooks/event-types` |
| `limits()` | `GET /v1/webhooks/limits` |
| `list_endpoints()` | `GET /v1/webhooks/endpoints` |
| `create_endpoint(params)` | `POST /v1/webhooks/endpoints` |
| `delete_endpoint(id)` | `DELETE /v1/webhooks/endpoints/{id}` |
| `enable_endpoint(id)` | `POST /v1/webhooks/endpoints/{id}/enable` |
| `rotate_secret(id)` | `POST /v1/webhooks/endpoints/{id}/rotate` |
| `test_endpoint(id)` | `POST /v1/webhooks/endpoints/{id}/test` |
| `list_deliveries(id, limit)` | `GET /v1/webhooks/endpoints/{id}/deliveries` |
| `redeliver(delivery_id)` | `POST /v1/webhooks/deliveries/{id}/redeliver` |
| `list_subscriptions()` | `GET /v1/webhooks/subscriptions` |
| `create_subscription(params)` | `POST /v1/webhooks/subscriptions` |
| `update_subscription(id, params)` | `PATCH /v1/webhooks/subscriptions/{id}` |
| `delete_subscription(id)` | `DELETE /v1/webhooks/subscriptions/{id}` |
| `resume_subscription(id)` | `POST /v1/webhooks/subscriptions/{id}/resume` |
| `resume_all_subscriptions()` | `POST /v1/webhooks/subscriptions/resume` |
| `dry_run(params)` | `POST /v1/webhooks/subscriptions/dry-run` |
| `estimate(params)` | `POST /v1/webhooks/subscriptions/estimate` |
| `list_addresses()` | `GET /v1/webhooks/addresses` |
| `add_address(address, label)` | `POST /v1/webhooks/addresses` |
| `delete_address(id)` | `DELETE /v1/webhooks/addresses/{id}` |

A refusal, such as a plan without webhook delivery or a cap already reached,
is an `Error::Api` whose message gives the reason. `test_endpoint()`,
`redeliver()`, and the resume calls answer HTTP 409 when today's delivery
budget is already spent.

### Pauses and Resuming

When an account reaches its deliveries per day, the rule that crossed the
limit pauses and says so, rather than dropping events without a signal:
`status` is `auto_paused`, `pause_message` explains it in plain words, and
`pause_reason` is `deliveries_per_day_cap` or `plan_no_webhooks`. A paused rule
delivers and buffers nothing. A pause at the daily limit lasts until the rule
is resumed; the resume response describes the missed window, which can be
re-read from the REST routes.

```rust
let limits = client.webhooks.limits().await?;
if limits.paused_subscriptions.count > 0 {
    let resumed = client.webhooks.resume_all_subscriptions().await?;
    if let Some(gap) = resumed.gap {
        println!(
            "resumed {}; re-read {:?} to {:?}",
            resumed.resumed_count, gap.replay_window.start, gap.replay_window.end,
        );
    }
}
```

### Verifying Deliveries

Every delivery carries `0xa-signature: t=<unix seconds>,v1=<hex>`, where the
digest is HMAC-SHA256 over `<t>.<raw body>` keyed with the endpoint's whole
secret string, plus `0xa-event-id` and `0xa-event-type`. `WebhookVerifier`
checks it:

- **Raw bytes.** Pass the request body exactly as it arrived, before any JSON
  parsing. A re-serialized body has different bytes and fails.
- **Replay window.** `t` must be within 5 minutes of your clock by default;
  change it with `tolerance_secs()`.
- **Rotation.** For 24 hours after `rotate_secret()`, each delivery carries a
  `v1` for the new secret and one for the previous secret. The verifier
  accepts a delivery when any `v1` matches any secret it holds.
- **Constant time.** Signatures are compared in constant time.

```rust
use oxarchive::webhook_signature::{WebhookVerifier, EVENT_ID_HEADER, SIGNATURE_HEADER};

// Hold both secrets while rolling over after a rotation.
let verifier = WebhookVerifier::with_secrets([new_secret, previous_secret]);

// In your HTTP handler, with the raw body and the `0xa-signature` header value:
match verifier.verify(raw_body, signature_header) {
    Ok(_) => {
        let event: serde_json::Value = serde_json::from_slice(raw_body)?;
        // Deduplicate on the `0xa-event-id` header (the payload `id`), then
        // answer 2xx and do the work off the request.
    }
    Err(err) => {
        // Answer 4xx and log `err`.
    }
}
```

Answer with any 2xx within 10 seconds. Delivery is at least once: a non-2xx
response, a timeout, or a connection error is retried (5 seconds, 30 seconds,
2 minutes, 10 minutes, 1 hour, then hourly) for up to 24 hours, and retries and
`redeliver()` keep the event id. An endpoint with 10 or more consecutive
failures sustained for 6 hours or more is disabled automatically; bring it
back with `enable_endpoint()`.

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
- **Live subscriptions**: supported Hyperliquid and Lighter live market channels, on both Lighter deployments
- **Replay**: bounded historical data with timing preserved

For large historical downloads, use the S3 Parquet bulk export in the [Data Catalog](https://www.0xarchive.io/data). Bulk streaming over WebSocket has been discontinued, and the deprecated `stream()` and `stream_stop()` methods now receive an error from the server.

> Lighter live subscriptions are available for `lighter_orderbook`, `lighter_trades`, `lighter_open_interest`, and `lighter_funding` on mainnet, and for `rh_lighter_orderbook`, `rh_lighter_trades`, `rh_lighter_open_interest`, and `rh_lighter_funding` on Robinhood Chain. `lighter_candles`, `lighter_l3_orderbook`, and `rh_lighter_candles` remain replay-only. Every Lighter channel on both deployments supports historical replay.

> The `mempool` channel (pending Hyperliquid transactions) is served only at `wss://stream.0xarchive.io/ws` and is included with the Pro, Scale and Enterprise plans. See [Pending Transactions (Mempool)](#pending-transactions-mempool).

The client connects with `version=2026-10-01`. Under that version, error messages carry an `error_code` (see [Error Handling](#error-handling)), and Lighter replay rows have the same shapes as the live payloads. `client.capabilities()` lists which channels stream live and which replay.

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
        ServerMsg::Error { message, error_code } => eprintln!("Error ({error_code:?}): {message}"),
        _ => {}
    }
}
```

### Lighter Live Streaming

Live Lighter data uses the same envelope as Hyperliquid live data and is served on
`wss://api.0xarchive.io/ws`, the default `ws_url` (not `wss://stream.0xarchive.io/ws`).
Symbols are the same as `client.lighter.instruments.list()`; they are case-insensitive
on subscribe and echoed uppercase.

The Robinhood Chain deployment has the same four live channels with the `rh_lighter_`
prefix (`rh_lighter_orderbook`, `rh_lighter_trades`, `rh_lighter_open_interest`,
`rh_lighter_funding`), using the symbols of `client.rh_lighter.instruments.list()`.
Their payloads have exactly the mainnet shapes below and decode with the same
`msg.lighter_live_data()`; `rh_lighter_orderbook` also accepts `interval_ms` from 100
to 5000 (default 1000). They are also served only on `wss://api.0xarchive.io/ws`. Live Lighter channels are available on every tier and
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
// Robinhood Chain: same payloads, rh_lighter_ channels.
ws.subscribe("rh_lighter_trades", Some("AAPL-USDG")).await?;
ws.subscribe_with_interval("rh_lighter_orderbook", "BTC", 500).await?;

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
            if let ServerMsg::Error { message, error_code } = msg {
                // Lag notices carry `slow_consumer` and do not always end the
                // subscription (see below).
                eprintln!("{error_code:?}: {message}");
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
`lighter_funding`, the server sends an `error` notice with `error_code`
`slow_consumer`, such as
`Dropped ~N live lighter_trades messages for BTC: ...`, and the subscription continues.
If the lag persists, a notice such as `Stopped the lighter_trades stream for BTC: ...`
ends that subscription; subscribe again to resume. `lighter_orderbook` never sends an
older book in place of a newer one.

### Pending Transactions (Mempool)

The `mempool` channel streams signed Hyperliquid transactions (orders, cancels,
modifies, TWAPs, leverage changes, transfers and every other action type) as our
Hyperliquid node receives them from its peers, before they are included in a block.
It covers every Hyperliquid product: perps, HIP-3, HIP-4 and spot.

- **Live only.** Pending transactions are never stored. There is no replay, history,
  REST route or export.
- **One endpoint.** It is served only at `wss://stream.0xarchive.io/ws`
  (`oxarchive::ws::STREAM_WS_URL`), with the same API key and protocol. At
  `wss://api.0xarchive.io/ws`, the default `ws_url`, a subscribe is answered with
  `ErrorCode::EndpointUnsupported`. Use a separate connection for the channels served
  at the default endpoint.
- **Plans.** It is included with the Pro, Scale and Enterprise plans; on other plans a
  subscribe is answered with `ErrorCode::Forbidden`. Every other channel stays on every
  plan, Free included. Each data message is metered like any other WebSocket message.

```rust
use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions, MEMPOOL_CHANNEL, STREAM_WS_URL};

let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key").ws_url(STREAM_WS_URL));
ws.connect().await?;
let mut rx = ws.rx.take().expect("receiver");

ws.subscribe(MEMPOOL_CHANNEL, Some("BTC")).await?;      // actions that reference BTC
ws.subscribe(MEMPOOL_CHANNEL, Some("xyz:TSLA")).await?; // HIP-3; spot "HYPE-USDC", HIP-4 "#49720"
// ws.subscribe(MEMPOOL_CHANNEL, None).await?;          // pending transactions our node receives (unfiltered)

while let Some(msg) = rx.recv().await {
    match msg {
        ServerMsg::Mempool { symbol, items, .. } => {
            for item in items {
                // item.action.get() is the action's exact bytes, for signature recovery.
                let action = item.action_value()?; // parsed, for inspection only
                println!("{symbol:?} {:?} {} {:?}", item.received_at, action["type"], item.symbols);
            }
        }
        ServerMsg::Error { message, error_code } => eprintln!("{error_code:?}: {message}"),
        _ => {}
    }
}
```

The symbol is optional on this channel only. Without it you receive every pending
transaction our Hyperliquid node receives; with it, every action whose asset ids include that market, whole (an
order batch that touches `BTC` and `ETH` reaches both subscriptions). Symbols are
spelled as everywhere else: perps `BTC`, HIP-3 `xyz:TSLA`, spot `HYPE-USDC`
(`HYPE/USDC` is also accepted) and HIP-4 `#49720`. An unknown symbol is answered with
`ErrorCode::InvalidSymbol`. The `subscribed` acknowledgement carries the canonical
symbol, or `None` for the unfiltered stream.

The server sends one message per batch of transactions as it arrives. The client
delivers it as `ServerMsg::Mempool`, with `coin` and `symbol` set to the
subscription's symbol (`None` when unfiltered) and one `MempoolItem` per signed
action in `items`:

| Field | Description |
|-------|-------------|
| `received_at` | When our node received the transaction: an RFC 3339 UTC string with nanosecond precision. Not a block time. |
| `received_at_ms` | The same time in Unix milliseconds. |
| `symbols` | Markets the action's asset ids reference, in first-seen order without repeats. Empty for actions with no market, such as transfers, `noop`, `scheduleCancel` and validator actions. |
| `action` | The action exactly as signed, in Hyperliquid's exchange-action format: asset ids (`a` or `asset`) rather than symbols, prices and sizes as strings. It is a `Box<RawValue>` that keeps the exact bytes the server sent, key order included, for signature recovery (`item.action.get()`). `item.action_value()` parses it into a `serde_json::Value` for inspection, which may not keep the original key order. |
| `nonce` | The action's nonce. |
| `vault_address` | The vault or subaccount the action acts for, or `None`. |
| `expires_after_ms` | The action's `expiresAfter` in Unix milliseconds, or `None`. |
| `signature` | `MempoolSignature` with `r`, `s` and `v`. The signer's address is not included. |

The action's `type` names it, for example `order`, `cancel`, `cancelByCloid`,
`modify`, `batchModify`, `scheduleCancel`, `twapOrder`, `twapCancel`,
`updateLeverage`, `updateIsolatedMargin`, `noop`, `evmRawTx`, or a transfer such as
`usdSend`, `spotSend`, `usdClassTransfer` or `sendAsset`. Hyperliquid adds action
types, so handle types you do not recognise.

A pending transaction is not an executed one: it can still be rejected, expire or
never land in a block. The same signed action can occasionally arrive twice;
deduplicate on `signature` if that matters to you.

**Volume and limits.** The unfiltered stream is several megabytes per second before
compression, and this client does not negotiate permessage-deflate compression, so
subscribe with a symbol where you can. Unfiltered subscriptions are limited
server-wide, and when they are at capacity a subscribe without a symbol is answered
with `ErrorCode::RateLimited`; symbol subscriptions are not capped this way. A
connection that reads too slowly is disconnected, as on any channel, and the usual
per-connection limits apply (subscriptions per plan, 10 subscribe operations per
second). If the feed is temporarily unavailable, a subscribe is answered with
`ErrorCode::UpstreamUnavailable`.

### Historical Replay

Replay a bounded historical window with original timing preserved. Every replay
ends with a `replay_completed` server message.

Lighter order book, trades, open interest, and funding replay rows have the
live shapes and decode with `msg.lighter_live_data()`, like live data. A
replayed trades row is a single fill leg.

Every L4 channel (`l4_diffs`, `l4_orders` and their `hip3_`, `spot_` and
`hip4_` counterparts) and both full-depth order book channels
(`orderbook_full`, `hip3_orderbook_full`) replay in bulk: the book at the
nearest L4 checkpoint at or before `start` arrives as an `l4_snapshot` frame
(`ServerMsg::L4Snapshot`), then the events follow as ordered `l4_batch` frames
(`ServerMsg::L4Batch`) as fast as the server sends them. `speed` is ignored
(pass `None`), `replay.seek` is refused, and these channels replay one at a time:
`replay_multi()` rejects them before sending. L4 replay starts at
2026-03-11 01:03 UTC for Hyperliquid, HIP-3 and Spot, and at
2026-05-02 07:47 UTC for HIP-4.

A replay request on a channel without replay (`ticker`, `all_tickers`, and the
Spot `spot_orderbook` and `spot_trades` channels) is answered with an error
whose `error_code` is `unsupported_for_venue`.

```rust
use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions};
use oxarchive::LighterLiveData;

let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key"));
ws.connect().await?;
let mut rx = ws.rx.take().expect("receiver");

// The hour that ended ten minutes ago, in Unix milliseconds
let end = chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000;
let start = end - 60 * 60 * 1000;

// All six Lighter channels support replay. Book, trades, open interest and
// funding rows have the live shapes shown above.
ws.replay(
    "lighter_orderbook",
    "BTC",
    start,
    Some(end),
    Some(10.0), // 10x; the fastest speed depends on the plan (see Tier Limits)
).await?;

while let Some(msg) = rx.recv().await {
    match msg {
        ServerMsg::ReplayCompleted { channel, snapshots_sent, .. } => {
            println!("Replay complete: {channel}, {snapshots_sent:?} records");
            break;
        }
        ServerMsg::HistoricalData { ref channel, .. } => {
            if let Some(Ok(LighterLiveData::OrderBook(book))) = msg.lighter_live_data() {
                println!("{channel}: {} bids, {} asks", book.bids().len(), book.asks().len());
            }
        }
        _ => {}
    }
}

// Control playback for the active bounded replay
ws.replay_pause().await?;
ws.replay_resume().await?;
ws.replay_seek(start + 30 * 60 * 1000).await?;
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
| `mempool` | Pending transactions on every Hyperliquid product, before they are in a block. Symbol optional. Pro, Scale and Enterprise plans; served only at `wss://stream.0xarchive.io/ws`. | Yes | No |
| `orderbook_full` | Hyperliquid core full-depth L2 order book, aggregated from order-level data (every price level, no user attribution): an `l4_snapshot` frame (`ServerMsg::L4Snapshot`) with the whole book, then `l4_batch` frames (`ServerMsg::L4Batch`) of price-level changes (`side`, `px`, `sz`, `n`, `bn`; `sz` and `n` are `0` when a level is removed). | Yes | Yes, bulk and single-channel, from 2026-03-11 01:03 UTC |
| `lighter_orderbook` | Lighter L2 order book | Yes | Yes |
| `lighter_trades` | Lighter trades | Yes | Yes |
| `lighter_candles` | Lighter candles | No | Yes |
| `lighter_open_interest` | Lighter open interest | Yes | Yes |
| `lighter_funding` | Lighter funding rates | Yes | Yes |
| `lighter_l3_orderbook` | Lighter L3 order-level orderbook | No | Yes |
| `rh_lighter_orderbook` | Lighter on Robinhood Chain L2 order book | Yes | Yes, from 2026-08-22 18:43 UTC |
| `rh_lighter_trades` | Lighter on Robinhood Chain trades | Yes | Yes, from 2026-06-26 20:10:26 UTC |
| `rh_lighter_candles` | Lighter on Robinhood Chain candles | No | Yes, from 2026-06-26 20:10 UTC |
| `rh_lighter_open_interest` | Lighter on Robinhood Chain open interest | Yes | Yes, from 2026-08-22 18:43 UTC |
| `rh_lighter_funding` | Lighter on Robinhood Chain funding rates | Yes | Yes, from 2026-08-22 18:43 UTC |
| `hip3_orderbook` | HIP-3 L2 order book | Yes | Yes |
| `hip3_trades` | HIP-3 trades | Yes | Yes |
| `hip3_candles` | HIP-3 candles | No | Yes |
| `hip3_open_interest` | HIP-3 open interest | Yes | Yes |
| `hip3_funding` | HIP-3 funding rates | Yes | Yes |
| `hip3_liquidations` | HIP-3 liquidation events. Same wire shape as `liquidations`. | Yes | Yes |
| `hip3_orderbook_full` | HIP-3 full-depth L2 order book. Same frames as `orderbook_full`. | Yes | Yes, bulk and single-channel |
| `hip4_orderbook` | HIP-4 outcome-market L2 order book | No | Yes |
| `hip4_trades` | HIP-4 trade/fill updates | Yes | Yes |
| `hip4_open_interest` | HIP-4 open interest snapshots | No | Yes |
| `l4_diffs` | Hyperliquid core L4 orderbook diffs with user attribution | Yes | Yes, `l4_snapshot` then ordered `l4_batch`; bulk and single-channel |
| `l4_orders` | Hyperliquid core L4 order lifecycle events | Yes | Yes, `l4_snapshot` then ordered `l4_batch`; bulk and single-channel |
| `hip3_l4_diffs` | HIP-3 L4 orderbook diffs with user attribution | Yes | Yes, as `l4_diffs` |
| `hip3_l4_orders` | HIP-3 order lifecycle events | Yes | Yes, as `l4_orders` |
| `hip4_l4_diffs` | HIP-4 L4 orderbook diffs with user attribution | Yes | Yes, as `l4_diffs`, from 2026-05-02 07:47 UTC |
| `hip4_l4_orders` | HIP-4 order lifecycle events | Yes | Yes, as `l4_orders`, from 2026-05-02 07:47 UTC |
| `spot_orderbook` | Hyperliquid Spot L2 order book | Yes | No |
| `spot_trades` | Hyperliquid Spot trades | Yes | No |
| `spot_l4_diffs` | Hyperliquid Spot L4 orderbook diffs with user attribution | Yes | Yes, as `l4_diffs` |
| `spot_l4_orders` | Hyperliquid Spot order lifecycle events | Yes | Yes, as `l4_orders` |

Current Lighter order books, trades, open interest, and funding are available as live subscriptions and through the Lighter REST resources; current Lighter candles and L3 order books are available through REST. Historical Lighter data is available through REST, WebSocket replay, or exports. The same holds for the Robinhood Chain deployment (`rh_lighter_*` channels, `client.rh_lighter`), which has no L3. A multi-channel replay cannot mix `lighter_*` and `rh_lighter_*` channels.

HIP-4 has no funding or liquidation channels and no candle channel. `hip4_orderbook` and `hip4_open_interest` replay stored history from 2026-05-02 16:51 UTC but have no live subscription; read the current HIP-4 order book and outcome-side OI over REST. HIP-4 candles are served over REST from 2026-05-02 08:00 UTC.

Hyperliquid Spot has no funding, open-interest, or liquidation resources. Spot candles (from 2025-03-22 10:50 UTC) and Spot TWAP statuses (from 2026-05-05 13:05 UTC) are served over REST only, with no WebSocket channel. Spot symbols use the dashed canonical form (`HYPE-USDC`, `PURR-USDC`).

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

All self-serve tiers reach the published route families; Free covers the most recent rolling 30 days of history (30-day span per request or replay), and Build and above keep the retained archive. Schema availability remains family-specific; plans gate capacity and Free's 30-day history window, not route families, schemas, or served depth. The one exception is the live `mempool` channel, included with the Pro, Scale and Enterprise plans.

| Tier | Max Subscriptions | Max Connections | Max Replay Speed |
|------|------------------|-----------------|------------------|
| Free | 10 | 2 | 10x |
| Build | 500 | 3 | 50x |
| Pro | 3,000 | 5 | 100x |
| Scale | 20,000 | 16 | 300x |
| Enterprise | Custom | Custom | 1000x |

On every tier, each WebSocket connection accepts at most 10 subscribe operations per second;
faster bursts are rejected with a "Subscription rate limit exceeded" error.

## Timestamp Formats

All time parameters accept the `Timestamp` enum, sent as Unix milliseconds. A
time without a time zone is UTC: `"2026-10-01"` is midnight UTC,
`"2026-10-01T12:00:00"` is noon UTC, and `chrono::NaiveDateTime` and
`chrono::NaiveDate` convert as UTC. A string with an offset (`Z`, `+02:00`)
keeps it.

Response times are RFC 3339 UTC strings. Where a response also gives the
instant as an integer, that field ends in `_ms` (`timestamp_ms`,
`snapshot_ts_ms`) and holds Unix milliseconds.

```rust
use oxarchive::types::Timestamp;

// Unix milliseconds (i64): 2026-10-01 00:00 UTC
let ts: Timestamp = 1790812800000_i64.into();

// ISO 8601 string: with an offset, or without one (UTC)
let ts: Timestamp = "2026-10-01T00:00:00Z".into();
let ts: Timestamp = "2026-10-01T12:00:00".into();
let ts: Timestamp = "2026-10-01".into();

// chrono::DateTime<Utc>, or a naive date or date-time (UTC)
let ts: Timestamp = chrono::Utc::now().into();
let ts: Timestamp = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap().into();
```

## Error Handling

An API error is `Error::Api`, with the HTTP status (`code`), a stable
`error_code`, the `request_id` to quote to support and, when the error is
about one parameter, `param` and the `valid_values` it accepts. Branch on
`error_code`; `message` is for people and its wording can change.
`ErrorCode` names every documented code and keeps any other as
`ErrorCode::Other`:

| `error_code` | Meaning |
|--------------|---------|
| `invalid_parameter`, `invalid_symbol`, `invalid_interval`, `invalid_cursor`, `invalid_time_range` | The request is malformed. |
| `range_before_coverage` | The whole range is before the dataset's coverage begins. |
| `historical_range_exceeded`, `historical_depth_exceeded` | The span or the history window is beyond the plan's limit. |
| `unsupported_for_venue` | The venue does not offer this datatype, channel or mode; the message names where it is offered. |
| `route_not_found`, `not_found` | Unknown path, or unknown resource id. |
| `unauthorized`, `forbidden`, `insufficient_credits` | Credentials, access or credits. |
| `rate_limited`, `upstream_unavailable`, `internal_error` | Retry later. |
| `conflict` | The request conflicts with current state. |
| `slow_consumer`, `endpoint_unsupported` | WebSocket only: the connection fell behind and messages were dropped (subscribe again or restart the replay), or this endpoint does not serve the channel. |

```rust
use oxarchive::{Error, ErrorCode};

match client.hyperliquid.orderbook.get("BTC", None).await {
    Ok(ob) => println!("Mid price: {:?}", ob.mid_price),
    Err(Error::Api { error_code: Some(ErrorCode::RateLimited), .. }) => {
        // back off and retry
    }
    Err(Error::Api { message, code, error_code, param, valid_values, request_id }) => {
        eprintln!("API error {code} {error_code:?}: {message} (param {param:?}, valid {valid_values:?}, request {request_id:?})");
    }
    Err(Error::Timeout) => eprintln!("Request timed out"),
    Err(Error::Http(e)) => eprintln!("HTTP error: {e}"),
    Err(e) => eprintln!("Other error: {e}"),
}
```

`Error` also has `error_code()`, `status()`, `request_id()`, `param()` and
`valid_values()` accessors. WebSocket `ServerMsg::Error` messages carry the
same `error_code`.

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

# Liquidation levels, trigger levels, candles, and liquidations by user
cargo run --example levels_smoke

# WebSocket (requires websocket feature)
cargo run --example websocket --features websocket
```

## Data Catalog

For large-scale data exports (route-specific order books, fill-level trade history, and other retained datasets), use the [Data Catalog](https://www.0xarchive.io/data). It lets you choose markets, datasets, and date ranges, see a live quote, and export zstd-compressed Parquet.

## Links

- [API Docs](https://docs.0xarchive.io)
- [Python SDK](https://pypi.org/project/oxarchive/)
- [TypeScript SDK](https://npmjs.com/package/@0xarchive/sdk)
- [CLI](https://npmjs.com/package/@0xarchive/cli)
- [MCP Server](https://docs.0xarchive.io/mcp-server)
- [0xArchive Skill](https://github.com/0xArchiveIO/0xarchive-skill)
- [Examples](https://github.com/0xArchiveIO/examples)

## Requirements

- Rust 1.85+
- tokio runtime

## License

MIT
