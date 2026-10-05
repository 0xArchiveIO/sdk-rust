# Changelog

All notable changes to the `oxarchive` Rust SDK are tracked in this file.
The format is loosely based on [Keep a Changelog](https://keepachangelog.com).

## [1.12.0] - 2026-09-28

### Upgrading from 1.8

1.8.0 is the last release published to crates.io before this one, so
upgrading from it also brings in the changes listed under 1.9.0 to 1.11.0
below. These need code changes:

- **Rust 1.85 or later.** The minimum supported Rust version was 1.75.
- **Removed fields.** `L4HistoryParams::depth` and
  `Hip4L4HistoryParams::depth` (L4 history returns whole snapshots), and
  `Hip4OpenInterestRecord::oracle_price` (HIP-4 has no oracle price).
- **Changed types.** `GetTradesParams::side` and `Hip4TradesParams::side` are
  `Option<TradeSide>` (`TradeSide::Buy` or `TradeSide::Sell`) instead of
  `Option<String>`. `client.hyperliquid.spot.orders` is a
  `SpotOrdersResource` whose `history()` takes `SpotOrderHistoryParams`, and
  `client.hyperliquid.hip3.liquidations` is a `Hip3LiquidationsResource`.
- **Removed methods.** `client.hyperliquid.hip3.liquidations.by_user()`, and
  `flow()`, `tpsl()`, `trigger_levels()` and `trigger_levels_history()` on
  `client.hyperliquid.spot.orders`. They called routes the API does not
  serve and always failed.
- **New fields on parameter structs.** A struct literal that lists every
  field needs the new ones: `OrderHistoryParams::triggered`,
  `Hip4OrderHistoryParams::triggered`, `OrderFlowParams::cursor`,
  `Hip4OrderFlowParams::cursor` and `L3HistoryParams::account`. End
  literals with `..Default::default()` where the struct implements
  `Default`, or set the field to `None`.
- **New fields on response types.** `CursorResponse` gains `has_more` and
  `meta`; `L4OrderEntry` gains `timestamp` and `timestamp_ms`;
  `OrderHistoryEntry` gains `trigger_condition` and `trigger_price`;
  `LiquidationLevels`, `LiquidationLevelsHistoryItem` and
  `TriggerLevelsHistoryItem` gain `snapshot_ts_ms`; `CoinFreshness` gains
  `symbol`; `IncidentsResponse` gains `pagination`. This affects only code
  that builds these types with struct literals, such as test fixtures.
- **New fields on enum variants.** `Error::Api` gains `error_code`, `param`
  and `valid_values`, and `ServerMsg::Error` gains `error_code`. A pattern
  that names every field needs them or `..`.
- **New enum variants.** `OiFundingInterval::OneMinute` and
  `ClientMsg::SubscribeWithInterval`. A `match` without a wildcard arm needs
  an arm for each, and an `as` cast of an `OiFundingInterval` value gives a
  different number.
- **`OxArchive`** has a private field, so it can no longer be built with a
  struct literal. Use `OxArchive::new()` or `OxArchive::builder()`.

These change what a call returns, with no code change needed:

- Every request selects API version 2026-10-01. The SDK's types read the
  shapes it selects; code that reads raw JSON or `extra` sees RFC 3339 times
  with integer `*_ms` fields alongside, and Lighter replay rows in the live
  shapes, which `ServerMsg::lighter_live_data()` decodes.
- Lighter `funding_rate` values are decimal fractions (`0.0001` means
  `0.01%`), not percentages (since 1.9.1).
- `Trade::fee`, `closed_pnl` and `start_position` are `"0"` when the venue
  recorded a zero; `None` means the source did not record the value (since
  1.10.0).
- `OxArchiveWs::stream()` and `stream_stop()` are deprecated: the server no
  longer streams in bulk over WebSocket (since 1.11.0).

### Added
- API version 2026-10-01. Every request sends the `0xArchive-Version:
  2026-10-01` header and the WebSocket client connects with
  `version=2026-10-01` (`API_VERSION`, `API_VERSION_HEADER`). The version
  selects the standard `{success, data, meta}` envelope on every route,
  RFC 3339 UTC times with an integer `*_ms` field alongside, the stable error
  codes, and the live row shapes on Lighter replay.
- Typed error codes. `Error::Api` gains `error_code` (`ErrorCode`), `param`
  and `valid_values` next to `code` (the HTTP status) and `request_id`, and
  `Error` has `error_code()`, `status()`, `request_id()`, `param()` and
  `valid_values()` accessors. `ErrorCode` names every documented code
  (`invalid_parameter`, `invalid_symbol`, `invalid_interval`,
  `invalid_cursor`, `invalid_time_range`, `range_before_coverage`,
  `historical_range_exceeded`, `historical_depth_exceeded`,
  `unsupported_for_venue`, `route_not_found`, `not_found`, `unauthorized`,
  `forbidden`, `insufficient_credits`, `rate_limited`, `conflict`,
  `upstream_unavailable`, `internal_error`, `endpoint_unsupported`,
  `slow_consumer`, `positions_unavailable`, `api_key_limit_reached`,
  `oauth_not_permitted`) and keeps any other as `ErrorCode::Other`. The
  error's `Display` output includes the code.
- WebSocket `ServerMsg::Error` carries `error_code`, and
  `ServerMsg::error_code()` reads it. Lag notices carry `slow_consumer`; an
  endpoint that does not serve a channel answers `endpoint_unsupported`; a
  channel without a mode (live, replay, `replay.seek`) answers
  `unsupported_for_venue`.
- `has_more` on every paged response. `CursorResponse` and `MetaResponse`
  are one type with `data`, `next_cursor`, `has_more` and the full `meta`
  block; page while `has_more` is `true`. `ResponseMeta` gains `has_more`,
  `symbol` (the canonical public symbol a per-symbol route answered for, such
  as `#0` for a HIP-4 `"0"`) and `venue` (`hyperliquid`, `hip3`, `hip4`,
  `spot`, `lighter` or `rh-lighter`).
- `client.capabilities()` (`GET /v1/capabilities`): one `Capability` row per
  venue and datatype with its REST routes, WebSocket channels, `live` and
  `replay` flags, `available_from`, cadence, `page_limit` and `intervals`.
  `Capability::for_channel()` finds a channel's row.
- `client.data_quality.status_coverage()` (`GET /v1/status/coverage`), the
  public coverage summary, and `client.list_symbols()`, the same call as
  `client.symbols()`.
- Trade side filter: `side` (`TradeSide::Buy` or `TradeSide::Sell`, sent as
  `buy` or `sell`) on `GetTradesParams` and `Hip4TradesParams`, and
  `RecentTradesParams` for `trades.recent_with()` and
  `hip4.get_trades_recent_with()`, on every venue. The filter keeps rows
  whose `side` is `"B"` or `"A"` and applies before paging.
- `triggered` on `OrderHistoryParams` and `Hip4OrderHistoryParams`:
  `Some(true)` keeps only orders whose trigger fired, `Some(false)` leaves
  them out (Hyperliquid, HIP-3 and HIP-4).
- `depth` on `L2HistoryParams` (full-depth L2 history, Hyperliquid and
  HIP-3) and `Hip4OrderBookHistoryParams`, which
  `hip4.get_orderbook_history()` now takes; a `Hip4HistoryRange` still
  converts into it. `OrderBookHistoryParams::depth` applies to HIP-3 and
  Spot order book history too.
- Verb aliases, next to the existing names: `trades.history()` for
  `trades.list()`, and on HIP-4 `list_instruments()`, `get_trades_history()`,
  `get_open_interest_history()` and `get_price_history()` for
  `get_instruments()`, `get_trades()`, `get_open_interest()` and
  `get_prices()`.
- `orderbook.history_tick_page()` with `TickPageParams` and `TickPage`: one
  page of tick-level order book data with its cursor and `has_more`, for
  paging through a range. The first page carries the checkpoint; later pages
  carry deltas only.
- WebSocket replay for every L4 channel and both full-depth order book
  channels: `hip3_l4_diffs`, `hip3_l4_orders`, `spot_l4_diffs`,
  `spot_l4_orders`, `hip4_l4_diffs`, `hip4_l4_orders`, `orderbook_full` and
  `hip3_orderbook_full` replay like core `l4_diffs` and `l4_orders`: an
  `l4_snapshot` from the nearest L4 checkpoint, then ordered `l4_batch`
  frames, in bulk (`speed` is ignored, `replay.seek` is refused) and one
  channel at a time. `replay()` sends them; `replay_multi()` rejects them
  before sending. `L4_REPLAY_CHANNELS`, `is_l4_channel()`,
  `FULL_DEPTH_CHANNELS`, `is_full_depth_channel()` and
  `is_single_channel_replay()` name them.
- `OrderHistoryEntry` gains `trigger_condition` and `trigger_price`, sent on
  `triggered` rows, and decodes rows without `tif`, such as `triggered`
  rows, with an empty `tif`.
- `L4OrderEntry` gains `timestamp` (RFC 3339) and `timestamp_ms`, the resting
  order's queue time (`None` when unknown). Checkpoint history rows, which
  send the queue time as integer milliseconds, fill both fields the same
  way. `LiquidationLevels`, `LiquidationLevelsHistoryItem` and
  `TriggerLevelsHistoryItem` gain `snapshot_ts_ms`.
- `client.rh_lighter`, a client for Lighter on Robinhood Chain
  (`/v1/rh-lighter`), the second Lighter deployment. It has the same
  resources as `client.lighter` except the L3 order book: `orderbook`,
  `trades`, `instruments`, `funding`, `open_interest`, `candles`,
  `liquidations`, `positions`, `freshness()`, `summary()` and
  `price_history()`. Markets are quoted in USDG, with uppercase perp symbols
  (`BTC`) and dashed spot pairs (`AAPL-USDG`). Trades and liquidations are
  served from the venue launch on 2026-06-26 20:10:26 UTC; order book, open
  interest and funding from 2026-08-22 18:43 UTC; candles from
  2026-06-26 20:10 UTC. `trades.list()` is
  final up to the finalization boundary and `trades.recent()` is the
  preliminary tier, as on mainnet.
- `liquidations` on both Lighter clients (`LighterLiquidationsResource`), with
  `history()` returning `LighterLiquidation` rows and `volume()` returning
  `LighterLiquidationVolume` buckets, each with `timestamp` as an RFC 3339
  UTC string and `timestamp_ms`. Mainnet history starts on 2026-06-10
  and Robinhood Chain history at the venue launch, 2026-06-26 20:10:26 UTC.
  On Robinhood Chain, rows from before live capture were backfilled from the
  venue's finalized export and have `source` `"bucket"` and an empty
  `raw_json`; rows captured live have `source` `"ws"` and the venue's raw
  JSON.
- `TradesResource::list_with_meta()` and `TradesResource::recent_with_meta()`,
  which return the same rows as `list()` and `recent()` as a
  `MetaResponse<Vec<Trade>>` with the full response `meta` (`list()` returns
  the same value, since every paged response carries `meta`). On both
  Lighter deployments that includes the finalization boundary
  (`meta.finalized_through`), the clamp of a range that reached past it
  (`meta.requested_end`, `meta.clamped_to`) and, on `recent_with_meta()`,
  `meta.preliminary_row_count`.
- Account positions: a `positions` resource on `client.hyperliquid`,
  `client.hyperliquid.hip3` (keyed by `0x` wallet address), `client.lighter`
  and `client.rh_lighter` (keyed by integer account index), with `get()`
  (current, or as of a `timestamp`), `history()`, `changes()`, `market()`,
  `market_summary()`, `all()`, `account()` and `account_history()`.
  `account()` is the clearinghouse summary on Hyperliquid and HIP-3
  (`account(address, dex)`) and the account's position aggregates (totals,
  long/short value, position count) on Lighter and Robinhood Chain
  (`account(account_index)`). `client.lighter.accounts.by_l1()`
  finds the Lighter account indices of an L1 address (mainnet only). Lighter
  market symbols are case-insensitive: the positions methods send them
  uppercase.
