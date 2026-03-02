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

    impl<'de> serde::de::Visitor<'de> for NumberOrString {
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

// ---------------------------------------------------------------------------
// Generic response envelope
// ---------------------------------------------------------------------------

/// Metadata returned with every API response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMeta {
    pub count: usize,
    pub request_id: String,
    pub next_cursor: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: Option<f64>,
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

/// Lighter.xyz orderbook snapshot granularity (tier-gated).
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
    pub day_ntl_volume: Option<String>,
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

/// Coverage information for all exchanges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageResponse {
    pub exchanges: Vec<ExchangeCoverage>,
}

/// Coverage information for a single exchange.
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
/// (Enterprise tier) and must be applied in sequence order. A `size` of
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
