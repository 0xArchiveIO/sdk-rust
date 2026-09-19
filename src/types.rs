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
    pub fee: Option<String>,
    pub fee_token: Option<String>,
    pub closed_pnl: Option<String>,
    pub direction: Option<String>,
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
/// Coin format: `#<10*outcome_id + side>`. Backend accepts the bare numeric
/// form on every path; the SDK passes `symbol` through unchanged.
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
    pub oracle_price: Option<String>,
    pub mid_price: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
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
// Aggregation intervals (OI / funding)
// ---------------------------------------------------------------------------

/// Supported aggregation intervals for open interest and funding queries.
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
/// clearinghouse positions and margin state. Snapshots refresh roughly every
/// 45 minutes; `snapshot_ts` identifies the snapshot served.
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
// Webhooks
// ---------------------------------------------------------------------------

/// One entry in the served event-type catalog
/// (`GET /v1/webhooks/event-types`).
///
/// The catalog is the single source the dashboard, the docs and the MCP all
/// render from. Read `params` and `metrics` from here rather than hardcoding
/// thresholds: subscribe-time validation checks a config against exactly this
/// declaration, so anything it does not declare is refused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventType {
    /// The event type, for example `account.fill`.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Payload schema version for this type.
    pub schema_version: i64,
    /// `true` when the type accepts subscriptions. The rest are published as
    /// coming soon and are refused at create time.
    pub live: bool,
    /// `public`, `addresses` (scoped to your watched wallets) or `user`
    /// (about your own account, such as an export finishing).
    pub scope: String,
    /// Venues the type covers. Empty when the type is not venue scoped.
    #[serde(default)]
    pub venues: Vec<String>,
    /// Filter keys this type accepts: some of `venue`, `symbols`, `addresses`.
    #[serde(default)]
    pub filters: Vec<String>,
    /// Declared parameters, keyed by parameter name.
    #[serde(default)]
    pub params: std::collections::HashMap<String, WebhookParamSpec>,
    /// Declared metrics that conditions may be written against, keyed by
    /// metric name.
    #[serde(default)]
    pub metrics: std::collections::HashMap<String, WebhookMetricSpec>,
    /// The exchange-wide scan floor for the type, when it has one. A
    /// subscription cannot ask for occurrences below it.
    pub cost_floor: Option<WebhookCostFloor>,
    /// Rough delivery latency: `seconds` or `minutes`.
    pub latency_class: String,
    /// Human description of what fires the event.
    pub description: String,
    /// A ready-to-send example config for this type.
    #[serde(default)]
    pub filters_example: serde_json::Value,
    /// The operator vocabulary, grouped by metric type (`number`, `text`,
    /// `boolean`, `timestamp`, `any`).
    #[serde(default)]
    pub operators: std::collections::HashMap<String, Vec<String>>,
}

/// A declared subscription parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookParamSpec {
    /// `number`, `integer`, `string` or `array_of_number`.
    #[serde(rename = "type")]
    pub value_type: String,
    /// The value used when the subscription does not set the parameter.
    pub default: Option<serde_json::Value>,
    /// Unit the value is expressed in, for example `USD`, `s` or `%`.
    pub unit: Option<String>,
    /// Closed set of accepted values, when the parameter has one.
    #[serde(rename = "enum")]
    pub allowed: Option<Vec<serde_json::Value>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub description: Option<String>,
}

/// A declared metric, which is what a condition is written against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookMetricSpec {
    /// `number`, `integer`, `string`, `enum`, `boolean` or `timestamp`.
    #[serde(rename = "type")]
    pub value_type: String,
    /// Unit the metric is expressed in.
    pub unit: Option<String>,
    /// Accepted members, for enum metrics.
    pub values: Option<Vec<String>>,
    pub description: Option<String>,
}

/// The exchange-wide scan floor declared by an event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookCostFloor {
    /// The metric the floor applies to.
    pub metric: String,
    /// The lowest value the detector scans for.
    pub min: f64,
    pub unit: Option<String>,
    pub note: Option<String>,
}

/// A delivery endpoint: one HTTPS URL 0xArchive posts events to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub url: String,
    pub description: String,
    /// `active`, `disabled`, or `auto_disabled` after a sustained failure run.
    pub status: String,
    /// Failed attempts since the last success. Ten of them spanning at least
    /// six hours auto disable the endpoint.
    pub consecutive_failures: i32,
    pub created_at: String,
    /// The signing secret, returned **only** by create and rotate. Store it on
    /// the spot; no list or get call will ever return it again.
    #[serde(default)]
    pub secret: Option<String>,
}

/// The result of rotating an endpoint's signing secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotatedSecret {
    /// The new secret. Shown once.
    pub secret: String,
    /// The server's note about the overlap window, when it sends one.
    #[serde(default)]
    pub note: Option<String>,
}