- `client.data_quality.positions_freshness()`: one `PositionsFreshness` row
  per venue with the latest live and hourly snapshots, the live snapshot's
  age and quality, `stale`, `built_through` and `finalized_through`.
- `From<chrono::NaiveDateTime>` and `From<chrono::NaiveDate>` for
  `Timestamp`, converted as UTC.
- Typed models `Position`, `MarketPosition`, `PositionChange`,
  `AccountSummary`, `MarketPositionsSummary`, `WalletPositions`,
  `LighterL1Accounts` and `LighterL1Account`, and parameter structs
  `GetPositionsParams`, `PositionRangeParams`, `AccountHistoryParams`,
  `MarketPositionsParams`, `MarketSummaryParams` and `BulkPositionsParams`.
- `MetaResponse<T>` and `ResponseMeta`, returned by the positions methods and
  the `*_with_meta` trades methods, expose the full response `meta`:
  `as_of`, `snapshot_ts`, `source`, `quality`, `stale`, `built_through`,
  `finalized_through`, `requested_end`, `clamped_to`,
  `preliminary_row_count`, `totals`, `notice` and `coverage_from`.
  `ResponseMeta::position_totals()` decodes the totals of a market listing.
- WebSocket: live subscriptions for `rh_lighter_orderbook`,
  `rh_lighter_trades`, `rh_lighter_open_interest` and `rh_lighter_funding`,
  served on `wss://api.0xarchive.io/ws`, and replay for those four and
  `rh_lighter_candles`. Live payloads have the mainnet Lighter shapes and
  decode with `ServerMsg::lighter_live_data()` and `LighterLiveData::decode()`.
  Replay rows of the book, trades, open interest and funding channels, on
  both deployments, now have the live shapes too and decode the same way
  (`historical_data` and `replay_snapshot`); a replayed trades row is one
  fill.
  `subscribe_with_interval()` accepts `rh_lighter_orderbook` (100 to 5000 ms).
