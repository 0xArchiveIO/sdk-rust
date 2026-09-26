/// Type definitions for all 0xArchive API responses.
///
/// The API returns `snake_case` JSON which matches Rust's native field
/// naming convention, so no `rename_all` attribute is needed.
use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize a value that may arrive as a JSON number or a JSON string,
/// always storing it as a `String`. This preserves decimal precision when the
/// API quotes the value, while still accepting bare floats.
fn deserialize_number_or_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct NumberOrString;

    impl serde::de::Visitor<'_> for NumberOrString {
        type Value = String;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number or string")
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<String, E> {
            Ok(v.to_owned())
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<String, E> {
            Ok(v)
        }
    }

    deserializer.deserialize_any(NumberOrString)
}

/// Option-aware variant of [`deserialize_number_or_string`]: `null`/absent
/// stays `None`; numbers and strings both become `Some(String)`.
fn deserialize_opt_number_or_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrap(#[serde(deserialize_with = "deserialize_number_or_string")] String);

    let opt = Option::<Wrap>::deserialize(deserializer)?;
    Ok(opt.map(|w| w.0))
}

// ---------------------------------------------------------------------------
// Generic response envelope
// ---------------------------------------------------------------------------

/// Metadata returned with every API response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMeta {
    pub count: usize,
    pub request_id: String,
    pub next_cursor: Option<String>,
    /// Coverage start date (ISO 8601), present when the requested window ends
    /// before the symbol's coverage begins.
    pub coverage_from: Option<String>,
    /// Advisory notice explaining an empty response (e.g. window predates coverage).
    pub notice: Option<String>,
}

/// Raw API response envelope (internal use).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ApiEnvelope<T> {
    pub data: T,
    pub meta: Option<ApiMeta>,
}

/// A paginated response containing data and an optional cursor for the next page.
#[derive(Debug, Clone)]
pub struct CursorResponse<T> {
    /// The response data.
    pub data: T,
    /// Pass this value as the `cursor` parameter to fetch the next page.
    /// `None` means there are no more pages.
    pub next_cursor: Option<String>,
}

/// The full `meta` block of a response.
///
/// Returned by the methods that give snapshot or finalization context with
/// their data, such as the account positions resources. Every field is
/// optional on the wire and is `None` (or `0` / empty for `count` and
/// `request_id`) when the server did not send it. Instants are RFC 3339 UTC
/// strings. The positions routes and the trades finalization fields always
/// send milliseconds (`2026-09-25T00:00:00.000Z`), while instants in data
/// rows carry a fraction only when it is not zero (`2026-09-25T00:00:00Z`),
/// so parse both before comparing them.
///
/// New fields may be added in minor releases, so the struct cannot be built
/// with a literal outside this crate; start from `ResponseMeta::default()`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseMeta {
    /// Number of rows in `data`.
    #[serde(default)]
    pub count: usize,
    /// Request identifier, useful when contacting support.
    #[serde(default)]
    pub request_id: String,
    /// Pass this value as the `cursor` parameter to fetch the next page.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// Instant the returned state describes: the snapshot tick, the hour, or
    /// the requested as-of time. Taken from the data, never the request time.
    #[serde(default)]
    pub as_of: Option<String>,
    /// Committed snapshot the rows were read from. Market routes echo the
    /// resolved `hour` here.
    #[serde(default)]
    pub snapshot_ts: Option<String>,
    /// How the rows were produced: `snapshot`, `reconstructed` or `changes`.
    #[serde(default)]
    pub source: Option<String>,
    /// Completeness of the snapshot the response was read from: `complete`,
    /// `partial` or `degraded`. Each row also carries its own `quality`.
    #[serde(default)]
    pub quality: Option<String>,
    /// `true` when the latest live snapshot is older than 12 minutes. Paired
    /// with `notice`.
    #[serde(default)]
    pub stale: Option<bool>,
    /// Every event before this instant is built into the position change log
    /// and the as-of state. Reads are clamped to it.
    #[serde(default)]
    pub built_through: Option<String>,
    /// Every event before this instant is final and will not change. Data
    /// after it is preliminary.
    #[serde(default)]
    pub finalized_through: Option<String>,
    /// The `end` (or `timestamp`) you asked for, set only when it was clamped.
    #[serde(default)]
    pub requested_end: Option<String>,
    /// The boundary the request was clamped to, set only when it was clamped.
    /// On positions routes this is `built_through`; on Lighter trades it is
    /// `finalized_through`.
    #[serde(default)]
    pub clamped_to: Option<String>,
    /// Number of preliminary (not yet final) rows in this response.
    #[serde(default)]
    pub preliminary_row_count: Option<usize>,
    /// Totals over the whole filtered result set, not just this page. Sent on
    /// the first page of market position listings; read them typed with
    /// [`ResponseMeta::position_totals`].
    #[serde(default)]
    pub totals: Option<serde_json::Value>,
    /// Advisory about the response, for example that the requested time is
    /// before coverage begins or that the live snapshot is stale.
    #[serde(default)]
    pub notice: Option<String>,
    /// Coverage start for the requested data, sent with `notice`.
    #[serde(default)]
    pub coverage_from: Option<String>,
}

impl ResponseMeta {
    /// Decode `totals` of a market position listing.
    ///
    /// Returns `None` when `totals` is absent (every page after the first) or
    /// does not have the market summary shape.
    pub fn position_totals(&self) -> Option<MarketPositionsSummary> {
        self.totals
            .as_ref()
            .and_then(|t| MarketPositionsSummary::deserialize(t).ok())
    }
}

/// Response envelope that keeps the whole `meta` block (internal use).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MetaEnvelope<T> {
    pub data: T,
    #[serde(default)]
    pub meta: Option<ResponseMeta>,
}

/// A response with its data, the cursor for the next page, and the full
/// `meta` block.
#[derive(Debug, Clone)]
pub struct MetaResponse<T> {
    /// The response data.
    pub data: T,
    /// Pass this value as the `cursor` parameter to fetch the next page.
    /// `None` means there are no more pages.
    pub next_cursor: Option<String>,
    /// Snapshot context, finalization boundaries and notices.
    pub meta: ResponseMeta,
}

impl<T> MetaResponse<T> {
    pub(crate) fn new(data: T, meta: ResponseMeta) -> Self {
        Self {
            data,
            next_cursor: meta.next_cursor.clone(),
            meta,
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

/// A flexible timestamp that can be specified as Unix milliseconds, an ISO-8601
/// string, or a `chrono::DateTime`.
#[derive(Debug, Clone)]
pub enum Timestamp {
    Millis(i64),
    Iso(String),
    DateTime(chrono::DateTime<chrono::Utc>),
}

impl Timestamp {
    /// Convert to Unix milliseconds for use in query parameters.
    pub fn to_millis(&self) -> i64 {
        match self {
            Timestamp::Millis(ms) => *ms,
            Timestamp::DateTime(dt) => dt.timestamp_millis(),
            Timestamp::Iso(s) => chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|_| s.parse::<i64>().unwrap_or(0)),
        }
    }
}

impl From<i64> for Timestamp {
    fn from(ms: i64) -> Self {
        Timestamp::Millis(ms)
    }
}

impl From<&str> for Timestamp {
    fn from(s: &str) -> Self {
        Timestamp::Iso(s.to_string())
    }
}

impl From<String> for Timestamp {
    fn from(s: String) -> Self {
        Timestamp::Iso(s)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        Timestamp::DateTime(dt)
    }
}

// ---------------------------------------------------------------------------
// Orderbook
// ---------------------------------------------------------------------------

/// A single price level in an order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price as a decimal string.
    pub px: String,
    /// Size as a decimal string.
    pub sz: String,
    /// Number of orders at this level.
    pub n: i64,
}

/// L2 order book snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub coin: String,
    pub timestamp: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub mid_price: Option<String>,
    pub spread: Option<String>,
    pub spread_bps: Option<String>,
}

