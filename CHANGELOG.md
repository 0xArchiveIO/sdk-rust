# Changelog

All notable changes to the `oxarchive` Rust SDK are tracked in this file.
The format is loosely based on [Keep a Changelog](https://keepachangelog.com).

## [1.7.0] - 2026-05-06

### Added
- **Hyperliquid Spot support** under `client.hyperliquid.spot` (REST base
  `/v1/hyperliquid/spot`). Symbols are dashed canonical (`HYPE-USDC`,
  `PURR-USDC`); the server resolves the dashed form to the wire format
  (`PURR/USDC`, `@107`) internally.
  - `pairs.list()`, `pairs.get(symbol)`: pair discovery (`/pairs`,
    `/pairs/{symbol}`).
  - `orderbook.get(symbol, params)`, `orderbook.history(...)`: current and
    historical L2 orderbook.
  - `l4_orderbook.get(...)`, `l4_orderbook.diffs(...)`,
    `l4_orderbook.history(...)`: L4 reconstruction (Pro+), raw diffs
    (Pro+), checkpoint history (Build+).
  - `trades.list(symbol, params)`: trade history. Backfills to 2025-03-22.
  - `orders.history(symbol, params)`: order lifecycle events (Pro+).
  - `twap.by_symbol(symbol, params)`, `twap.by_user(user, params)`:
    TWAP execution statuses.
  - `freshness(symbol)`: per-table lag.
- **Spot WebSocket channels** (delivered via the existing `Data` envelope as
  channel-name strings): `spot_orderbook` (Build+), `spot_trades` (Build+),
  `spot_l4_diffs` (Pro+), `spot_l4_orders` (Pro+), `spot_twap` (Build+).
- **`SpotPair` and `SpotTwapStatus` types** in `oxarchive::types`.
- **New `examples/spot.rs`** mirroring the HIP-3 example.

### Notes
- Spot has **no funding, no open interest, no liquidations, no candles**:
  those are perp-only constructs. The candles endpoint returns 501 by
  design; the SDK does not expose it.
- Trade history backfills to 2025-03-22 (the earliest published Hyperliquid
  S3 spot data). Orderbook, L4, TWAP, and freshness are live-only from
  2026-05-05.

## [1.6.0] - 2026-05-04

### Added
- **`OxArchive::from_env()`.** Constructs a client by reading the
  `OXARCHIVE_API_KEY` environment variable. Returns
  `Error::InvalidParam` if the variable is unset.
- **Real-time WebSocket support for liquidations.** `liquidations` and
  `hip3_liquidations` channels now stream live with the same wire shape as
  `trades` (each item is a fill row with `is_liquidation: true`). Both
  channels also support historical replay.
- **Full HIP-4 WebSocket surface.** New channels (delivered via the existing
  `Data` envelope as channel-name strings):
  - `hip4_orderbook`, `hip4_trades`, `hip4_open_interest` (realtime + replay).
  - `hip4_l4_diffs`, `hip4_l4_orders` (realtime only, Pro+).
- **`outcome_settled` server message variant** in `ServerMsg`. Emitted at
  most once per `(outcome_id, side)` when a HIP-4 outcome settles. The
  server proactively unsubscribes the client from every `hip4_*`
  subscription on the coin; other subscriptions remain active. Carries
  `coin`, `outcome_id`, `side`, `settlement_value`, `settlement_at`.
- **HIP-4 REST: `get_outcome_by_slug`.** Resolves either the per-outcome
  slug (`btc-above-78213-may-04-0600`) or the per-side slug
  (`btc-above-78213-yes-may-04-0600`) to the same outcome detail, including
  `aggregated_oi`.
- **HIP-4 REST: `slug` filter on `list_outcomes`.** New
  `Hip4ListOutcomesParams.slug` short-circuits the list to a single match.
- **HIP-4 outcome fields.** `Hip4Outcome` and `Hip4OutcomeAggregate` now
  expose `display_title` and `slug`. `Hip4SideSpec` gains `display_title`
  and `slug`. `Hip4OutcomeAggregate` also exposes `outcome_pair`
  (`["#0", "#1"]`) for symmetry with `/v1/symbols`. `Hip4Outcome` now
  surfaces `settlement_value` and `settlement_at`.
- **HIP-4 OI: `oracle_price`** on `Hip4OpenInterestRecord`.

### Changed
- **HIP-4 path encoding clarified.** The user-facing API takes the **bare**
  numeric form (`"#0"`). Backend accepts the bare form as well. The SDK
  percent-encodes `#` to `%23` strictly at the URL wire layer because raw
  `#` is the fragment delimiter per RFC 3986 and is otherwise stripped by
  HTTP clients. No SDK call signatures change; documentation and examples
  recommend the bare form.
- **`mark_price` doc comment** on `Hip4OpenInterestRecord` clarifies it is
  an implied probability in `[0, 1]`, not a USD price.

### Fixed
- **`client.hyperliquid.trades.recent()` now fails fast** with
  `Error::InvalidParam` instead of issuing an HTTP request that returns
  404. The Hyperliquid base namespace does not expose `/recent`; use
  `trades.list()` with a time range. `recent()` continues to work on
  `client.lighter.trades` and `client.hyperliquid.hip3.trades`.

### Notes
- HIP-4 still has **no funding, no liquidations, and no candles** by design.
  Outcomes settle to 0/1 at expiry; OHLCV can be reconstructed from
  `hip4_fills` if needed.

## [1.5.0]

- Liquidation volume bucket fields are now `String` with a flexible
  deserializer that accepts JSON numbers or strings.
- Empty / invalid-header API keys are rejected at construction.
- Future-dated candle queries are rejected client-side.
- `data_quality.coverage()` uses an extended timeout independent of the
  client default.