- `RH_LIGHTER_REPLAY_CHANNELS`, `RH_LIGHTER_LIVE_CHANNELS`,
  `RH_LIGHTER_REPLAY_ONLY_CHANNELS`, `RH_LIGHTER_SUBSCRIPTION_ERROR`,
  `is_rh_lighter_channel()`, `is_rh_lighter_live_channel()` and
  `is_rh_lighter_replay_only_channel()`.
- `OiFundingInterval::OneMinute` (`"1m"`). The API serves 1-minute
  buckets on funding, open interest and HIP-3 breadth history, the three
  params typed with `OiFundingInterval`. `OiFundingInterval` is not
  `#[non_exhaustive]`, so a `match` on it without a wildcard arm needs a
  `OneMinute` arm.
- `cursor` on `OrderFlowParams` and `Hip4OrderFlowParams`, sent by
  `orders.flow()` (Hyperliquid and HIP-3) and `hip4.get_order_flow()`. The
  API pages order flow: a page holds the oldest `limit` buckets of the
  window, and `next_cursor` is set while more may follow. Pass it back as
  `cursor` with the same `start`, `end` and `interval` until it is `None`.
  Both structs are built with struct literals, so a literal that lists
  every field needs `cursor: None` (or `..Default::default()`).
- Webhooks: `client.webhooks` (`WebhooksResource`) covers all 21 routes
  under `/v1/webhooks`: `event_types()`, `limits()`, `list_endpoints()`,
  `create_endpoint()`, `delete_endpoint()`, `enable_endpoint()`,
  `rotate_secret()`, `test_endpoint()`, `list_deliveries()`, `redeliver()`,
  `list_subscriptions()`, `create_subscription()`, `update_subscription()`,
  `delete_subscription()`, `resume_subscription()`,
  `resume_all_subscriptions()`, `dry_run()`, `estimate()`,
  `list_addresses()`, `add_address()` and `delete_address()`. Parameter
  structs `CreateEndpointParams`, `CreateSubscriptionParams`,
  `UpdateSubscriptionParams`, `DryRunParams` and `EstimateParams`, and typed
  models for the catalog, limits, endpoints, deliveries, subscriptions (with
  their pause state), resume gaps, previews and watched wallets.
  `WebhookSubscriptionConfig` and `WebhookSubscriptionCondition` build a
  rule's config. Webhook delivery starts on the Build plan; the estimate and
  the dry-run answer on every plan.