// ---------------------------------------------------------------------------
// Trades
// ---------------------------------------------------------------------------

/// A single trade (fill) record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub coin: String,
    /// `"A"` (ask/sell) or `"B"` (bid/buy) — taker side.
    pub side: String,
    pub price: String,
    pub size: String,
    pub timestamp: String,
    pub tx_hash: Option<String>,
    pub trade_id: Option<i64>,
    pub order_id: Option<i64>,
    /// `true` for taker (crossed the spread), `false` for maker.
    pub crossed: Option<bool>,
    /// Fee paid on this fill in `fee_token`, including any builder fee; negative is a rebate.
    /// `"0"` is a recorded zero fee. `None` when the source did not record fees, for example
    /// fills from 2025-03-22 to 2025-05-25.
    pub fee: Option<String>,
    /// Fee denomination (e.g. USDC). Present exactly when `fee` and `closed_pnl` were recorded.
    pub fee_token: Option<String>,
    /// Realized PnL on this fill. `"0"` when the fill opened or added to a position. `None` when
    /// the source did not record it (same cases as `fee`).
    pub closed_pnl: Option<String>,
    pub direction: Option<String>,
    /// Position size (spot: balance) before this fill; negative is short. `"0"` means flat.
    /// `None` when the source did not record it.
    pub start_position: Option<String>,
    pub user_address: Option<String>,
    pub maker_address: Option<String>,
    pub taker_address: Option<String>,
    /// Builder address that routed this order. Present only when the order was placed through a builder.
    pub builder_address: Option<String>,
    /// Builder fee charged on this fill, paid to the builder (in quote currency, typically USDC).
    /// Present only when `builder_address` is set.
    pub builder_fee: Option<String>,
    /// HIP-3 deployer fee share on this fill (in quote currency). Negative for the maker side (rebate),
    /// positive for the taker side. Present only on HIP-3 fills.
    pub deployer_fee: Option<String>,
    /// Priority fee burned in HYPE (not USDC) for write priority on the Hyperliquid validator queue.
    /// Independent of `builder_fee` and `deployer_fee` — paid to the network, not to a builder or
    /// deployer. Present only when the order paid for priority.
    pub priority_gas: Option<f64>,
    /// Client order ID.
    pub cloid: Option<String>,
    /// TWAP execution ID.
    pub twap_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------

/// A Hyperliquid perpetual instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub sz_decimals: i32,
    pub max_leverage: Option<i32>,
    pub only_isolated: Option<bool>,
    pub instrument_type: Option<String>,
    pub is_active: bool,
}

/// A Lighter.xyz instrument with fee and precision metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterInstrument {
    pub symbol: String,
    pub market_id: i64,
    pub market_type: Option<String>,
    pub status: Option<String>,
    pub taker_fee: Option<f64>,
    pub maker_fee: Option<f64>,
    pub liquidation_fee: Option<f64>,
    pub min_base_amount: Option<f64>,
    pub min_quote_amount: Option<f64>,
    pub size_decimals: Option<i32>,
    pub price_decimals: Option<i32>,
    pub quote_decimals: Option<i32>,
    pub is_active: Option<bool>,
}

/// A HIP-3 builder-deployed perp instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip3Instrument {
    /// Case-sensitive symbol, e.g. `km:US500`.
    pub coin: String,
    pub namespace: Option<String>,
    pub ticker: Option<String>,
    pub mark_price: Option<f64>,
    pub open_interest: Option<f64>,
    pub mid_price: Option<f64>,
    pub latest_timestamp: Option<String>,
}

// ---------------------------------------------------------------------------
// Hyperliquid Spot
// ---------------------------------------------------------------------------

/// A Hyperliquid Spot trading pair (e.g. `HYPE-USDC`, `PURR-USDC`).
///
/// Symbols are dashed canonical. The server resolves the dashed form to the
/// wire format (`PURR/USDC`, `@107`) internally. Spot pairs have no funding
/// rate, no open interest, and no liquidations: those are perp-only
/// constructs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotPair {
    /// Dashed canonical symbol (e.g. `HYPE-USDC`).
    pub symbol: String,
    /// Base asset (e.g. `HYPE`).
    pub base: Option<String>,
    /// Quote asset (e.g. `USDC`).
    pub quote: Option<String>,
    /// Hyperliquid wire-format pair (e.g. `PURR/USDC` or `@107`).
    pub wire_symbol: Option<String>,
    /// Hyperliquid spot index (the `@N` form), when applicable.
    pub spot_index: Option<i64>,
    pub mark_price: Option<f64>,
    pub mid_price: Option<f64>,
    pub latest_timestamp: Option<String>,
    pub is_active: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// A Hyperliquid Spot TWAP execution status record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotTwapStatus {
    pub coin: String,
    pub timestamp: String,
    pub twap_id: i64,
    pub user_address: Option<String>,
    pub side: Option<String>,
    pub status: Option<String>,
    pub executed_size: Option<String>,
    pub executed_notional: Option<String>,
    pub minutes: Option<i64>,
    pub randomize: Option<bool>,
    pub reduce_only: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HIP-4 (outcome markets)
// ---------------------------------------------------------------------------

