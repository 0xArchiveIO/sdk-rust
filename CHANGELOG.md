# Changelog

All notable changes to the `oxarchive` Rust SDK are tracked in this file.
The format is loosely based on [Keep a Changelog](https://keepachangelog.com).

## [1.9.1] - Unreleased

### Changed
- Lighter WebSocket channels are explicitly replay-only: current data remains
  available through REST, all six channels remain available for bounded
  historical replay, and live subscription requests fail fast with REST/replay
  guidance.

## [1.9.0] - 2026-08-22

### Added
- Typed HIP-4 candle history at `client.hyperliquid.hip4.candles.history()`;
  served from 2026-05-02 with a 1,000-row page maximum.
- Typed Hyperliquid Spot candle history at
  `client.hyperliquid.spot.candles.history()`; coverage starts exactly at
  2025-03-22T10:50:22Z, supports `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`,
  and `1w`, and accepts at most 1,000 rows per page. API-returned cursor
  strings are passed through unchanged.

### Changed
- Coverage copy now states HIP-4 outcome-side OI at roughly 10-second cadence,
  Lighter candles from 2025-08-01, Lighter L3 at 250 orders per side from
  March 5, 2026, and Lighter per-fill trade history from August 27, 2025.
- HIP-4 WebSocket docs now distinguish live trades/L4/settlement delivery from
  stored-replay-only L2 and OI while those live bridges are paused.
- HIP-4 path documentation now uses the bare numeric form (`"0"`) as primary
  while retaining legacy `"#0"` compatibility with percent-encoded transport.
- Candle page validation now matches the served route caps: 10,000 rows for
  Hyperliquid, HIP-3, and Lighter; 1,000 for HIP-4 and Spot.
- `Hip4OpenInterestRecord` no longer advertises `oracle_price`, and Lighter L3
  `depth` is validated as 1 through 250 individual resting orders per side.

## [1.8.0] - 2026-07-27

### Added
- **Liquidation levels**: `liquidations.levels()` and `levels_history()` on
  the Hyperliquid and HIP-3 clients. Projected forced-liquidation levels
  computed from clearinghouse positions and margin state (~45-minute
  snapshots, `at` point-in-time reads, `side` filter, cursor-paginated
  history with `summary` mode). History retained from 2026-07-27.
- **Trigger levels**: `orders.trigger_levels()` and
  `trigger_levels_history()` — the pending stop-loss / take-profit map
  (15-minute snapshot history). Typed models exported at the crate root.
- **WebSocket L4 frames**: `ServerMsg::L4Snapshot` and `ServerMsg::L4Batch`.
  Previously these server messages failed to deserialize and were silently
  discarded, so L4 channel subscribers received nothing.
- `ServerMsg::Unknown` catch-all: unrecognized message types now surface as
  a variant instead of being silently dropped.
- `L4DiffEntry.seq` (within-block sequence, default 0 on pre-native-seq
  rows) and `L4DiffEntry.insert_before` (ALO queue-priority target oid).
- `CoinSummary.volume_24h` (Lighter naming; `day_ntl_volume` is
  Hyperliquid-only).
- **`ApiMeta.coverage_from` / `ApiMeta.notice`**: empty responses for range
  windows that end before a symbol's coverage begins now carry the coverage
  start date and an advisory notice.

### Fixed
- `liquidations.by_user()` hit `/liquidations/{address}` instead of
  `/liquidations/user/{address}` — the server treated the wallet as a coin
  symbol and returned an empty array, silently.
- `Candle` OHLCV fields declared plain `String` but the wire serves JSON
  numbers, so every `candles.history()` call failed to deserialize on all
  mounts. Fields now accept numbers or strings (still stored as `String`).
- `data_quality.sla()` now takes `(year, month)` matching the API contract;
  the old lone `month` string was silently ignored by the server.

### Changed
- The server-side `/liquidations/{symbol}/levels` endpoints now serve
  projected forced-liquidation levels; the pending trigger-order map moved
  to `/orders/{symbol}/trigger-levels`.
- `ServerMsg` gained variants; exhaustive matches on it will need new arms.

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
- At the time of the 1.7.0 release, Spot had **no funding, no open interest,
  no liquidations, and no candles**: the candles endpoint returned 501 and the
  SDK did not expose it. Spot candle history is now available as of 1.9.0;
  see the current release notes above for its served coverage.
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
- **HIP-4 WebSocket channel helpers.** New channels (delivered via the existing
  `Data` envelope as channel-name strings):
  - `hip4_trades` (live + replay), `hip4_orderbook` and `hip4_open_interest`
    (stored replay; live bridges currently paused).
  - `hip4_l4_diffs`, `hip4_l4_orders` (live only).
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
- HIP-4 has no funding or liquidations. Candle history and outcome-side OI are
  served from May 2, 2026.

## [1.5.0]

- Liquidation volume bucket fields are now `String` with a flexible
  deserializer that accepts JSON numbers or strings.
- Empty / invalid-header API keys are rejected at construction.
- Future-dated candle queries are rejected client-side.
- `data_quality.coverage()` uses an extended timeout independent of the
  client default.