- `webhook_signature`: `WebhookVerifier` verifies a delivery from the raw
  body bytes and the `0xa-signature` header. It checks HMAC-SHA256 over
  `<t>.<body>` in constant time, enforces a replay window (300 seconds by
  default, `tolerance_secs()` to change it), and accepts a delivery when any
  `v1` in the header matches any secret it holds, so a receiver keeps
  verifying through the 24 hours after a secret rotation.
  `parse_signature_header()` returns the parsed parts. The verifier's `Debug`
  output never includes a secret.
- `client.hyperliquid.cvd()` and `client.hyperliquid.hip3.cvd()`: cumulative
  volume delta buckets (`CvdBucket`: open time as `timestamp` (RFC 3339) and
  `timestamp_ms`, taker buy and sell notional, `delta`, `cumulative_delta`)
  at `1m` to `1w`, cursor-paged with `CvdParams`. They
  return a `MetaResponse`, whose `meta.notice` says when a response is one
  page of several; `cumulative_delta` restarts on every page.
- `client.symbols()`: the public symbol universe (`GET /v1/symbols`), one
  `SymbolEntry` per market with its venue family, data types, coverage
  dates by data type and estimated size per day, plus the slug, side pair,
  title and settlement state on HIP-4 entries.
- `client.hyperliquid.breadth`: breadth above session VWAP for Hyperliquid
  core perps (`current()` and `history()`, from 2026-08-24), on the same
  `BreadthResource` as `client.hyperliquid.hip3.breadth`. Core snapshots are
  aggregate only, with empty `namespaces` maps. `BreadthSnapshot` is a
  venue-neutral alias of `Hip3BreadthSnapshot`.