/// A single side of a HIP-4 outcome market, returned by `/instruments`.
///
/// Each market has two per-side rows (e.g. `#0` Yes, `#1` No). For the
/// per-outcome aggregate (both sides combined plus `aggregated_oi`), see
/// [`Hip4OutcomeAggregate`].
///
/// Response coin format: `#<10*outcome_id + side>`. For path inputs, use the
/// bare numeric form (`"0"`) in new code. Legacy `"#0"` input remains
/// supported and is percent-encoded only for URL transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip4Outcome {
    pub outcome_id: i64,
    pub side: i32,
    pub asset_id: i64,
    pub coin: String,
    pub symbol: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub side_name: Option<String>,
    pub recurring_class: Option<String>,
    pub recurring_underlying: Option<String>,
    pub recurring_expiry: Option<String>,
    pub recurring_target_px: Option<f64>,
    pub recurring_period: Option<String>,
    pub builder_address: Option<String>,
    pub is_settled: Option<bool>,
    pub settlement_value: Option<f64>,
    pub settlement_at: Option<String>,
    pub first_seen_at: Option<String>,
    pub last_updated_at: Option<String>,
    /// Per-side human-readable title, deterministic from parsed metadata.
    /// e.g. `"BTC above 78,213 on May 4 at 06:00 UTC? . Yes"`.
    pub display_title: Option<String>,
    /// Per-side URL slug mirroring Hyperliquid's pattern.
    /// e.g. `"btc-above-78213-yes-may-04-0600"`.
    pub slug: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Per-side specification embedded in [`Hip4OutcomeAggregate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip4SideSpec {
    pub side: i32,
    pub name: Option<String>,
    pub coin: String,
    pub asset_id: i64,
    /// Per-side human-readable title (e.g. `"BTC above 78,213 on May 4 at 06:00 UTC? . Yes"`).
    pub display_title: Option<String>,
    /// Per-side URL slug mirroring HL's pattern (e.g. `"btc-above-78213-yes-may-04-0600"`).
    pub slug: Option<String>,
}

/// Latest aggregated open-interest snapshot for a HIP-4 outcome (both sides).
///
/// Populated only on `/outcomes/{outcome_id}` (detail), omitted from the
/// `/outcomes` list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip4AggregatedOi {
    pub side0_open_interest_contracts: Option<f64>,
    pub side1_open_interest_contracts: Option<f64>,
    pub outcome_display_open_interest_contracts: Option<f64>,
    pub paired_set_supply_contracts: Option<f64>,
    pub side_supply_parity: Option<bool>,
    pub currency: Option<String>,
    pub as_of: Option<String>,
    pub side0_as_of: Option<String>,
    pub side1_as_of: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// A HIP-4 outcome market (per-outcome view, both sides combined).
///
/// Returned by `/outcomes` (list, no `aggregated_oi`) and `/outcomes/{id}`
/// (detail, with `aggregated_oi` populated). Also returned by
/// `/outcomes/by-slug/{slug}` and the `?slug=` filter on `/outcomes`.
///
/// `mark_price` (when present on related endpoints like
/// `Hip4OpenInterestRecord`) is an implied probability in `[0, 1]`, not a
/// USD price. The field name matches the perp/HIP-3 convention because the
/// Hyperliquid upstream uses `markPx` for both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip4OutcomeAggregate {
    pub outcome_id: i64,
    pub name: Option<String>,
    pub description_raw: Option<String>,
    pub class: Option<String>,
    pub underlying: Option<String>,
    pub expiry: Option<String>,
    pub target_price: Option<f64>,
    pub period: Option<String>,
    #[serde(default)]
    pub side_specs: Vec<Hip4SideSpec>,
    pub is_settled: Option<bool>,
    pub status: Option<String>,
    pub source_seen_at: Option<String>,
    /// Outcome-level human-readable title (no side suffix).
    /// e.g. `"BTC above 78,213 on May 4 at 06:00 UTC?"`.
    pub display_title: Option<String>,
    /// Outcome-level URL slug (no side word, no leading `#`).
    /// e.g. `"btc-above-78213-may-04-0600"`.
    pub slug: Option<String>,
    /// Pair of side coins for this outcome, e.g. `["#0", "#1"]`. Surfaced
    /// on `/v1/symbols` HIP-4 rows; included here for symmetry.
    pub outcome_pair: Option<[String; 2]>,
    pub aggregated_oi: Option<Hip4AggregatedOi>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// HIP-4 open-interest record (mirrors HIP-3 OI plus `outcome_id` and `side`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip4OpenInterestRecord {
    pub coin: String,
    pub symbol: Option<String>,
    pub outcome_id: Option<i64>,
    pub side: Option<i32>,
    pub timestamp: String,
    pub open_interest: String,
    /// **Implied probability in `[0, 1]`, not a USD price.** The field name
    /// matches perp/HIP-3 because the Hyperliquid upstream uses `markPx` for
    /// both. To convert to a percentage, multiply by 100.
    pub mark_price: Option<String>,
    pub mid_price: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HIP-3 market breadth above session VWAP
// ---------------------------------------------------------------------------

/// Instrument counts for one HIP-3 breadth-above-session-VWAP snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip3BreadthCounts {
    pub candidates: i64,
    pub eligible: i64,
    pub above: i64,
    pub at: i64,
    pub below: i64,
    pub excluded_no_session_volume: i64,
    pub excluded_stale_price: i64,
}

/// Per-namespace counts for one HIP-3 breadth snapshot.
///
/// The API includes only namespaces with non-zero counts in each map. An empty
/// map is therefore meaningful and is not equivalent to missing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip3BreadthNamespaces {
    pub eligible: std::collections::HashMap<String, i64>,
    pub above: std::collections::HashMap<String, i64>,
    pub at: std::collections::HashMap<String, i64>,
    pub below: std::collections::HashMap<String, i64>,
}

/// A validated HIP-3 percentage-above-session-VWAP snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hip3BreadthSnapshot {
    /// UTC calendar date for the current session.
    pub session_date: String,
    /// Job timestamp; the newest included one-minute candle closed at this minute.
    pub calculated_at: String,
    /// `100 * above / eligible`; `None` when no instrument is eligible.
    pub value_pct: Option<f64>,
    /// `eligible / candidates`, in the range 0 to 1.
    pub coverage_ratio: f64,
    pub counts: Hip3BreadthCounts,
    pub namespaces: Hip3BreadthNamespaces,
}

// ---------------------------------------------------------------------------
// Funding rates
// ---------------------------------------------------------------------------

/// A funding rate snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub coin: String,
    pub timestamp: String,
    pub funding_rate: String,
    pub premium: Option<String>,
}

// ---------------------------------------------------------------------------
// Open interest
// ---------------------------------------------------------------------------

/// An open interest snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenInterest {
    pub coin: String,
    pub timestamp: String,
    pub open_interest: String,
    pub mark_price: Option<String>,
    pub oracle_price: Option<String>,
    pub day_ntl_volume: Option<String>,
    pub prev_day_price: Option<String>,
    pub mid_price: Option<String>,
    pub impact_bid_price: Option<String>,
    pub impact_ask_price: Option<String>,
}

// ---------------------------------------------------------------------------
// Candles
// ---------------------------------------------------------------------------

/// OHLCV candle (candlestick) data.
///
/// The wire serves OHLCV as JSON numbers; these fields accept both numbers
/// and strings and store the value as `String` to preserve precision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub open: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub high: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub low: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub close: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub volume: String,
    #[serde(default, deserialize_with = "deserialize_opt_number_or_string")]
    pub quote_volume: Option<String>,
    pub trade_count: Option<i64>,
}

/// Supported candle intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleInterval {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    FourHours,
    OneDay,
    OneWeek,
}

impl CandleInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            CandleInterval::OneMinute => "1m",
            CandleInterval::FiveMinutes => "5m",
            CandleInterval::FifteenMinutes => "15m",
            CandleInterval::ThirtyMinutes => "30m",
            CandleInterval::OneHour => "1h",
            CandleInterval::FourHours => "4h",
            CandleInterval::OneDay => "1d",
            CandleInterval::OneWeek => "1w",
        }
    }
}

// ---------------------------------------------------------------------------
// Liquidations
// ---------------------------------------------------------------------------

/// A single liquidation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Liquidation {
    pub coin: String,
    pub timestamp: String,
    pub liquidated_user: String,
    pub liquidator_user: Option<String>,
    pub price: String,
    pub size: String,
    pub side: String,
    pub mark_price: Option<String>,
    pub closed_pnl: Option<String>,
    pub direction: Option<String>,
    pub trade_id: Option<i64>,
    pub tx_hash: Option<String>,
}

/// Pre-aggregated liquidation volume for a time bucket.
///
/// USD fields use `String` to stay consistent with every other money/size
/// field in the SDK and avoid floating-point precision loss on large values.
/// The API may return these as bare JSON numbers or quoted strings depending
/// on the aggregation path, so a custom deserializer accepts both formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationVolume {
    pub coin: String,
    pub timestamp: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub total_usd: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub long_usd: String,
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub short_usd: String,
    pub count: i64,
    pub long_count: i64,
    pub short_count: i64,
}

// ---------------------------------------------------------------------------
// Lighter liquidations (mainnet and Robinhood Chain)
// ---------------------------------------------------------------------------