/// A subscription: one event type plus the config that decides which
/// occurrences of it reach an endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSubscription {
    pub id: String,
    pub endpoint_id: String,
    pub event_type: String,
    /// The stored, normalised config: `venue`, `symbols`, `addresses`,
    /// `params` with declared defaults filled in, and `conditions`. The API
    /// calls this object `filters` here and on create, and `config` on the
    /// dry-run and estimate bodies; it is the same object in all four places.
    #[serde(default)]
    pub filters: serde_json::Value,
    pub enabled: bool,
    pub created_at: String,
    /// Any field the API adds that this SDK version does not name yet.
    ///
    /// Pause state lands here for now. When an account exceeds its plan's
    /// deliveries per day the offending subscription is paused and says so,
    /// rather than events being dropped without a signal, but the field names
    /// for that state are still settling, so they are deliberately not bound
    /// to typed fields yet. Read them out of this map if you need them early,
    /// and expect named fields in a later release.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// One delivery attempt record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    /// The event UUID. Stable across retries and manual redelivery, and what
    /// a receiver deduplicates on.
    pub event_id: String,
    pub event_type: String,
    /// `pending`, `delivered` or `exhausted`.
    pub state: String,
    pub attempts: i32,
    pub last_status_code: Option<i32>,
    pub last_error: Option<String>,
    pub last_latency_ms: Option<i32>,
    pub next_attempt_at: String,
    pub delivered_at: Option<String>,
    pub created_at: String,
    /// The exact JSON body that was or will be posted.
    pub payload: serde_json::Value,
}

/// The result of queueing a `webhook.test` delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookTestFire {
    pub delivery_id: String,
    pub event_id: String,
}

/// The result of asking for a past delivery to be sent again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRedelivery {
    pub delivery_id: String,
    /// Unchanged from the original delivery, which is the point: a receiver
    /// that already processed this event should deduplicate on it.
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub attempts: Option<i32>,
    #[serde(default)]
    pub next_attempt_at: Option<String>,
}

/// A wallet the account watches, which is what address-scoped event types
/// report on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedAddress {
    pub id: String,
    /// Stored lowercase, `0x` prefixed.
    pub address: String,
    pub label: String,
    pub created_at: String,
}

/// Watched wallets plus the plan's cap, which the list and add routes both
/// report alongside the rows.
#[derive(Debug, Clone)]
pub struct WatchedAddressList {
    pub addresses: Vec<WatchedAddress>,
    /// Watched wallets this plan allows. `0` on a plan without webhooks.
    pub limit: Option<i64>,
}

/// The window an answer vouches for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookWindow {
    pub from: String,
    pub to: String,
}

/// One occurrence a preview found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookOccurrence {
    /// The occurrence's own timestamp. A real delivery's `observed_at` would
    /// be this plus the detector's ingest lag.
    pub observed_at_estimate: String,
    /// The `data` a delivery would have carried.
    pub data: serde_json::Value,
}

/// What a subscription would have delivered over a recent window
/// (`POST /v1/webhooks/subscriptions/dry-run`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDryRun {
    pub event_type: String,
    /// `from` moves up when a capped scan did not reach the requested start.
    pub window: WebhookWindow,
    /// Occurrences that matched inside the window, before `limit` is applied.
    pub matched: usize,
    /// `true` when fewer occurrences are returned than matched, or a scan hit
    /// its row cap and the window was narrowed.
    pub truncated: bool,
    /// Newest first.
    #[serde(default)]
    pub occurrences: Vec<WebhookOccurrence>,
}

/// How often a subscription would have fired over a longer window
/// (`POST /v1/webhooks/subscriptions/estimate`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEstimate {
    pub event_type: String,
    pub window: WebhookWindow,
    pub days: i64,
    /// Occurrences over the whole window.
    pub total: i64,
    /// Exactly `days` entries, oldest first, zero filled.
    #[serde(default)]
    pub per_day: Vec<WebhookDayCount>,
    pub per_day_p50: f64,
    pub per_day_max: i64,
    /// The metric the ladder and the distribution are about.
    pub primary_metric: Option<String>,
    /// Ascending: the daily rate this config would have had at each
    /// threshold on `primary_metric`, everything else unchanged. This is how
    /// you tune a rule to a delivery budget before you create it.
    #[serde(default)]
    pub ladder: Vec<WebhookLadderRung>,
    /// Quantiles of `primary_metric` over the matched occurrences.
    pub distribution: Option<WebhookDistribution>,
    /// Newest matches first, same shape as the dry-run.
    #[serde(default)]
    pub sample: Vec<WebhookOccurrence>,
    pub basis: WebhookEstimateBasis,
}

/// One 24 hour bin of an estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDayCount {
    /// The UTC date the bin ends on, so the last entry is today.
    pub date: String,
    pub count: i64,
}

/// One rung of the threshold ladder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookLadderRung {
    /// The threshold on the estimate's `primary_metric`.
    pub value: f64,
    /// Deliveries a day this config would have averaged at that threshold.
    pub per_day: f64,
}

/// Quantiles of an estimate's primary metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDistribution {
    pub n: i64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
}

/// How an estimate was arrived at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEstimateBasis {
    /// Whether the answer is exact or replayed from history.
    pub mode: String,
    pub note: Option<String>,
}