- `client.hyperliquid.hip3.oracle`: `external_price()` returns the latest
  deployer-pushed external price and mark price
  (`Hip3OracleExternalPrice`), and `discovery_bounds()` the instantaneous
  discovery bounds (`Hip3OracleDiscoveryBounds`), both with the source time
  as `timestamp` (RFC 3339) and `timestamp_ms`.
- `client.hyperliquid.hip4.list_questions()` and `get_question()`: HIP-4
  questions (`Hip4Question`), which group binary outcomes under one ballot.
  `list_questions()` is cursor-paged with `Hip4ListQuestionsParams`.
- `client.hyperliquid.wallets.classify()` and
  `client.hyperliquid.hip3.wallets.classify()`: precomputed daily behavior
  metrics for active wallets (`WalletClassification`, `ClassifiedWallet`,
  `WalletMetrics`), filtered, sorted and offset-paged with
  `WalletClassifyParams`.
- WebSocket: `orderbook_full` and `hip3_orderbook_full`, the full-depth L2
  order books, are documented in the README channel table. Their frames
  decode as `ServerMsg::L4Snapshot` (the whole book) and `ServerMsg::L4Batch`
  (price-level changes), live and in replay.
- `client.lighter.l3_orderbook.get_with_params()` with `L3OrderBookParams`:
  a snapshot at `timestamp`, filtered to one `account` index, with `depth`.
  `L3HistoryParams` gains `account`; it is built with a struct literal, so a
  literal that lists every field needs `account: None`.
- `client.data_quality.list_incidents_with()` filters incidents by
  `status`, `exchange` and `since` and pages them with `limit` and `offset`
  (`ListIncidentsParams`); `IncidentsResponse` gains `pagination` (`total`,
  `limit`, `offset`). `client.data_quality.symbol_coverage_with()` sets the
  gap-detection window with `from` and `to` (`SymbolCoverageParams`).