/// A single Lighter liquidation trade, from `client.lighter.liquidations` or
/// `client.rh_lighter.liquidations`.
///
/// Lighter liquidation rows carry both accounts of the trade rather than a
/// single liquidated user. `ask_account` and `bid_account` are Lighter
/// account indices as strings. A row backfilled from the venue's historical
/// export has `source` `"bucket"` and an empty `raw_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterLiquidation {
    /// Market symbol, uppercase for perps (`BTC`).
    pub symbol: String,
    /// Trade time in Unix milliseconds.
    pub timestamp: i64,
    /// Transaction time in microseconds, for ordering within a block.
    #[serde(default)]
    pub transaction_time_us: Option<i64>,
    pub trade_id: i64,
    /// Liquidation type as published by Lighter.
    #[serde(default)]
    pub liquidation_type: Option<String>,
    /// Price as a decimal string.
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub price: String,
    /// Size in base units as a decimal string.
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub size: String,
    /// Notional in the quote asset (USDC on mainnet, USDG on Robinhood Chain).
    #[serde(default, deserialize_with = "deserialize_opt_number_or_string")]
    pub usd_amount: Option<String>,
    /// Account index on the ask side of the trade.
    #[serde(default)]
    pub ask_account: Option<String>,
    /// Account index on the bid side of the trade.
    #[serde(default)]
    pub bid_account: Option<String>,
    #[serde(default)]
    pub ask_order_id: Option<i64>,
    #[serde(default)]
    pub bid_order_id: Option<i64>,
    /// `true` when the maker was on the ask side.
    #[serde(default)]
    pub is_maker_ask: Option<bool>,
    /// Taker's signed position before the trade.
    #[serde(default)]
    pub taker_position_size_before: Option<f64>,
    /// Maker's signed position before the trade.
    #[serde(default)]
    pub maker_position_size_before: Option<f64>,
    #[serde(default)]
    pub taker_entry_quote_before: Option<f64>,
    #[serde(default)]
    pub maker_entry_quote_before: Option<f64>,
    #[serde(default)]
    pub taker_initial_margin_fraction_before: Option<i64>,
    #[serde(default)]
    pub maker_initial_margin_fraction_before: Option<i64>,
    #[serde(default)]
    pub taker_allocated_margin_usdc_before: Option<i64>,
    #[serde(default)]
    pub taker_allocated_margin_usdc_after: Option<i64>,
    #[serde(default)]
    pub maker_allocated_margin_usdc_before: Option<i64>,
    #[serde(default)]
    pub maker_allocated_margin_usdc_after: Option<i64>,
    #[serde(default)]
    pub taker_fee: Option<i64>,
    #[serde(default)]
    pub maker_fee: Option<i64>,
    #[serde(default)]
    pub taker_position_sign_changed: Option<bool>,
    #[serde(default)]
    pub maker_position_sign_changed: Option<bool>,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub tx_hash: Option<String>,
    /// The original trade object as JSON text. Empty on rows backfilled from
    /// the venue's historical export (`source` `"bucket"`).
    #[serde(default)]
    pub raw_json: String,
    /// Where the row came from: `"bucket"` for the venue's historical export,
    /// `"ws"` for the live capture.
    #[serde(default)]
    pub source: Option<String>,
}

/// Lighter liquidation volume for one time bucket.
///
/// Lighter buckets carry the total and the count only; there is no long/short
/// split.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterLiquidationVolume {
    pub symbol: String,
    /// Bucket start in Unix milliseconds.
    pub timestamp: i64,
    /// Total liquidated notional in the quote asset, as a decimal string.
    #[serde(deserialize_with = "deserialize_number_or_string")]
    pub total_usd: String,
    pub count: i64,
}

// ---------------------------------------------------------------------------
// Aggregation intervals (OI / funding)
// ---------------------------------------------------------------------------

/// Supported aggregation intervals for open interest, funding, and HIP-3 breadth history queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OiFundingInterval {
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    FourHours,
    OneDay,
}

impl OiFundingInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            OiFundingInterval::FiveMinutes => "5m",
            OiFundingInterval::FifteenMinutes => "15m",
            OiFundingInterval::ThirtyMinutes => "30m",
            OiFundingInterval::OneHour => "1h",
            OiFundingInterval::FourHours => "4h",
            OiFundingInterval::OneDay => "1d",
        }
    }
}

// ---------------------------------------------------------------------------
// Lighter orderbook granularity
// ---------------------------------------------------------------------------

/// Lighter.xyz orderbook snapshot granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LighterGranularity {
    Checkpoint,
    ThirtySeconds,
    TenSeconds,
    OneSecond,
    Tick,
}

impl LighterGranularity {
    pub fn as_str(&self) -> &'static str {
        match self {
            LighterGranularity::Checkpoint => "checkpoint",
            LighterGranularity::ThirtySeconds => "30s",
            LighterGranularity::TenSeconds => "10s",
            LighterGranularity::OneSecond => "1s",
            LighterGranularity::Tick => "tick",
        }
    }
}

// ---------------------------------------------------------------------------
// Lighter live WebSocket payloads
// ---------------------------------------------------------------------------

/// One price level in a live `lighter_orderbook` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LighterLiveLevel {
    /// Price as a decimal string, exactly as Lighter publishes it.
    pub px: String,
    /// Size as a decimal string, exactly as Lighter publishes it.
    pub sz: String,
    /// Always `1`. Lighter does not publish per-level order counts.
    pub n: i64,
}

/// The `data` payload of a live `lighter_orderbook` message.
///
/// Every message is a full book of up to 20 levels per side, not a diff. The
/// server sends the newest book at most once per subscription interval (one
/// second by default, see `OxArchiveWs::subscribe_with_interval`), and sends
/// the current book right after subscribing when one is available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LighterLiveOrderBook {
    /// Symbol, uppercase.
    pub coin: String,
    /// Lighter's book update time in Unix milliseconds.
    pub time: i64,
    /// `[bids, asks]`: bids best (highest) first, asks best (lowest) first.
    /// Use [`bids`](Self::bids) and [`asks`](Self::asks) to read each side.
    pub levels: Vec<Vec<LighterLiveLevel>>,
}

impl LighterLiveOrderBook {
    /// Bid levels, best (highest price) first.
    pub fn bids(&self) -> &[LighterLiveLevel] {
        self.levels.first().map(Vec::as_slice).unwrap_or(&[])
    }

    /// Ask levels, best (lowest price) first.
    pub fn asks(&self) -> &[LighterLiveLevel] {
        self.levels.get(1).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// One fill in a live `lighter_trades` message.
///
/// Each `lighter_trades` message carries an array of fills with two fills per
/// trade, one for each side, sharing the same `tid`. Count trades by distinct
/// `tid`, not by array length, and compute volume by summing `sz` over one
/// fill per `tid`.
///
/// Live fills are preliminary. The finalized record, including fields the
/// live stream does not carry (such as fees), is served by
/// `client.lighter.trades.list(...)` (Robinhood Chain:
/// `client.rh_lighter.trades.list(...)`), which returns reconciled trades
/// only; `trades.recent(...)` serves the preliminary tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LighterLiveTrade {
    /// Symbol, uppercase.
    pub coin: String,
    /// `"A"` for the ask side, `"B"` for the bid side.
    pub side: String,
    /// Price as a decimal string.
    pub px: String,
    /// Size as a decimal string.
    pub sz: String,
    /// Trade time in Unix milliseconds.
    pub time: i64,
    /// Lighter transaction hash.
    pub hash: Option<String>,
    /// Trade id, shared by both fills of the trade.
    pub tid: i64,
    /// This side's order id.
    pub oid: Option<i64>,
    /// `true` for the taker fill, `false` for the maker fill.
    pub crossed: bool,
    /// Always `None` in live messages.
    pub dir: Option<String>,
    /// Always `None` in live messages. Fees are on the finalized REST record.
    pub fee: Option<String>,
    /// Always `None` in live messages.
    pub fee_token: Option<String>,
    /// Always `None` in live messages.
    pub closed_pnl: Option<String>,
    /// This account's signed position before the trade, as a decimal string.
    pub start_position: Option<String>,
    /// The Lighter account index for this fill, as a one-element list of
    /// strings.
    #[serde(default)]
    pub users: Vec<String>,
}

/// The `data` payload of a live `lighter_open_interest` or `lighter_funding`
/// message. Both channels carry the same message.
///
/// Updates arrive as Lighter publishes them, about once per second per
/// market. The latest values are sent right after subscribing when available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LighterLiveMarketStats {
    /// Symbol, uppercase.
    pub coin: String,
    /// The market statistics.
    pub ctx: LighterLiveAssetCtx,
}

/// Market statistics in a live Lighter open-interest or funding message.
///
/// Wire keys are camelCase (`openInterest`, `markPx`, ...). All values are
/// decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LighterLiveAssetCtx {
    /// Lighter's reported open interest, the same value as `open_interest`
    /// from `client.lighter.open_interest.current(...)`.
    pub open_interest: Option<String>,
    /// Current funding rate as a decimal fraction, the same unit as
    /// `funding_rate` from `client.lighter.funding.current(...)`. Lighter
    /// publishes a percent; this value is that percent divided by 100.
    pub funding: Option<String>,
    /// Premium as a decimal fraction.
    pub premium: Option<String>,
    /// Mark price.
    pub mark_px: Option<String>,
    /// Lighter's index price.
    pub oracle_px: Option<String>,
    /// Mid price.
    pub mid_px: Option<String>,
    /// 24-hour quote volume.
    pub day_ntl_vlm: Option<String>,
    /// 24-hour base volume.
    pub day_base_vlm: Option<String>,
    /// Derived from the last trade price and Lighter's 24-hour percent change.
    pub prev_day_px: Option<String>,
    /// Always `None`. Lighter has no impact prices.
    pub impact_pxs: Option<Vec<String>>,
}

/// A typed payload from a live Lighter WebSocket `data` message.
///
/// Both Lighter deployments use these shapes: the mainnet `lighter_*`
/// channels and the Robinhood Chain `rh_lighter_*` channels. The message's
/// `channel` tells the deployments apart.
///
/// Decode with [`LighterLiveData::decode`], or with `ServerMsg::lighter_live_data`
/// when the `websocket` feature is enabled. Replay messages keep their
/// existing replay row shapes, which differ from these live payloads, so they
/// are not decoded by this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LighterLiveData {
    /// A `lighter_orderbook` or `rh_lighter_orderbook` book.
    OrderBook(LighterLiveOrderBook),
    /// A `lighter_trades` or `rh_lighter_trades` batch: two fills per trade.
    Trades(Vec<LighterLiveTrade>),
    /// A `lighter_open_interest` or `rh_lighter_open_interest` update.
    OpenInterest(LighterLiveMarketStats),
    /// A `lighter_funding` or `rh_lighter_funding` update (same message as
    /// the open-interest channel of the same deployment).
    Funding(LighterLiveMarketStats),
}

impl LighterLiveData {
    /// Decode the `data` field of a live `data` message.
    ///
    /// Returns `None` when `channel` is not one of the live Lighter channels
    /// (`lighter_orderbook`, `lighter_trades`, `lighter_open_interest`,
    /// `lighter_funding`, or the same four with the `rh_lighter_` prefix), and
    /// `Some(Err(..))` when the payload does not match the live shape.
    pub fn decode(channel: &str, data: &serde_json::Value) -> Option<crate::Result<Self>> {
        let kind = channel
            .strip_prefix("rh_lighter_")
            .or_else(|| channel.strip_prefix("lighter_"))?;
        let decoded = match kind {
            "orderbook" => LighterLiveOrderBook::deserialize(data).map(Self::OrderBook),
            "trades" => Vec::<LighterLiveTrade>::deserialize(data).map(Self::Trades),
            "open_interest" => LighterLiveMarketStats::deserialize(data).map(Self::OpenInterest),
            "funding" => LighterLiveMarketStats::deserialize(data).map(Self::Funding),
            _ => return None,
        };
        Some(decoded.map_err(|e| crate::Error::Deserialize(e.to_string())))
    }
}

// ---------------------------------------------------------------------------
// Convenience / summary types
// ---------------------------------------------------------------------------

/// Freshness information for a single data type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeFreshness {
    pub last_updated: Option<String>,
    pub lag_ms: Option<i64>,
}

/// Per-coin data freshness across all data types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinFreshness {
    pub coin: String,
    pub exchange: Option<String>,
    pub measured_at: Option<String>,
    #[serde(flatten)]
    pub data_types: std::collections::HashMap<String, DataTypeFreshness>,
}

/// Combined market summary for a single coin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinSummary {
    pub coin: String,
    pub mark_price: Option<String>,
    pub mid_price: Option<String>,
    pub oracle_price: Option<String>,
    pub open_interest: Option<String>,
    pub funding_rate: Option<String>,
    /// 24h notional volume, Hyperliquid naming. Lighter sends `volume_24h`
    /// instead; check that field on Lighter summaries.
    pub day_ntl_volume: Option<String>,
    /// 24h volume, Lighter naming.
    #[serde(default, deserialize_with = "deserialize_opt_number_or_string")]
    pub volume_24h: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// A price snapshot (mark, oracle, mid at a point in time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub timestamp: String,
    pub mark_price: Option<String>,
    pub oracle_price: Option<String>,
    pub mid_price: Option<String>,
}

// ---------------------------------------------------------------------------
// Data quality
// ---------------------------------------------------------------------------

/// Overall system status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub exchanges: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub data_types: std::collections::HashMap<String, serde_json::Value>,
    pub active_incidents: Option<i64>,
}

/// Coverage information for supported venue APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageResponse {
    pub exchanges: Vec<ExchangeCoverage>,
}

/// Coverage information for a single venue scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCoverage {
    pub exchange: String,
    #[serde(default)]
    pub data_types: std::collections::HashMap<String, DataTypeCoverage>,
}

/// Coverage metrics for a single data type on an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeCoverage {
    pub earliest: Option<String>,
    pub latest: Option<String>,
    pub total_records: Option<i64>,
    pub completeness: Option<f64>,
}

/// Symbol-level coverage with gap detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolCoverageResponse {
    pub exchange: String,
    pub symbol: String,
    #[serde(default)]
    pub data_types: std::collections::HashMap<String, serde_json::Value>,
}

/// A data incident.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub status: String,
    pub severity: String,
    pub exchange: Option<String>,
    #[serde(default)]
    pub data_types: Vec<String>,
    #[serde(default)]
    pub symbols_affected: Vec<String>,
    pub started_at: String,
    pub resolved_at: Option<String>,
    pub duration_minutes: Option<f64>,
    pub title: String,
    pub description: Option<String>,
    pub root_cause: Option<String>,
    pub resolution: Option<String>,
}

/// List of incidents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentsResponse {
    pub incidents: Vec<Incident>,
}

/// Latency metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyResponse {
    pub measured_at: Option<String>,
    #[serde(default)]
    pub exchanges: std::collections::HashMap<String, serde_json::Value>,
}

/// SLA compliance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaResponse {
    pub period: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Web3 authentication
// ---------------------------------------------------------------------------

/// SIWE challenge response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiweChallenge {
    pub message: String,
    pub nonce: String,
}

/// Result of a web3 signup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3SignupResult {
    pub api_key: String,
    pub tier: String,
    pub wallet_address: String,
}

/// A web3 API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3ApiKey {
    pub id: String,
    pub name: Option<String>,
    pub key_prefix: String,
    pub is_active: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// List of web3 API keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3KeysList {
    pub keys: Vec<Web3ApiKey>,
    pub wallet_address: String,
}

/// Result of revoking a web3 API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3RevokeResult {
    pub message: String,
    pub wallet_address: String,
}