### Changed
- New dependencies `hmac` and `sha2`, used only by `webhook_signature`.
- `GetTradesParams::side` and `Hip4TradesParams::side` are
  `Option<TradeSide>` (`buy` or `sell`), which the API now applies, in
  place of the `"A"` / `"B"` strings it ignored. A literal with
  `side: None` is unchanged.
- `depth` is no longer offered on `L4HistoryParams` and
  `Hip4L4HistoryParams`: L4 history returns whole snapshots. Remove it from
  struct literals.
- `CursorResponse<T>` is now the same type as `MetaResponse<T>` and gains
  `has_more` and `meta`. A struct literal or a pattern that names every
  field needs the two new fields (or `..`).
- `Error::Api` gains `error_code`, `param` and `valid_values`, and
  `ServerMsg::Error` gains `error_code`. A pattern that names every field,
  such as `ServerMsg::Error { message }`, needs the new fields or `..`.
- `ServerMsg::lighter_live_data()` also decodes Lighter replay rows, which
  arrive in the live shapes.
- `replay()` no longer rejects the HIP-3, Spot and HIP-4 L4 channels or the
  full-depth channels, which the API now replays.
- `client.symbols()` and the data quality methods read the standard
  envelope, and still read the older bodies.
- Documentation and messages name the venue "Lighter".
- `client.hyperliquid.spot.orders` is a `SpotOrdersResource` whose
  `history()` takes `SpotOrderHistoryParams` (`start`, `end`, `cursor`,
  `limit`). The Spot route does not filter by user, status or order type,
  so those fields are gone for Spot.
- `client.hyperliquid.hip3.liquidations` is a `Hip3LiquidationsResource`,
  with the same `history()`, `volume()`, `levels()` and `levels_history()`
  as the Hyperliquid resource.
- `subscribe()` rejects `rh_lighter_candles` before sending, since it is
  replay-only.
- Install snippets and rustdoc examples reference `1.12`.
- Minimum supported Rust version is 1.85 (was 1.75). A fresh dependency
  resolution now includes crates published with edition 2024, which Cargo
  reads from 1.85 on.
- `OrderFlowParams::interval` and `Hip4OrderFlowParams::interval` document
  the buckets the API serves: `"1m"` (the default), `"5m"`, `"15m"` and
  `"1h"`.

### Deprecated
- `LIVE_ONLY_L4_CHANNELS` and `is_live_only_l4_channel()`: every L4 channel
  now replays, and `is_live_only_l4_channel()` always returns `false`. Use
  `L4_REPLAY_CHANNELS` and `is_l4_channel()`.
- `SpotPair::mark_price`, `mid_price`, `latest_timestamp` and `is_active`.
  The pairs routes do not return them, so they are always `None`. Read
  prices from the pair's order book or trades.

### Removed
- `client.hyperliquid.hip3.liquidations.by_user()`, and `flow()`, `tpsl()`,
  `trigger_levels()` and `trigger_levels_history()` on
  `client.hyperliquid.spot.orders`. They called routes the API does not
  serve and always failed. `client.hyperliquid.liquidations.by_user()`
  remains.

### Fixed
- A WebSocket replay requested with `speed: None` got no answer: the
  request carried `"speed": null`, which the server does not accept.
  `replay()` and `replay_multi()` now leave an unset `speed` or `end` out of
  the request, so the server applies its defaults (1x, and up to now). This
  matters most for the L4 and full-depth channels, which ignore `speed`.
- `SpotPair::base`, `quote`, `wire_symbol` and `spot_index` were always
  `None` on `spot.pairs.list()` and `spot.pairs.get()`: the pairs routes send
  them as `base_token_name`, `quote_token_name`, `name` and `pair_index`.
  Both spellings now deserialize. The other registry fields (such as
  `is_canonical` and the token decimals) stay in `extra`.