/// x402 payment details for upgrading via crypto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3PaymentRequired {
    pub amount: String,
    pub asset: String,
    pub network: String,
    pub pay_to: String,
    pub asset_address: Option<String>,
}

/// Result of a web3 subscription payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3SubscribeResult {
    pub api_key: Option<String>,
    pub tier: String,
    pub expires_at: Option<String>,
    pub wallet_address: String,
}

// ---------------------------------------------------------------------------
// Orderbook reconstruction (tick-level data)
// ---------------------------------------------------------------------------

/// A single atomic change to the order book.
///
/// Deltas are returned by the tick-level orderbook history endpoint
/// and must be applied in sequence order. A `size` of
/// `0.0` means the price level should be removed entirely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookDelta {
    /// Unix milliseconds.
    pub timestamp: i64,
    /// `"bid"` or `"ask"`.
    pub side: String,
    /// Price level.
    pub price: f64,
    /// New total size at this level. `0.0` means remove.
    pub size: f64,
    /// Monotonically increasing sequence number.
    pub sequence: i64,
}

/// Raw tick-level data: a checkpoint snapshot plus incremental deltas.
///
/// Returned by [`crate::resources::OrderBookResource::history_tick`].
#[derive(Debug, Clone)]
pub struct TickData {
    /// Full L2 snapshot at the start of the requested range.
    pub checkpoint: OrderBook,
    /// Incremental changes to apply on top of the checkpoint.
    pub deltas: Vec<OrderbookDelta>,
}

/// A reconstructed order book snapshot with sequence tracking.
///
/// Produced by [`crate::orderbook_reconstructor::OrderBookReconstructor`]
/// after applying deltas to a checkpoint.
#[derive(Debug, Clone)]
pub struct ReconstructedOrderBook {
    pub coin: String,
    pub timestamp: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub mid_price: Option<String>,
    pub spread: Option<String>,
    pub spread_bps: Option<String>,
    /// Sequence number of the last applied delta, if any.
    pub sequence: Option<i64>,
}

/// Options controlling orderbook reconstruction behavior.
#[derive(Debug, Clone)]
pub struct ReconstructOptions {
    /// Limit output to the top N price levels per side.
    pub depth: Option<usize>,
    /// If `true` (default), emit a snapshot after every delta.
    /// If `false`, only return the final state.
    pub emit_all: bool,
}

impl Default for ReconstructOptions {
    fn default() -> Self {
        Self {
            depth: None,
            emit_all: true,
        }
    }
}

// ---------------------------------------------------------------------------
// L4 Orderbook (typed responses)
// ---------------------------------------------------------------------------

/// A single order in an L4 orderbook snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L4OrderEntry {
    pub oid: u64,
    pub user_address: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
}

/// L4 orderbook snapshot with individual orders and user attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L4OrderBookSnapshot {
    pub coin: String,
    pub timestamp: String,
    pub checkpoint_timestamp: String,
    pub diffs_applied: u64,
    pub last_block_number: u64,
    pub bid_count: usize,
    pub ask_count: usize,
    pub total_bid_size: f64,
    pub total_ask_size: f64,
    pub bids: Vec<L4OrderEntry>,
    pub asks: Vec<L4OrderEntry>,
}

/// A single L4 orderbook diff (order placement, modification, or cancellation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L4DiffEntry {
    pub coin: String,
    pub timestamp: String,
    pub block_number: u64,
    /// Within-block sequence number. Faithful engine ordering from late May
    /// 2026 onward; `0` on earlier rows.
    #[serde(default)]
    pub seq: u64,
    pub oid: u64,
    pub side: String,
    pub price: f64,
    pub diff_type: String,
    pub new_size: Option<f64>,
    pub user_address: String,
    /// ALO queue priority: for `new` diffs placed with queue priority, the
    /// `oid` this order was inserted ahead of. Absent for tail placements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_before: Option<u64>,
}

// ---------------------------------------------------------------------------
// Liquidation levels (projected forced-liquidation levels)
// ---------------------------------------------------------------------------

/// One price bucket of projected forced-liquidation exposure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationLevelBucket {
    /// Bucket center price.
    pub price: f64,
    /// USD notional of long positions projected to liquidate in this bucket.
    pub long_notional: f64,
    /// USD notional of short positions projected to liquidate in this bucket.
    pub short_notional: f64,
    /// Number of long positions in this bucket.
    pub long_count: u64,
    /// Number of short positions in this bucket.
    pub short_count: u64,
}

/// Projected forced-liquidation levels for one snapshot, computed from
/// clearinghouse positions and margin state.
/// Snapshots refresh approximately every five minutes; `snapshot_ts` identifies the snapshot served.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationLevels {
    /// Mark price at the snapshot, center of the requested range.
    pub mid_price: f64,
    /// UTC snapshot time the levels reflect.
    pub snapshot_ts: String,
    /// Hyperliquid block height the snapshot reflects.
    pub block_number: u64,
    /// Total long notional at risk across the whole book.
    pub total_long: f64,
    /// Total short notional at risk across the whole book.
    pub total_short: f64,
    /// Notional computed approximately or not bucketed (HIP-3 cross-margin exposure).
    pub flagged_notional: f64,
    /// Price buckets inside the requested range.
    pub levels: Vec<LiquidationLevelBucket>,
}

/// One historical liquidation-levels snapshot. `levels` is `None` when the
/// history was requested with `summary = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationLevelsHistoryItem {
    pub snapshot_ts: String,
    pub block_number: u64,
    pub mid_price: f64,
    pub total_long: f64,
    pub total_short: f64,
    pub flagged_notional: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<LiquidationLevelBucket>>,
}

// ---------------------------------------------------------------------------
// Trigger levels (pending stop-loss / take-profit orders)
// ---------------------------------------------------------------------------

/// Aggregated currently open trigger orders at one rounded price bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerLevelBucket {
    /// Rounded trigger price bucket.
    pub price_bucket: f64,
    /// Number of bid-side trigger orders in the bucket.
    pub bid_count: u64,
    /// Bid-side trigger size in the bucket.
    pub bid_size: f64,
    /// Number of ask-side trigger orders in the bucket.
    pub ask_count: u64,
    /// Ask-side trigger size in the bucket.
    pub ask_size: f64,
}

/// Currently pending stop-loss and take-profit trigger orders grouped into
/// price buckets. Voluntary trigger orders, not projected forced
/// liquidations; use [`LiquidationLevels`] for those.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerLevels {
    /// Current mid/mark price, center of the requested range.
    pub mid_price: f64,
    /// UTC RFC3339 server time the pending-trigger state was read.
    pub as_of: String,
    /// Total pending bid size across the returned window.
    pub total_bid_size: f64,
    /// Total pending ask size across the returned window.
    pub total_ask_size: f64,
    /// Price buckets inside the requested range.
    pub levels: Vec<TriggerLevelBucket>,
}

/// One historical trigger-levels snapshot (15-minute cadence). `levels` is
/// `None` when the history was requested with `summary = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerLevelsHistoryItem {
    pub snapshot_ts: String,
    pub mid_price: f64,
    pub total_bid_size: f64,
    pub total_ask_size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<TriggerLevelBucket>>,
}

// ---------------------------------------------------------------------------
// L2 Full-Depth Orderbook (typed responses)
// ---------------------------------------------------------------------------

/// A single price level in an L2 orderbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2PriceLevel {
    pub px: f64,
    pub sz: f64,
    pub n: u32,
}

/// L2 full-depth orderbook snapshot with aggregated price levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2OrderBookSnapshot {
    pub coin: String,
    pub timestamp: String,
    pub bid_levels: usize,
    pub ask_levels: usize,
    pub total_bid_size: f64,
    pub total_ask_size: f64,
    pub mid_price: Option<f64>,
    pub spread: Option<f64>,
    pub spread_bps: Option<f64>,
    pub bids: Vec<L2PriceLevel>,
    pub asks: Vec<L2PriceLevel>,
}