- `orderbook.collect_tick_history()` returned only the first page of a
  range: it stopped on a page of fewer than 1,000 deltas, while tick pages
  hold 100 by default, and it advanced by time, which reopened the same
  checkpoint. It now follows the response's cursor (`cursor` and
  `cursor_seq`) and applies every delta until `has_more` is `false`. The
  README's manual pagination example does the same with
  `history_tick_page()`.
- `spot.twap.by_symbol()` and `by_user()` failed to decode rows whose
  `executed_size` or `executed_notional` arrive as numbers. Both fields
  accept a number or a string and keep the decimal string.
- `data_quality.symbol_coverage()` and `exchange_coverage()` percent-encode
  their path segments and keep the symbol's case, so `km:US500`,
  `HYPE-USDC` and HIP-4 `#0` reach the right route. A `#0` was cut off as a
  URL fragment before.
- A `Timestamp` string without a time zone is UTC. Before, only RFC 3339
  strings with an offset were read; a date alone (`"2026-09-01"`) or a
  date-time without an offset (`"2026-09-01T00:00:00"`) silently became
  `0`. Both now convert to the UTC instant, and a string of digits is still
  Unix milliseconds.
- `freshness()` on every client (and `hip4.get_freshness()`) failed with a
  deserialization error: the response carries a `symbol` string next to the
  per-data-type entries, and the flattened `data_types` map tried to read it
  as one. `CoinFreshness` now has a `symbol: Option<String>` field, and
  `data_types` holds only the data types.

### Documentation
- The README's coverage table gives each dataset's first available instant
  on every venue, as `GET /v1/capabilities` returns it, in place of months
  (such as "April 2023+" and "February 2026+") and fixed market counts. The
  Robinhood Chain candle start is 2026-06-26 20:10 UTC.
- README, rustdoc and example code read the most recent minutes, hours or
  days instead of fixed dates, so every snippet works on a Free key's
  rolling 30-day window, and uses markets that are trading (`xyz:XYZ100` on
  HIP-3, the open daily BTC outcome on HIP-4). The README's Robinhood Chain
  trades snippet sets `side`, and the HIP-3 instruments snippet prints
  `namespace` and `ticker` with `{:?}`; both now compile. The WebSocket
  replay examples use 10x, which every plan allows.
- The README's WebSocket channel table matches `GET /v1/capabilities`:
  `spot_twap` is not a WebSocket channel (Spot TWAP is REST only), and
  `hip4_orderbook` and `hip4_open_interest` replay but do not stream live.
  The Enterprise replay speed is 1000x.
- Recent trades are documented for HIP-3, HIP-4, Spot, Lighter and Lighter
  on Robinhood Chain.
- `examples/levels_smoke.rs` describes each call, and
  `examples/websocket.rs` no longer subscribes to a settled HIP-4 market.
- Documentation links point at docs.0xarchive.io.
- docs.rs builds the documentation with every feature, so the WebSocket
  client (`oxarchive::ws`, behind the `websocket` feature) is documented
  there.

## [1.11.0] - 2026-09-25

Versions 1.9.0, 1.9.1 and 1.10.0 were not published to crates.io. This
release includes their changes, listed in the sections below.

### Added
- Live WebSocket subscriptions for four Lighter.xyz channels:
  `lighter_orderbook`, `lighter_trades`, `lighter_open_interest` and
  `lighter_funding`, served on `wss://api.0xarchive.io/ws`. `subscribe()`
  now sends these requests instead of rejecting them. They use the same
  envelope as Hyperliquid live data and the same symbols as
  `client.lighter.instruments.list()`.
- `OxArchiveWs::subscribe_with_interval()` and
  `ClientMsg::SubscribeWithInterval` set `interval_ms` on
  `lighter_orderbook` (100 to 5000). Without it the server sends one book per
  second. Other channels and out-of-range values are rejected before a
  request is sent.
- Typed live payloads: `LighterLiveOrderBook` (full top-20 book per message,
  with `bids()` and `asks()`), `LighterLiveTrade` (two fills per trade that
  share `tid`), and `LighterLiveMarketStats` / `LighterLiveAssetCtx` for the
  open-interest and funding channels, which carry the same message. Decode a
  message with `ServerMsg::lighter_live_data()` or `LighterLiveData::decode()`.
- `LIGHTER_LIVE_CHANNELS`, `LIGHTER_REPLAY_ONLY_CHANNELS`,
  `is_lighter_live_channel()` and `is_lighter_replay_only_channel()`.

### Changed
- `lighter_candles` and `lighter_l3_orderbook` remain replay-only.
  `subscribe()` still rejects them before sending, and
  `LIGHTER_SUBSCRIPTION_ERROR` now names these two channels.
- `is_lighter_replay_channel()` still returns `true` for all six Lighter
  channels, since all six support replay, but no longer means that live
  subscription is unavailable. Use `is_lighter_replay_only_channel()` for that.
- Lighter replay is unchanged. Replay rows keep their existing shapes, which
  differ from the live payloads.
- Install snippets and rustdoc examples reference `1.11`.
- `examples/websocket.rs` no longer includes a bulk streaming section, and the
  README no longer describes bulk download or a per-tier batch size.

### Deprecated
- `OxArchiveWs::stream()` and `OxArchiveWs::stream_stop()`. The server has
  discontinued bulk streaming over WebSocket and now answers these requests
  with an error message instead of data. Both methods still compile and send
  the request, so existing code keeps building, with a deprecation warning.
  For large historical downloads, use the S3 Parquet bulk export at
  https://www.0xarchive.io/data. For bounded windows over WebSocket, use
  `replay()`. The `ServerMsg::Stream*` variants are no longer sent.

### Fixed
- The README WebSocket channel table now lists Hyperliquid `open_interest`
  and `funding` as available for live subscription as well as replay.

## [1.10.0] - 2026-09-23

Versions 1.9.0 and 1.9.1 were not published to crates.io. This release
includes their changes, listed in the sections below, and aligns the Rust,
TypeScript and Python SDKs on one version.

### Changed
- `Trade::fee`, `closed_pnl` and `start_position` are now returned as `"0"`
  when the venue recorded a zero, instead of being omitted. `None` now means
  the source did not record the value (for example fills from 2025-03-22 to
  2025-05-25), never zero. This is a server-side change and applies to every
  SDK version.
- HIP-3 and HIP-4 trades now include `fee`, `fee_token`, `closed_pnl` and
  `start_position`.
- Install snippets and rustdoc examples reference `1.10`.

## [1.9.1] - 2026-08-31

### Added
- Typed HIP-3 breadth above current UTC-session VWAP through
  `client.hyperliquid.hip3.breadth.current()` and cursor-paginated
  `history(...)`. Recorded history begins on 2026-08-28.

### Changed
- Entitlement copy: Free history is a rolling 30-day window (30-day span per
  request or replay); Build and above keep the full retained archive. Route
  families, schemas, and served depth remain available on every tier.
- Correct Lighter per-fill trade history to the observed global floor of January 17, 2025; exact starts vary by market. This supersedes the August floor documented in the earlier release notes below.
- Lighter WebSocket channels are explicitly replay-only: current data remains
  available through REST, all six channels remain available for bounded
  historical replay, and live subscription requests fail fast with REST/replay
  guidance.
- Hyperliquid core `l4_diffs` and `l4_orders` historical replay now documents
  its initial `l4_snapshot` followed by ordered `l4_batch` pages; HIP-3,
  HIP-4, and Spot L4 remain live-only.
- Projected forced-liquidation price-level guidance now reflects an
  approximately five-minute refresh cadence.
- Lighter `funding_rate` values are fractional and non-annualized. Consumers
  that compensated for the former raw percent representation must update.

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
  computed from clearinghouse positions and margin state (approximately
  five-minute snapshots, `at` point-in-time reads, `side` filter,
  cursor-paginated history with `summary` mode). History retained from
  2026-07-27.
- **Trigger levels**: `orders.trigger_levels()` and
  `trigger_levels_history()`: the pending stop-loss / take-profit map
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
  `/liquidations/user/{address}`; the server treated the wallet as a coin
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