/// A single L2 tick-level diff (price level change).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2DiffEntry {
    pub timestamp: String,
    pub block_number: u64,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Order History (typed responses)
// ---------------------------------------------------------------------------

/// An order lifecycle event (placement, fill, cancel, trigger).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderHistoryEntry {
    pub coin: String,
    pub timestamp: String,
    pub block_number: u64,
    pub block_time: String,
    pub oid: u64,
    pub user_address: String,
    pub side: String,
    pub limit_price: f64,
    pub size: f64,
    pub orig_size: f64,
    pub status: String,
    pub order_type: String,
    pub tif: String,
    pub reduce_only: bool,
    pub is_trigger: bool,
    pub is_position_tpsl: bool,
    pub cloid: Option<String>,
}

// ---------------------------------------------------------------------------
// Account positions
// ---------------------------------------------------------------------------
//
// Numbers are decimal strings: sizes at the market's size precision, prices
// at its price precision, USD values at 6 decimals. A flat position is "0".
// Instants in rows are RFC 3339 UTC strings with a fractional part only when
// it is not zero ("2026-09-25T00:00:00Z"); instants in `meta` always carry
// milliseconds ("2026-09-25T00:00:00.000Z"). Parse before comparing.

/// Leverage of a position.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PositionLeverage {
    /// `cross` or `isolated`; `unknown` on reconstructed Hyperliquid rows.
    /// Lighter reports its margin mode here.
    #[serde(rename = "type", default)]
    pub kind: String,
    /// Leverage multiple as a decimal string, when known.
    #[serde(default)]
    pub value: Option<String>,
}

/// Cumulative funding on a position, in USD.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CumulativeFunding {
    pub all_time: Option<String>,
    pub since_open: Option<String>,
    pub since_change: Option<String>,
}

/// One open position, from the wallet or account routes.
///
/// `size` is signed (negative is short) and `side` is `long` or `short`.
/// Hyperliquid rows identify the market with `symbol` (and `dex` on HIP-3);
/// Lighter rows add `account_index`, `account_kind` and the Lighter margin
/// fields. Fields a response cannot state are `None`: a reconstructed row
/// (`meta.source` `reconstructed`) has exact size, entry and `opened_at` and
/// a mark at the requested time. On Hyperliquid and HIP-3 its
/// `leverage.kind` is `unknown`, `leverage.value`, the `cum_funding`
/// members, `margin_used`, `return_on_equity` and `liquidation_price` are
/// `None`, and `liquidation_price_status` is `unavailable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Hour the row describes, on history rows.
    #[serde(default)]
    pub snapshot_ts: Option<String>,
    /// Lighter account index as a string.
    #[serde(default)]
    pub account_index: Option<String>,
    /// Lighter account kind: `user`, `insurance`, `settlement` or `system`.
    #[serde(default)]
    pub account_kind: Option<String>,
    pub symbol: String,
    /// Same value as `symbol`.
    #[serde(default)]
    pub coin: String,
    /// HIP-3 dex name. `None` on other venues.
    #[serde(default)]
    pub dex: Option<String>,
    /// Signed size; negative is short.
    pub size: String,
    /// `long` or `short`.
    pub side: String,
    pub entry_price: Option<String>,
    pub mark_price: Option<String>,
    /// Time of `mark_price`.
    #[serde(default)]
    pub mark_time: Option<String>,
    /// Absolute size times mark, in USD.
    pub position_value: Option<String>,
    pub unrealized_pnl: Option<String>,
    #[serde(default)]
    pub return_on_equity: Option<String>,
    #[serde(default)]
    pub leverage: PositionLeverage,
    #[serde(default)]
    pub max_leverage: Option<u32>,
    #[serde(default)]
    pub margin_used: Option<String>,
    #[serde(default)]
    pub liquidation_price: Option<String>,
    /// How `liquidation_price` was determined: `exact`,
    /// `not_published_cross`, `changed_since_snapshot` or `unavailable`.
    #[serde(default)]
    pub liquidation_price_status: String,
    #[serde(default)]
    pub cum_funding: CumulativeFunding,
    /// Start of the current position lifecycle (the last open from flat).
    /// `None` when it opened before coverage.
    #[serde(default)]
    pub opened_at: Option<String>,
    /// Time the leverage, funding and margin fields describe, when it differs
    /// from the snapshot.
    #[serde(default)]
    pub snapshot_as_of: Option<String>,
    /// Row quality. Hyperliquid: `complete`, `partial` or `degraded`.
    /// Lighter adds `preliminary`, `unreconciled` and `incomplete`.
    pub quality: String,
    /// Lighter: initial margin fraction at the last trade, as a fraction.
    #[serde(default)]
    pub initial_margin_fraction: Option<String>,
    /// Lighter: allocated margin for an isolated position.
    #[serde(default)]
    pub allocated_margin: Option<String>,
    /// Lighter: margin mode.
    #[serde(default)]
    pub margin_mode: Option<String>,
    /// Lighter: where `mark_price` came from.
    #[serde(default)]
    pub mark_source: Option<String>,
    /// Lighter: `true` once every trade behind the row is final.
    #[serde(default)]
    pub finalized: Option<bool>,
}

/// One position in a market-wide listing (`market` and `all`).
///
/// A lean record: Hyperliquid rows carry `user_address`, Lighter rows carry
/// `account_index` and `account_kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketPosition {
    /// Hour the row describes, on bulk rows.
    #[serde(default)]
    pub snapshot_ts: Option<String>,
    /// Hyperliquid wallet address.
    #[serde(default)]
    pub user_address: Option<String>,
    /// Lighter account index as a string.
    #[serde(default)]
    pub account_index: Option<String>,
    #[serde(default)]
    pub account_kind: Option<String>,
    pub symbol: String,
    #[serde(default)]
    pub coin: String,
    #[serde(default)]
    pub dex: Option<String>,
    /// Signed size; negative is short.
    pub size: String,
    /// `long` or `short`.
    pub side: String,
    pub entry_price: Option<String>,
    pub mark_price: Option<String>,
    pub position_value: Option<String>,
    pub unrealized_pnl: Option<String>,
    /// `cross` or `isolated` (Lighter: its margin mode).
    #[serde(default)]
    pub leverage_type: String,
    #[serde(default)]
    pub liquidation_price: Option<String>,
    pub quality: String,
}

/// One position change (one fill leg) from the change log.
///
/// `side` is `B` or `A` exactly as on trades. Hyperliquid rows carry
/// `direction`, `closed_pnl` and `crossed`; Lighter rows carry `realized_pnl`,
/// `is_maker`, `fee_rate`, `fee_usdc`, `usdc_amount` and the
/// `position_size_before` / `position_size_after` aliases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionChange {
    pub timestamp: String,
    #[serde(default)]
    pub account_index: Option<String>,
    #[serde(default)]
    pub account_kind: Option<String>,
    pub symbol: String,
    #[serde(default)]
    pub coin: String,
    #[serde(default)]
    pub dex: Option<String>,
    /// `B` (bid) or `A` (ask).
    pub side: String,
    pub price: Option<String>,
    pub size: Option<String>,
    /// Signed position before the leg.
    pub start_position: Option<String>,
    /// Signed position after the leg.
    pub end_position: Option<String>,
    /// Entry price after the leg; `None` when the position is flat.
    #[serde(default)]
    pub entry_price_after: Option<String>,
    /// What the leg did to the position, for example `open`, `increase`,
    /// `reduce`, `close` or `flip`. On Lighter, a leg that leaves the
    /// position unchanged is `settlement` or `unchanged`.
    pub event_type: String,
    /// Why the leg happened: `trade`, `liquidation`,
    /// `liquidation_counterparty`, `adl`, `settlement`, or `unknown` where
    /// the source cannot tell.
    #[serde(default)]
    pub cause: String,
    /// Hyperliquid trade direction, for example `Open Long`.
    #[serde(default)]
    pub direction: Option<String>,
    /// Hyperliquid realized PnL on the leg.
    #[serde(default)]
    pub closed_pnl: Option<String>,
    /// Lighter realized PnL on the leg.
    #[serde(default)]
    pub realized_pnl: Option<String>,
    #[serde(default)]
    pub fee: Option<String>,
    #[serde(default)]
    pub fee_token: String,
    /// Hyperliquid: `true` for the taker leg.
    #[serde(default)]
    pub crossed: Option<bool>,
    /// Lighter: `true` for the maker leg.
    #[serde(default)]
    pub is_maker: Option<bool>,
    pub trade_id: i64,
    #[serde(default)]
    pub order_id: Option<i64>,
    #[serde(default)]
    pub opened_at: Option<String>,
    /// Hyperliquid: execution order of the legs that share a timestamp.
    #[serde(default)]
    pub seq: Option<u64>,
    /// Hyperliquid block number, when known.
    #[serde(default)]
    pub block_number: Option<u64>,
    /// Position of the event within its block, when known.
    #[serde(default)]
    pub event_index: Option<u64>,
    /// `ok`, `inferred`, `first_seen` or `quarantined`.
    #[serde(default)]
    pub continuity: String,
    #[serde(default)]
    pub position_size_before: Option<String>,
    #[serde(default)]
    pub position_size_after: Option<String>,
    #[serde(default)]
    pub fee_rate: Option<String>,
    #[serde(default)]
    pub fee_usdc: Option<String>,
    #[serde(default)]
    pub usdc_amount: Option<String>,
    /// `true` once the leg is final.
    #[serde(default)]
    pub finalized: Option<bool>,
}

/// Account summary.
///
/// Hyperliquid returns one per (address, dex): the margin fields describe
/// that clearinghouse. `withdrawable` is only recorded for part of the
/// history. Lighter summaries carry the position aggregates only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountSummary {
    /// Hour the row describes, on history rows.
    #[serde(default)]
    pub snapshot_ts: Option<String>,
    #[serde(default)]
    pub account_index: Option<String>,
    /// HIP-3 dex name.
    #[serde(default)]
    pub dex: Option<String>,
    #[serde(default)]
    pub account_value: Option<String>,
    #[serde(default)]
    pub cross_account_value: Option<String>,
    #[serde(default)]
    pub collateral: Option<String>,
    #[serde(default)]
    pub total_margin_used: Option<String>,
    #[serde(default)]
    pub cross_maintenance_margin_used: Option<String>,
    #[serde(default)]
    pub withdrawable: Option<String>,
    pub total_position_value: Option<String>,
    pub total_unrealized_pnl: Option<String>,
    pub long_value: Option<String>,
    pub short_value: Option<String>,
    pub n_positions: u64,
    #[serde(default)]
    pub account_mode: Option<String>,
    #[serde(default)]
    pub snapshot_as_of: Option<String>,
    pub quality: String,
}

/// Long and short aggregates of one market at one snapshot.
///
/// Returned by `market_summary` and, for market listings, in `meta.totals`
/// (see [`ResponseMeta::position_totals`]). Average entries cover exactly
/// the positions counted in `*_positions_with_entry`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketPositionsSummary {
    #[serde(default)]
    pub snapshot_ts: Option<String>,
    pub symbol: String,
    #[serde(default)]
    pub coin: String,
    #[serde(default)]
    pub dex: Option<String>,
    pub long_count: u64,
    pub short_count: u64,
    pub long_size: String,
    pub short_size: String,
    pub long_value: Option<String>,
    pub short_value: Option<String>,
    pub long_avg_entry_price: Option<String>,
    pub short_avg_entry_price: Option<String>,
    #[serde(default)]
    pub long_positions_with_entry: u64,
    #[serde(default)]
    pub short_positions_with_entry: u64,
    /// Share of long value held by the ten largest long positions, 0 to 1.
    pub long_top10_value_share: Option<String>,
    /// Share of short value held by the ten largest short positions, 0 to 1.
    pub short_top10_value_share: Option<String>,
    /// Share of total value held by the ten largest positions, 0 to 1.
    pub top10_value_share: Option<String>,
    pub quality: String,
}

/// `data` of the current or as-of wallet (account) positions route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletPositions {
    pub positions: Vec<Position>,
    /// Account summary, on the first page of a snapshot response only:
    ///
    /// - Hyperliquid core: when the wallet has an account row in the
    ///   snapshot served (a wallet that was never seen has none).
    /// - HIP-3: when the request is scoped to one dex, by `dex` or by a HIP-3
    ///   `symbol` (which names its dex), and the wallet has an account row
    ///   on that dex.
    /// - Lighter (both deployments): when the request has no `symbol`. An
    ///   account with no open positions in the snapshot gets a summary of
    ///   zeros.
    ///
    /// `None` on continuation pages and on reconstructed responses
    /// (`meta.source` `reconstructed`).
    #[serde(default)]
    pub account: Option<AccountSummary>,
    /// Set when `positions` is empty: `flat` (seen before, no open
    /// positions), `never_seen` (no recorded activity in the covered history;
    /// see `meta.notice` and `meta.coverage_from`), or `outside_coverage`.
    #[serde(default)]
    pub account_seen: Option<String>,
}

/// One Lighter account owned by an L1 address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LighterL1Account {
    /// Account index as a string.
    pub account_index: String,
    /// Lighter account type.
    pub account_type: u32,
    #[serde(default)]
    pub first_seen: Option<String>,
}

/// Lighter accounts owned by an L1 address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LighterL1Accounts {
    pub l1_address: String,
    /// Accounts owned in total, across every page.
    pub total_accounts: u64,
    pub accounts: Vec<LighterL1Account>,
}

#[cfg(test)]
mod lighter_live_payload_tests {
    use super::LighterLiveData;

    #[test]
    fn decode_works_without_the_websocket_feature() {
        let book = serde_json::json!({
            "coin": "BTC",
            "time": 1790294171459_i64,
            "levels": [
                [{"px": "84368.7", "sz": "0.00020", "n": 1}],
                [{"px": "84368.8", "sz": "0.05720", "n": 1}]
            ]
        });
        match LighterLiveData::decode("lighter_orderbook", &book) {
            Some(Ok(LighterLiveData::OrderBook(book))) => {
                assert_eq!(book.bids()[0].px, "84368.7");
                assert_eq!(book.asks()[0].sz, "0.05720");
            }
            other => panic!("unexpected decode result: {other:?}"),
        }

        let stats = serde_json::json!({
            "coin": "BTC",
            "ctx": {"openInterest": "172706178.266310", "funding": "0.000012", "impactPxs": null}
        });
        match LighterLiveData::decode("lighter_funding", &stats) {
            Some(Ok(LighterLiveData::Funding(stats))) => {
                assert_eq!(stats.ctx.funding.as_deref(), Some("0.000012"));
                assert_eq!(stats.ctx.mark_px, None);
            }
            other => panic!("unexpected decode result: {other:?}"),
        }

        assert!(LighterLiveData::decode("lighter_candles", &stats).is_none());
        assert!(LighterLiveData::decode("orderbook", &book).is_none());
        assert!(matches!(
            LighterLiveData::decode("lighter_trades", &stats),
            Some(Err(crate::Error::Deserialize(_)))
        ));
    }
}
