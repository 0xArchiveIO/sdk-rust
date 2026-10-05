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
///
/// Not returned by any method; [`ResponseMeta`] is the full `meta` block that
/// [`MetaResponse`] carries.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMeta {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
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
    #[serde(default)]
    #[allow(dead_code)]
    pub meta: Option<ResponseMeta>,
}

/// The full `meta` block of a response.
///
/// Every paged method returns it on [`MetaResponse::meta`] (a
/// [`CursorResponse`] is the same type), as do the methods that give snapshot
/// or finalization context with their data, such as the account positions
/// resources. Every field is optional on the wire and is `None` (or `0` /
/// empty for `count` and `request_id`) when the server did not send it.
/// Instants are RFC 3339 UTC strings. The positions routes and the trades
/// finalization fields always send milliseconds
/// (`2026-09-25T00:00:00.000Z`), while instants in data rows carry a
/// fraction only when it is not zero (`2026-09-25T00:00:00Z`), so parse both
/// before comparing them.
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
    /// Present exactly when `has_more` is `true`.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// `true` while more pages may follow, `false` on the last page. Every
    /// cursor-paged route sends it; a final page can be empty when the page
    /// before it was exactly full.
    #[serde(default)]
    pub has_more: Option<bool>,
    /// The canonical public symbol a per-symbol route answered for (`BTC`,
    /// `km:US500`, `#0`, `HYPE-USDC`), which can differ from the path input:
    /// a HIP-4 `"0"` is answered as `#0`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The venue a per-symbol route answered for: `hyperliquid`, `hip3`,
    /// `hip4`, `spot`, `lighter` or `rh-lighter`.
    #[serde(default)]
    pub venue: Option<String>,
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

/// A response with its data, the paging state and the full `meta` block.
///
/// Every paged method returns one (as [`CursorResponse`], the same type).
/// To page, pass `next_cursor` back as the `cursor` parameter with unchanged
/// filters while `has_more` is `true`:
///
/// ```no_run
/// # use oxarchive::OxArchive;
/// # use oxarchive::resources::trades::GetTradesParams;
/// # async fn example() -> oxarchive::Result<()> {
/// # let client = OxArchive::new("key")?;
/// // The last ten minutes
/// let end = chrono::Utc::now();
/// let start = end - chrono::Duration::minutes(10);
/// let mut cursor = None;
/// loop {
///     let page = client.hyperliquid.trades.history("BTC", GetTradesParams {
///         start: start.into(),
///         end: end.into(),
///         cursor,
///         limit: Some(1000),
///         side: None,
///     }).await?;
///     println!("{} trades for {:?} on {:?}", page.data.len(), page.meta.symbol, page.meta.venue);
///     if !page.has_more {
///         break;
///     }
///     cursor = page.next_cursor;
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MetaResponse<T> {
    /// The response data.
    pub data: T,
    /// Pass this value as the `cursor` parameter to fetch the next page.
    /// `None` means there are no more pages.
    pub next_cursor: Option<String>,
    /// `true` while more pages may follow. It is `meta.has_more` when the
    /// server sent it, and otherwise whether `next_cursor` is set. Stop
    /// paging when it is `false`.
    pub has_more: bool,
    /// The full `meta` block: `symbol` and `venue` on per-symbol routes,
    /// snapshot context, finalization boundaries and notices.
    pub meta: ResponseMeta,
}

/// A paginated response: the data, the cursor for the next page, `has_more`
/// and the full `meta` block. The same type as [`MetaResponse`].
pub type CursorResponse<T> = MetaResponse<T>;

impl<T> MetaResponse<T> {
    pub(crate) fn new(data: T, meta: ResponseMeta) -> Self {
        let has_more = meta.has_more.unwrap_or(meta.next_cursor.is_some());
        Self {
            data,
            next_cursor: meta.next_cursor.clone(),
            has_more,
            meta,
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

/// A flexible timestamp that can be specified as Unix milliseconds, an ISO-8601
/// string, or a `chrono` date or time.
///
/// A time without a time zone is UTC: `"2026-09-01"` is midnight UTC,
/// `"2026-09-01T12:00:00"` is noon UTC, and a `chrono::NaiveDateTime` or
/// `chrono::NaiveDate` converts as UTC. A string with an offset (`Z`,
/// `+02:00`) keeps it, and a string of digits is Unix milliseconds.
#[derive(Debug, Clone)]
pub enum Timestamp {
    Millis(i64),
    Iso(String),
    DateTime(chrono::DateTime<chrono::Utc>),
}

impl Timestamp {
    /// Convert to Unix milliseconds for use in query parameters.
    ///
    /// A string that is not a timestamp converts to `0`.
    pub fn to_millis(&self) -> i64 {
        match self {
            Timestamp::Millis(ms) => *ms,
            Timestamp::DateTime(dt) => dt.timestamp_millis(),
            Timestamp::Iso(s) => parse_timestamp_str(s).unwrap_or(0),
        }
    }
}

/// Unix milliseconds of a timestamp string: digits, RFC 3339 with an offset,
/// or an ISO 8601 date-time or date without one (read as UTC).
fn parse_timestamp_str(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(ms) = s.parse::<i64>() {
        return Some(ms);
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, format) {
            return Some(utc(naive).timestamp_millis());
        }
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|naive| utc(naive).timestamp_millis())
}

/// A date-time without a time zone, read as UTC.
fn utc(naive: chrono::NaiveDateTime) -> chrono::DateTime<chrono::Utc> {
    chrono::TimeZone::from_utc_datetime(&chrono::Utc, &naive)
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

/// A date-time without a time zone is UTC.
impl From<chrono::NaiveDateTime> for Timestamp {
    fn from(naive: chrono::NaiveDateTime) -> Self {
        Timestamp::DateTime(utc(naive))
    }
}

/// A date is midnight UTC.
impl From<chrono::NaiveDate> for Timestamp {
    fn from(date: chrono::NaiveDate) -> Self {
        Timestamp::DateTime(utc(date.and_hms_opt(0, 0, 0).unwrap_or_default()))
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
    /// `"A"` (ask/sell) or `"B"` (bid/buy): the side of this fill.
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
    /// Independent of `builder_fee` and `deployer_fee`: paid to the network, not to a builder or
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

/// A Lighter instrument with fee and precision metadata.
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
///
/// The pairs routes send the base and quote as `base_token_name` and
/// `quote_token_name`, the wire-format pair as `name` and the spot index as
/// `pair_index`; they fill `base`, `quote`, `wire_symbol` and `spot_index`.
/// The other registry fields (`is_canonical`, token ids, size and wei
/// decimals, `base_token_address`, `deployer_fee_share`, `first_seen_at`,
/// `last_updated_at`) are kept in `extra`. For prices, read the pair's order
/// book or trades.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotPair {
    /// Dashed canonical symbol (e.g. `HYPE-USDC`).
    pub symbol: String,
    /// Base token (e.g. `HYPE`). Sent as `base_token_name`.
    #[serde(alias = "base_token_name")]
    pub base: Option<String>,
    /// Quote token (e.g. `USDC`). Sent as `quote_token_name`.
    #[serde(alias = "quote_token_name")]
    pub quote: Option<String>,
    /// Hyperliquid wire-format pair (e.g. `PURR/USDC` or `@107`). Sent as
    /// `name`.
    #[serde(alias = "name")]
    pub wire_symbol: Option<String>,
    /// Index of the pair in Hyperliquid's spot universe (the `N` of `@N`).
    /// Sent as `pair_index`.
    #[serde(alias = "pair_index")]
    pub spot_index: Option<i64>,
    /// Not returned by the pairs routes; always `None`.
    #[deprecated(
        since = "1.12.0",
        note = "the pairs routes do not return prices; read the order book or trades"
    )]
    pub mark_price: Option<f64>,
    /// Not returned by the pairs routes; always `None`.
    #[deprecated(
        since = "1.12.0",
        note = "the pairs routes do not return prices; read the order book or trades"
    )]
    pub mid_price: Option<f64>,
    /// Not returned by the pairs routes; always `None`. The registry's own
    /// times are `first_seen_at` and `last_updated_at` in `extra`.
    #[deprecated(
        since = "1.12.0",
        note = "the pairs routes do not return it; see first_seen_at and last_updated_at in extra"
    )]
    pub latest_timestamp: Option<String>,
    /// Not returned by the pairs routes; always `None`.
    #[deprecated(since = "1.12.0", note = "the pairs routes do not return it")]
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
    /// Executed size as a decimal string; the API may send a number.
    #[serde(default, deserialize_with = "deserialize_opt_number_or_string")]
    pub executed_size: Option<String>,
    /// Executed notional as a decimal string; the API may send a number.
    #[serde(default, deserialize_with = "deserialize_opt_number_or_string")]
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

/// A validated percentage-above-session-VWAP snapshot.
///
/// Returned by `client.hyperliquid.hip3.breadth` and, with empty
/// `namespaces` maps, by `client.hyperliquid.breadth`. [`BreadthSnapshot`]
/// is the same type under a venue-neutral name.
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

/// A breadth-above-session-VWAP snapshot, for Hyperliquid core or HIP-3.
pub type BreadthSnapshot = Hip3BreadthSnapshot;

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

/// Side filter for trade queries (`side=buy` or `side=sell`).
///
/// `Buy` keeps the rows whose `side` is `"B"` and `Sell` the rows whose
/// `side` is `"A"`. The filter reads each row's own side: where a trade is
/// returned as one fill per side (maker and taker), `Buy` keeps the buying
/// fill of each trade, whichever of the two was the taker. The filter applies
/// before paging, so a full page still holds `limit` matching rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeSide {
    /// Rows with `side` `"B"`.
    Buy,
    /// Rows with `side` `"A"`.
    Sell,
}

impl TradeSide {
    /// The wire value: `"buy"` or `"sell"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            TradeSide::Buy => "buy",
            TradeSide::Sell => "sell",
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
/// account indices as strings. A row backfilled from the venue's finalized
/// export (on Robinhood Chain, the span before live capture) has `source`
/// `"bucket"` and an empty `raw_json`; a row captured live has `source`
/// `"ws"` and the venue's raw JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterLiquidation {
    /// Market symbol, uppercase for perps (`BTC`).
    pub symbol: String,
    /// Trade time, as an RFC 3339 UTC string with milliseconds.
    pub timestamp: String,
    /// Trade time in Unix milliseconds.
    pub timestamp_ms: i64,
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
    /// the venue's finalized export (`source` `"bucket"`).
    #[serde(default)]
    pub raw_json: String,
    /// Where the row came from: `"ws"` for the live capture, `"bucket"` for
    /// rows backfilled from the venue's finalized export (on Robinhood Chain,
    /// the span before live capture).
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
    /// Bucket start, as an RFC 3339 UTC string with milliseconds.
    pub timestamp: String,
    /// Bucket start in Unix milliseconds.
    pub timestamp_ms: i64,
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
    OneMinute,
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
            OiFundingInterval::OneMinute => "1m",
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

/// Lighter orderbook snapshot granularity.
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
    /// Always `None` in live messages. Fees are on the finalized REST record,
    /// and replayed rows from it carry them.
    pub fee: Option<String>,
    /// Always `None` in live messages.
    pub fee_token: Option<String>,
    /// Always `None` in live messages; set on replayed rows from the
    /// finalized record.
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
/// when the `websocket` feature is enabled. Replay rows (`historical_data` and
/// `replay_snapshot`) have the same shapes under the API version the SDK
/// requests, so they decode alike; a replayed trades row is a single fill.
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
    /// Decode the `data` field of a live `data` message or of a replay row
    /// (`historical_data`, `replay_snapshot`).
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
    /// Canonical symbol of the market (the same value as `coin` on most
    /// venues).
    #[serde(default)]
    pub symbol: Option<String>,
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
    /// Paging totals: `total` incidents matching the filters, and the
    /// `limit` and `offset` applied.
    #[serde(default)]
    pub pagination: Option<IncidentsPagination>,
}

/// Paging totals of an incident listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentsPagination {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
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
#[serde(from = "RawL4OrderEntry")]
pub struct L4OrderEntry {
    pub oid: u64,
    pub user_address: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    /// When the order took its place in the queue, as an RFC 3339 UTC string
    /// with milliseconds. `None` when the queue time is unknown or the row
    /// does not carry it.
    pub timestamp: Option<String>,
    /// `timestamp` in Unix milliseconds.
    pub timestamp_ms: Option<i64>,
}

/// Wire form of [`L4OrderEntry`]: the queue time arrives as an RFC 3339
/// string with `timestamp_ms` on current and point-in-time snapshots, and as
/// integer milliseconds on checkpoint history rows.
#[derive(Deserialize)]
struct RawL4OrderEntry {
    oid: u64,
    user_address: String,
    side: String,
    price: f64,
    size: f64,
    #[serde(default)]
    timestamp: Option<serde_json::Value>,
    #[serde(default)]
    timestamp_ms: Option<i64>,
}

impl From<RawL4OrderEntry> for L4OrderEntry {
    fn from(raw: RawL4OrderEntry) -> Self {
        let (timestamp, timestamp_ms) = match raw.timestamp {
            Some(serde_json::Value::String(s)) => {
                let ms = raw.timestamp_ms.or_else(|| parse_timestamp_str(&s));
                (Some(s), ms)
            }
            Some(serde_json::Value::Number(n)) => match n.as_i64().filter(|ms| *ms > 0) {
                // Zero means the queue time is unknown.
                Some(ms) => (rfc3339_millis(ms), Some(ms)),
                None => (None, None),
            },
            _ => (None, None),
        };
        L4OrderEntry {
            oid: raw.oid,
            user_address: raw.user_address,
            side: raw.side,
            price: raw.price,
            size: raw.size,
            timestamp,
            timestamp_ms,
        }
    }
}

/// Unix milliseconds as an RFC 3339 UTC string with milliseconds, the form
/// the API writes.
fn rfc3339_millis(ms: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
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
    /// UTC snapshot time the levels reflect, as an RFC 3339 string with
    /// milliseconds.
    pub snapshot_ts: String,
    /// `snapshot_ts` in Unix milliseconds.
    #[serde(default)]
    pub snapshot_ts_ms: Option<i64>,
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
    /// UTC snapshot time, as an RFC 3339 string with milliseconds.
    pub snapshot_ts: String,
    /// `snapshot_ts` in Unix milliseconds.
    #[serde(default)]
    pub snapshot_ts_ms: Option<i64>,
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
    /// UTC snapshot time, as an RFC 3339 string with milliseconds.
    pub snapshot_ts: String,
    /// `snapshot_ts` in Unix milliseconds.
    #[serde(default)]
    pub snapshot_ts_ms: Option<i64>,
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
    /// Time in force. Empty on rows that carry none, such as `triggered`
    /// rows.
    #[serde(default)]
    pub tif: String,
    pub reduce_only: bool,
    pub is_trigger: bool,
    pub is_position_tpsl: bool,
    pub cloid: Option<String>,
    /// Trigger condition of a trigger order, on `triggered` rows.
    #[serde(default)]
    pub trigger_condition: Option<String>,
    /// Trigger price of a trigger order, on `triggered` rows.
    #[serde(default)]
    pub trigger_price: Option<f64>,
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
    /// `cross`, `isolated` or `unknown` (always `unknown` on reconstructed
    /// Hyperliquid rows). Lighter reports its margin mode here.
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
    /// `cross`, `isolated` or `unknown` (Lighter: its margin mode).
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

/// Freshness of the account positions data of one venue, from
/// `client.data_quality.positions_freshness()`.
///
/// One row per venue: Hyperliquid core (`venue` `hyperliquid`, `product`
/// `core`), HIP-3 (`hyperliquid`, `hip3`), Lighter (`lighter`, `lighter`) and
/// Lighter on Robinhood Chain (`rh_lighter`, `rh_lighter`). Instants are
/// RFC 3339 UTC strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionsFreshness {
    /// `hyperliquid`, `lighter` or `rh_lighter`.
    pub venue: String,
    /// `core`, `hip3`, `lighter` or `rh_lighter`.
    pub product: String,
    /// Time of the latest live snapshot.
    #[serde(default)]
    pub live_snapshot_ts: Option<String>,
    /// Age of the latest live snapshot, in seconds.
    #[serde(default)]
    pub live_age_seconds: Option<i64>,
    /// `true` when the latest live snapshot is older than 12 minutes (or
    /// there is none).
    pub stale: bool,
    /// Quality of the latest live snapshot: `complete`, `partial` or
    /// `degraded`.
    #[serde(default)]
    pub live_quality: Option<String>,
    /// Hour of the latest hourly snapshot.
    #[serde(default)]
    pub hourly_snapshot_ts: Option<String>,
    /// Every event before this instant is built into the change log and the
    /// as-of state.
    #[serde(default)]
    pub built_through: Option<String>,
    /// Every event before this instant is final and will not be re-derived.
    #[serde(default)]
    pub finalized_through: Option<String>,
}

// ---------------------------------------------------------------------------
// Public symbol universe
// ---------------------------------------------------------------------------

/// One market in the public symbol universe, from `client.symbols()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolEntry {
    /// Symbol in the venue family's own form (`BTC`, `km:US500`,
    /// `HYPE-USDC`, `#0`).
    pub symbol: String,
    /// Venue family: `hyperliquid`, `hip3`, `hip4`, `spot`, `lighter` or
    /// `rh-lighter`.
    pub exchange: String,
    /// Earliest coverage across the symbol's data types.
    #[serde(default)]
    pub coverage_from: Option<String>,
    /// Latest coverage, when the market has stopped trading or settled.
    #[serde(default)]
    pub coverage_to: Option<String>,
    /// Data types served for the symbol.
    #[serde(default)]
    pub data_types: Vec<String>,
    /// Coverage start for each data type.
    #[serde(default)]
    pub coverage_by_type: std::collections::BTreeMap<String, String>,
    /// Estimated data size per day, by data type.
    #[serde(default)]
    pub size_per_day: std::collections::BTreeMap<String, f64>,
    /// HIP-4 slug, when available.
    #[serde(default)]
    pub slug: Option<String>,
    /// HIP-4 pair of side coins, for example `["#0", "#1"]`.
    #[serde(default)]
    pub outcome_pair: Option<Vec<String>>,
    /// HIP-4 human-readable title.
    #[serde(default)]
    pub display_title: Option<String>,
    /// HIP-4 settlement state.
    #[serde(default)]
    pub is_settled: Option<bool>,
    /// Whether the market is active, when the API reports it.
    #[serde(default)]
    pub is_active: Option<bool>,
    /// Fields the API sends that this release does not name.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Body of `GET /v1/symbols` (internal use). With the API version the SDK
/// sends, the symbols arrive as the envelope's `data` array; the older body
/// carried them in a `symbols` member.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum SymbolsResponse {
    List(Vec<SymbolEntry>),
    Legacy { symbols: Vec<SymbolEntry> },
}

impl SymbolsResponse {
    pub(crate) fn into_symbols(self) -> Vec<SymbolEntry> {
        match self {
            SymbolsResponse::List(symbols) | SymbolsResponse::Legacy { symbols } => symbols,
        }
    }
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// One venue and datatype the API serves, from `client.capabilities()`
/// (`GET /v1/capabilities`).
///
/// A row lists the REST routes and WebSocket channels for the datatype on
/// that venue, whether the channels stream live and replay history, the
/// first instant served, the cadence, the largest page and the accepted
/// intervals. It is the source for which channels can be replayed: see
/// [`Capability::for_channel`].
///
/// New fields may be added in minor releases, so the struct cannot be built
/// with a literal outside this crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Capability {
    /// `hyperliquid`, `hip3`, `hip4`, `spot`, `lighter` or `rh-lighter`.
    pub venue: String,
    /// Datatype id, for example `trades`, `l4_diffs` or `l2_full_depth`.
    pub datatype: String,
    /// REST route templates, for example `/v1/hyperliquid/trades/{symbol}`.
    #[serde(default)]
    pub rest_routes: Vec<String>,
    /// WebSocket channels for this venue and datatype.
    #[serde(default)]
    pub ws_channels: Vec<String>,
    /// `true` when a WebSocket subscription streams it live.
    #[serde(default)]
    pub live: bool,
    /// `true` when a WebSocket replay serves its history.
    #[serde(default)]
    pub replay: bool,
    /// First instant served, as an RFC 3339 UTC string, when the datatype has
    /// a fixed start.
    #[serde(default)]
    pub available_from: Option<String>,
    /// `event`, `snapshot`, `sample`, `interval` or `reference`.
    #[serde(default)]
    pub cadence: Option<String>,
    /// Largest `limit` one page accepts.
    #[serde(default)]
    pub page_limit: Option<u64>,
    /// Values `interval` accepts, empty when the datatype has none.
    #[serde(default)]
    pub intervals: Vec<String>,
    /// Further detail, such as how replay behaves.
    #[serde(default)]
    pub notes: Option<String>,
}

impl Capability {
    /// Whether this row lists the WebSocket `channel`.
    pub fn has_channel(&self, channel: &str) -> bool {
        self.ws_channels.iter().any(|c| c == channel)
    }

    /// The row that lists the WebSocket `channel`, if any.
    ///
    /// ```no_run
    /// # use oxarchive::{Capability, OxArchive};
    /// # async fn example() -> oxarchive::Result<()> {
    /// # let client = OxArchive::new("key")?;
    /// let rows = client.capabilities().await?;
    /// let replayable = Capability::for_channel(&rows, "spot_l4_diffs").map(|row| row.replay);
    /// println!("spot_l4_diffs replay: {replayable:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn for_channel<'a>(rows: &'a [Capability], channel: &str) -> Option<&'a Capability> {
        rows.iter().find(|row| row.has_channel(channel))
    }
}

// ---------------------------------------------------------------------------
// Cumulative volume delta (Hyperliquid core and HIP-3)
// ---------------------------------------------------------------------------

/// One cumulative volume delta bucket, from `client.hyperliquid.cvd()` or
/// `client.hyperliquid.hip3.cvd()`.
///
/// A bucket is labelled by its open time in UTC and is omitted when it holds
/// no trades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CvdBucket {
    /// Bucket open time, as an RFC 3339 UTC string with milliseconds.
    pub timestamp: String,
    /// Bucket open time, in Unix milliseconds.
    pub timestamp_ms: i64,
    /// Taker buy notional in the bucket.
    pub buy_volume: f64,
    /// Taker sell notional in the bucket.
    pub sell_volume: f64,
    /// `buy_volume` minus `sell_volume`.
    pub delta: f64,
    /// Running total of `delta` from the first bucket of this response. It
    /// restarts on every page, so rebuild it from `delta` when joining pages.
    pub cumulative_delta: f64,
}

// ---------------------------------------------------------------------------
// HIP-3 oracle
// ---------------------------------------------------------------------------

/// The latest deployer-pushed external price and mark price of a HIP-3
/// market, from `client.hyperliquid.hip3.oracle.external_price()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hip3OracleExternalPrice {
    /// HIP-3 symbol, with its builder prefix (`xyz:XYZ100`).
    pub symbol: String,
    /// Externally derived reference price, when available.
    #[serde(default)]
    pub external_price: Option<f64>,
    /// On-chain mark input.
    #[serde(default)]
    pub mark_price: Option<f64>,
    /// Source block number.
    pub block_number: i64,
    /// Source timestamp, as an RFC 3339 UTC string with milliseconds.
    pub timestamp: String,
    /// Source timestamp, in Unix milliseconds.
    pub timestamp_ms: i64,
}

/// Instantaneous discovery bounds of a HIP-3 market, from
/// `client.hyperliquid.hip3.oracle.discovery_bounds()`.
///
/// The bounds are `reference_price` times `1 - bound_fraction` and
/// `1 + bound_fraction`, with the fraction derived from the market's maximum
/// leverage. The full ratcheted range can be wider when deployer-specific
/// reset configuration applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hip3OracleDiscoveryBounds {
    /// HIP-3 symbol, with its builder prefix.
    pub symbol: String,
    /// The external price when available, otherwise the mark price.
    pub reference_price: f64,
    /// Price source used for the bounds: `external` or `mark`.
    pub reference_source: String,
    /// Market maximum leverage used for the bound fraction.
    pub max_leverage: i64,
    /// Fraction applied on each side of `reference_price`.
    pub bound_fraction: f64,
    /// Instantaneous lower discovery bound.
    pub lower_bound: f64,
    /// Instantaneous upper discovery bound.
    pub upper_bound: f64,
    /// Source block number.
    pub block_number: i64,
    /// Source timestamp, as an RFC 3339 UTC string with milliseconds.
    pub timestamp: String,
    /// Source timestamp, in Unix milliseconds.
    pub timestamp_ms: i64,
}

// ---------------------------------------------------------------------------
// HIP-4 questions
// ---------------------------------------------------------------------------

/// A HIP-4 question: a multi-choice resolver that groups binary outcome
/// markets under one ballot.
///
/// Each named choice is its own outcome, and the fallback outcome resolves
/// Yes when no named choice does. Look outcomes up with
/// `client.hyperliquid.hip4.get_outcome()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hip4Question {
    /// Question identifier.
    pub question_id: i64,
    /// Question name as published on-chain. Recurring markets use a generic
    /// name such as `Recurring`.
    pub name: String,
    /// Pipe-delimited question metadata: class, underlying, expiry, price
    /// thresholds and period.
    pub description: String,
    /// Outcome that resolves Yes when no named outcome does.
    pub fallback_outcome_id: i64,
    /// Outcomes of the named choices grouped under this question.
    #[serde(default)]
    pub named_outcome_ids: Vec<i64>,
    /// The subset of `named_outcome_ids` that has already settled.
    #[serde(default)]
    pub settled_named_outcomes: Vec<i64>,
    /// When the question was first observed (UTC, RFC 3339).
    pub first_seen_at: String,
    /// When the question was last updated (UTC, RFC 3339).
    pub last_updated_at: String,
}

// ---------------------------------------------------------------------------
// Wallet classification (Hyperliquid core and HIP-3)
// ---------------------------------------------------------------------------

/// One page of wallet classification results, from
/// `client.hyperliquid.wallets.classify()` or
/// `client.hyperliquid.hip3.wallets.classify()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletClassification {
    /// Wallets on this page, in the requested sort order.
    #[serde(default)]
    pub wallets: Vec<ClassifiedWallet>,
    /// Wallets matching the filters, across every page.
    pub total: i64,
    /// The daily snapshot date the metrics describe (`YYYY-MM-DD`).
    pub date: String,
}

/// A wallet with its precomputed behavior metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifiedWallet {
    /// Wallet address.
    pub address: String,
    /// Behavior metrics over `period`.
    pub metrics: WalletMetrics,
    /// Metric lookback period, for example `24h`.
    pub period: String,
}

/// Precomputed behavior metrics of one wallet.
///
/// Every field is optional: a metric the API does not send for a wallet is
/// `None`. Ratios and rates are fractions from 0 to 1. Metrics the API adds
/// after this release are kept in `extra`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WalletMetrics {
    /// Orders placed.
    #[serde(default)]
    pub total_orders: Option<i64>,
    /// Share of orders that were canceled.
    #[serde(default)]
    pub cancel_rate: Option<f64>,
    /// Share of orders that filled.
    #[serde(default)]
    pub fill_rate: Option<f64>,
    /// Orders placed per fill.
    #[serde(default)]
    pub order_to_trade_ratio: Option<f64>,
    /// Share of orders sent immediate-or-cancel.
    #[serde(default)]
    pub ioc_ratio: Option<f64>,
    /// Share of orders sent post-only.
    #[serde(default)]
    pub post_only_ratio: Option<f64>,
    /// Share of orders that were take-profit or stop-loss orders.
    #[serde(default)]
    pub tpsl_ratio: Option<f64>,
    /// Share of orders that were trigger orders.
    #[serde(default)]
    pub trigger_order_ratio: Option<f64>,
    /// Distinct coins the wallet placed orders on.
    #[serde(default)]
    pub unique_coins_traded: Option<i64>,
    /// Whether the wallet placed take-profit or stop-loss orders.
    #[serde(default)]
    pub uses_tpsl: Option<bool>,
    /// Whether the wallet routed orders through a builder.
    #[serde(default)]
    pub uses_builder: Option<bool>,
    /// The builder the wallet used most, when it used one.
    #[serde(default)]
    pub top_builder: Option<String>,
    /// Mean order size, in USD.
    #[serde(default)]
    pub avg_order_size_usd: Option<f64>,
    /// Largest order size, in USD.
    #[serde(default)]
    pub max_order_size_usd: Option<f64>,
    /// Typical time from placement to cancel, in milliseconds.
    #[serde(default)]
    pub median_cancel_speed_ms: Option<f64>,
    /// Distinct hours in which the wallet was active.
    #[serde(default)]
    pub active_hours: Option<i64>,
    /// Fills received.
    #[serde(default)]
    pub total_fills: Option<i64>,
    /// Filled volume, in USD.
    #[serde(default)]
    pub total_volume_usd: Option<f64>,
    /// Share of fills on the maker side.
    #[serde(default)]
    pub maker_ratio: Option<f64>,
    /// Buy volume divided by sell volume.
    #[serde(default)]
    pub long_short_ratio: Option<f64>,
    /// Buy volume, in USD.
    #[serde(default)]
    pub buy_volume_usd: Option<f64>,
    /// Sell volume, in USD.
    #[serde(default)]
    pub sell_volume_usd: Option<f64>,
    /// Fees paid, in USD. Negative when rebates exceeded fees.
    #[serde(default)]
    pub total_fees_usd: Option<f64>,
    /// Realized profit and loss, in USD.
    #[serde(default)]
    pub realized_pnl_usd: Option<f64>,
    /// Liquidation fills.
    #[serde(default)]
    pub liquidation_count: Option<i64>,
    /// Largest single fill, in USD.
    #[serde(default)]
    pub max_single_fill_usd: Option<f64>,
    /// Distinct coins the wallet received fills on.
    #[serde(default)]
    pub unique_fill_coins: Option<i64>,
    /// Whether the wallet received TWAP fills.
    #[serde(default)]
    pub uses_twap: Option<bool>,
    /// Share of fills that came from TWAP orders.
    #[serde(default)]
    pub twap_fill_ratio: Option<f64>,
    /// Whether the wallet set client order ids.
    #[serde(default)]
    pub uses_cloid: Option<bool>,
    /// Share of orders with a client order id.
    #[serde(default)]
    pub cloid_ratio: Option<f64>,
    /// Whether the wallet paid priority gas.
    #[serde(default)]
    pub uses_priority_gas: Option<bool>,
    /// Priority gas paid.
    #[serde(default)]
    pub total_priority_gas_paid: Option<f64>,
    /// Builder fees paid.
    #[serde(default)]
    pub total_builder_fees_paid: Option<f64>,
    /// Metrics this release does not name yet.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Webhooks
// ---------------------------------------------------------------------------
//
// Instants are RFC 3339 UTC strings. Identifiers are UUID strings.

/// Deserialize `null` (or an absent field, with `#[serde(default)]`) as the
/// type's default.
fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One entry of the served event catalog, from
/// `client.webhooks.event_types()`.
///
/// Subscriptions are validated against this declaration, so it is the
/// authority on the filters, parameters and metrics an event type accepts.
/// Read `params` and `metrics` from here rather than hardcoding them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEventType {
    /// Event type identifier, for example `market.liquidation`. Send it as
    /// `event_type` when creating a subscription.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Version of the delivered payload shape for this event type.
    pub schema_version: i64,
    /// `true` when the type accepts subscriptions. A type that is published
    /// but not yet live is refused at create time.
    pub live: bool,
    /// Who the event is about: `public` (market wide), `user` (your own
    /// account and platform activity) or `addresses` (only wallets on your
    /// watched list).
    pub scope: String,
    /// Venues the type covers. Empty when the type is not venue scoped.
    #[serde(default)]
    pub venues: Vec<String>,
    /// Filter keys the subscription config accepts for this type, from
    /// `venue`, `symbols` and `addresses`. A key not listed is refused.
    #[serde(default)]
    pub filters: Vec<String>,
    /// Tunable parameters, keyed by parameter name.
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, WebhookEventTypeParam>,
    /// Metrics the event carries, keyed by metric name. A condition may only
    /// reference a metric declared here.
    #[serde(default)]
    pub metrics: std::collections::BTreeMap<String, WebhookEventTypeMetric>,
    /// The smallest occurrence the type reports at all. `None` when the type
    /// has no floor.
    #[serde(default)]
    pub cost_floor: Option<WebhookCostFloor>,
    /// Rough delivery latency from the occurrence to the first delivery
    /// attempt: `seconds` or `minutes`.
    pub latency_class: String,
    /// What the event reports and what one occurrence means.
    pub description: String,
    /// A worked example of a config for this type.
    #[serde(default)]
    pub filters_example: Option<serde_json::Value>,
    /// Operator vocabulary grouped by metric type, plus the `any` group that
    /// applies to every metric.
    #[serde(default)]
    pub operators: std::collections::BTreeMap<String, Vec<String>>,
}

/// A tunable parameter declared by an event type.
///
/// Values sent under `params` are validated against this declaration, and a
/// declared parameter left unset is stored at its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEventTypeParam {
    /// Value type: `integer`, `number`, `string` or `array_of_number`.
    #[serde(rename = "type")]
    pub value_type: String,
    /// Unit the value is expressed in, when it has one.
    #[serde(default)]
    pub unit: Option<String>,
    /// Value used when the parameter is not supplied.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Lowest accepted value, when the parameter is bounded below.
    #[serde(default)]
    pub min: Option<f64>,
    /// Highest accepted value, when the parameter is bounded above.
    #[serde(default)]
    pub max: Option<f64>,
    /// Accepted values, when the parameter is a fixed choice. Sent on the
    /// wire as `enum`.
    #[serde(default, rename = "enum")]
    pub allowed: Option<Vec<serde_json::Value>>,
    /// What the parameter changes.
    #[serde(default)]
    pub description: Option<String>,
}

/// A metric an event carries, and so a metric a condition may be written
/// against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEventTypeMetric {
    /// Value type of the metric, for example `number`, `integer`, `string`,
    /// `enum`, `boolean` or `timestamp`. It decides which operators a
    /// condition on the metric may use.
    #[serde(rename = "type")]
    pub value_type: String,
    /// Unit the metric is expressed in, when it has one.
    #[serde(default)]
    pub unit: Option<String>,
    /// Accepted values, when the metric is a fixed choice.
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// What the metric measures, including when it is null.
    #[serde(default)]
    pub description: Option<String>,
}

/// The smallest occurrence an event type reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookCostFloor {
    /// Metric the floor applies to, for example `notional_usd`.
    pub metric: String,
    /// Lowest value still reported.
    pub min: f64,
    /// Unit of `min`, when the API sends one.
    #[serde(default)]
    pub unit: Option<String>,
    /// How the floor relates to the default subscription threshold, when the
    /// API sends one.
    #[serde(default)]
    pub note: Option<String>,
}

/// A delivery destination, from `client.webhooks.list_endpoints()`.
///
/// The signing secret is never part of this shape. It is returned once by
/// [`create_endpoint`](crate::resources::webhooks::WebhooksResource::create_endpoint)
/// and again by
/// [`rotate_secret`](crate::resources::webhooks::WebhooksResource::rotate_secret).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    /// Endpoint identifier.
    pub id: String,
    /// HTTPS destination that receives deliveries.
    pub url: String,
    /// Your own label for the endpoint.
    #[serde(default)]
    pub description: String,
    /// `active`, `disabled` (switched off by you) or `auto_disabled`
    /// (switched off after a long run of failed deliveries; bring it back
    /// with `enable_endpoint`).
    pub status: String,
    /// Failed attempts since the last success.
    #[serde(default)]
    pub consecutive_failures: i32,
    /// When the endpoint was created.
    pub created_at: String,
    /// Fields the API sends that this release does not name.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A newly created endpoint with its signing secret, from
/// `client.webhooks.create_endpoint()`.
///
/// This and a rotation are the only responses that carry a secret. Store
/// `secret` before doing anything else: it is not shown again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEndpointCreated {
    /// Endpoint identifier.
    pub id: String,
    /// HTTPS destination that receives deliveries.
    pub url: String,
    /// Your own label for the endpoint.
    #[serde(default)]
    pub description: String,
    /// `active`, `disabled` or `auto_disabled`.
    pub status: String,
    /// Failed attempts since the last success.
    #[serde(default)]
    pub consecutive_failures: i32,
    /// When the endpoint was created.
    pub created_at: String,
    /// Signing secret for this endpoint. Every delivery is signed with it.
    pub secret: String,
    /// The API's reminder that the secret is shown once.
    #[serde(default)]
    pub note: Option<String>,
    /// Fields the API sends that this release does not name.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A freshly rotated signing secret, from `client.webhooks.rotate_secret()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEndpointSecret {
    /// The new signing secret. The previous one keeps verifying for 24
    /// hours, and every delivery in that window carries a signature for
    /// each.
    pub secret: String,
    /// How long the previous secret keeps verifying.
    #[serde(default)]
    pub note: Option<String>,
}

/// Venue filter of a subscription config: one venue, or several.
///
/// A single string may also hold several venues separated by `|`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebhookVenueFilter {
    /// One venue, for example `hyperliquid`.
    One(String),
    /// Several venues.
    Many(Vec<String>),
}

/// What a subscription matches on.
///
/// Every key is checked against the event type's catalog declaration, so an
/// unknown key, an undeclared parameter or a condition on an undeclared
/// metric is refused rather than ignored. The config the API stores and
/// returns is the normalized form: venues and addresses lowercased, declared
/// parameters filled in at their defaults, and operators in their canonical
/// spelling.
///
/// ```
/// use oxarchive::types::{WebhookSubscriptionCondition, WebhookSubscriptionConfig};
///
/// let config = WebhookSubscriptionConfig::default()
///     .venue("hyperliquid")
///     .symbols(["BTC", "ETH"])
///     .condition(WebhookSubscriptionCondition::new(
///         "notional_usd",
///         "greater_than_or_equal",
///         250_000,
///     ));
/// assert_eq!(config.conditions.len(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebhookSubscriptionConfig {
    /// Venue or venues to match, from the event type's `venues`. `None`
    /// matches every covered venue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<WebhookVenueFilter>,
    /// Instrument symbols to match. `None` matches every symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
    /// Wallets to match, for an address scoped event type. Each one must
    /// already be on your watched list. `None` matches all of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<String>>,
    /// Parameter values, keyed by the parameter names the event type
    /// declares. A declared parameter left out is stored at its default.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub params: std::collections::BTreeMap<String, serde_json::Value>,
    /// Conditions on the event's metrics, all of which must hold for a
    /// delivery. At most 16.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<WebhookSubscriptionCondition>,
    /// Shorthand for a `notional_usd` at-or-above condition, kept for
    /// compatibility. It is stored as a condition and mirrored back here as
    /// the loosest notional lower bound the config carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_notional_usd: Option<f64>,
    /// Other top-level keys. A declared parameter may also be written at the
    /// top level of the config, and the API may add keys in later releases.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

impl WebhookSubscriptionConfig {
    /// Match one venue, or several written as `a|b`.
    pub fn venue(mut self, venue: impl Into<String>) -> Self {
        self.venue = Some(WebhookVenueFilter::One(venue.into()));
        self
    }

    /// Match several venues.
    pub fn venues<I, S>(mut self, venues: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.venue = Some(WebhookVenueFilter::Many(
            venues.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// Match these instrument symbols.
    pub fn symbols<I, S>(mut self, symbols: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.symbols = Some(symbols.into_iter().map(Into::into).collect());
        self
    }

    /// Match these watched wallets.
    pub fn addresses<I, S>(mut self, addresses: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.addresses = Some(addresses.into_iter().map(Into::into).collect());
        self
    }

    /// Set one declared parameter.
    pub fn param(mut self, name: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.params.insert(name.into(), value.into());
        self
    }

    /// Add a condition.
    pub fn condition(mut self, condition: WebhookSubscriptionCondition) -> Self {
        self.conditions.push(condition);
        self
    }
}

/// One condition on an event metric.
///
/// The operator vocabulary is grouped by the metric's type; see
/// [`WebhookEventType::operators`]. Symbol spellings such as `>=` are
/// accepted and stored in the canonical spelling (`greater_than_or_equal`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookSubscriptionCondition {
    /// A metric declared by the event type.
    pub metric: String,
    /// Comparison to apply, for example `greater_than_or_equal`, `between`,
    /// `in` or `is_empty`.
    pub op: String,
    /// What to compare against: a single value, a `[low, high]` pair for
    /// `between` and `not_between`, or a non-empty list for `in` and
    /// `not_in`. `None` for `is_empty` and `is_not_empty`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

impl WebhookSubscriptionCondition {
    /// A condition that compares `metric` against `value`.
    pub fn new(
        metric: impl Into<String>,
        op: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        Self {
            metric: metric.into(),
            op: op.into(),
            value: Some(value.into()),
        }
    }

    /// A condition without a value, for `is_empty` and `is_not_empty`.
    pub fn without_value(metric: impl Into<String>, op: impl Into<String>) -> Self {
        Self {
            metric: metric.into(),
            op: op.into(),
            value: None,
        }
    }
}

/// A rule: one event type and one config, delivered to one endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookSubscription {
    /// Subscription identifier.
    pub id: String,
    /// Endpoint that receives this rule's deliveries.
    pub endpoint_id: String,
    /// Event type the rule subscribes to.
    pub event_type: String,
    /// The stored, normalized config.
    #[serde(default, deserialize_with = "null_as_default")]
    pub filters: WebhookSubscriptionConfig,
    /// Your own on and off switch. Resuming a paused rule never changes it.
    pub enabled: bool,
    /// When the rule was created.
    pub created_at: String,
    /// `active` while serving, `auto_paused` while delivery is paused. A
    /// pause at the daily delivery limit lasts until the rule is resumed.
    #[serde(default)]
    pub status: String,
    /// Why the rule is paused and what clears it, in plain words. Present
    /// only while it is paused. This is the field to show a person.
    #[serde(default)]
    pub pause_message: Option<String>,
    /// Start of the current pause. `None` while serving.
    #[serde(default)]
    pub paused_at: Option<String>,
    /// Machine readable cause of the current pause:
    /// `deliveries_per_day_cap` or `plan_no_webhooks`. `None` while serving.
    #[serde(default)]
    pub pause_reason: Option<String>,
    /// Matches observed but not delivered since the current pause began. A
    /// lower bound, not a total.
    #[serde(default)]
    pub suppressed_count: i64,
    /// First suppressed match of the current pause.
    #[serde(default)]
    pub suppressed_first_at: Option<String>,
    /// Most recent suppressed match of the current pause.
    #[serde(default)]
    pub suppressed_last_at: Option<String>,
    /// Start of the last pause that has already ended.
    #[serde(default)]
    pub last_paused_at: Option<String>,
    /// When that pause ended.
    #[serde(default)]
    pub last_resumed_at: Option<String>,
    /// Cause of the last pause that has already ended.
    #[serde(default)]
    pub last_pause_reason: Option<String>,
    /// Matches suppressed during the last pause that has already ended.
    #[serde(default)]
    pub last_suppressed_count: i64,
    /// First suppressed match of that pause.
    #[serde(default)]
    pub last_suppressed_first_at: Option<String>,
    /// Most recent suppressed match of that pause.
    #[serde(default)]
    pub last_suppressed_last_at: Option<String>,
    /// Fields the API sends that this release does not name.
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

impl WebhookSubscription {
    /// `true` while delivery is paused (`status` is `auto_paused`).
    pub fn is_paused(&self) -> bool {
        self.status == "auto_paused"
    }
}

/// The window a resume closed.
///
/// Nothing is buffered while a rule is paused, so this describes what was
/// missed rather than replaying it. Re-read `replay_window` from the REST
/// routes to recover it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookResumeGap {
    /// Start of the gap.
    #[serde(default)]
    pub paused_at: Option<String>,
    /// End of the gap.
    #[serde(default)]
    pub resumed_at: Option<String>,
    /// The gap as a window to re-read.
    #[serde(default, deserialize_with = "null_as_default")]
    pub replay_window: WebhookReplayWindow,
    /// Cause of the pause that was cleared. `None` on a bulk resume whose
    /// rules were paused for more than one reason.
    #[serde(default)]
    pub reason: Option<String>,
    /// Distinct causes across the resumed rules. Sent on a bulk resume only.
    #[serde(default)]
    pub reasons: Option<Vec<String>>,
    /// The cause in plain words.
    #[serde(default)]
    pub pause_message: Option<String>,
    /// Matches suppressed inside the window. `None` when the rules were
    /// address scoped, because their occurrences were never looked at.
    #[serde(default)]
    pub suppressed_count: Option<i64>,
    /// Whether anything inside the window was counted. `false` means the
    /// count is `None` because nothing was looked at, not because nothing
    /// happened.
    #[serde(default)]
    pub counted: bool,
    /// First suppressed match inside the window.
    #[serde(default)]
    pub suppressed_first_at: Option<String>,
    /// Most recent suppressed match inside the window.
    #[serde(default)]
    pub suppressed_last_at: Option<String>,
    /// How many of the resumed rules were address scoped, and so not
    /// counted. Sent on a bulk resume only.
    #[serde(default)]
    pub uncounted_subscriptions: Option<i64>,
    /// What can and cannot be recovered for the window, and how.
    #[serde(default)]
    pub note: String,
}

/// A window to re-read from the REST routes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebhookReplayWindow {
    /// Window start.
    #[serde(default)]
    pub start: Option<String>,
    /// Window end.
    #[serde(default)]
    pub end: Option<String>,
}

/// The result of resuming one subscription, from
/// `client.webhooks.resume_subscription()`.
#[derive(Debug, Clone, PartialEq)]
pub struct WebhookResume {
    /// The subscription after the call.
    pub subscription: WebhookSubscription,
    /// The window the resume closed. `None` when the rule was already
    /// serving, in which case nothing changed.
    pub gap: Option<WebhookResumeGap>,
    /// Why nothing changed, when nothing did.
    pub note: Option<String>,
}

/// The result of resuming every paused subscription, from
/// `client.webhooks.resume_all_subscriptions()`.
#[derive(Debug, Clone, PartialEq)]
pub struct WebhookResumeAll {
    /// The rules that were put back into service.
    pub subscriptions: Vec<WebhookSubscription>,
    /// How many rules were put back into service.
    pub resumed_count: i64,
    /// The window the resume closed, across the rules it cleared. `None`
    /// when nothing was paused.
    pub gap: Option<WebhookResumeGap>,
    /// Why nothing changed, when nothing did.
    pub note: Option<String>,
}

/// One delivery record for one event to one endpoint.
///
/// An event and an endpoint share a single record for their whole life, so
/// a repeat delivery rewrites this record rather than adding another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookDelivery {
    /// Delivery identifier.
    pub id: String,
    /// Event identifier, stable across retries and repeat deliveries.
    /// Deduplicate on it in your receiver.
    pub event_id: String,
    /// Event type delivered.
    pub event_type: String,
    /// `pending`, `delivered`, `failed` or `exhausted`.
    pub state: String,
    /// Attempts made so far.
    pub attempts: i32,
    /// HTTP status your receiver returned on the last attempt.
    #[serde(default)]
    pub last_status_code: Option<i32>,
    /// Why the last attempt failed. `None` when it succeeded.
    #[serde(default)]
    pub last_error: Option<String>,
    /// How long the last attempt took, in milliseconds.
    #[serde(default)]
    pub last_latency_ms: Option<i32>,
    /// When the next attempt is due.
    pub next_attempt_at: String,
    /// When the delivery landed. `None` until it does.
    #[serde(default)]
    pub delivered_at: Option<String>,
    /// When the delivery was queued. A repeat delivery resets it.
    pub created_at: String,
    /// The event body as sent: `id`, `type`, `schema_version`,
    /// `observed_at` and `data`.
    pub payload: serde_json::Value,
}

/// A delivery that has just been queued, from
/// `client.webhooks.test_endpoint()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookDeliveryQueued {
    /// Delivery identifier. Read it back from the endpoint's delivery log.
    pub delivery_id: String,
    /// Event identifier carried in the delivered payload.
    pub event_id: String,
}

/// A past delivery queued for another attempt, from
/// `client.webhooks.redeliver()`.
///
/// The identifiers are unchanged because the delivery is re-queued in place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookRedelivery {
    /// Delivery identifier, the one that was asked for.
    pub delivery_id: String,
    /// Event identifier, unchanged, so a receiver that already processed the
    /// event can deduplicate on it.
    pub event_id: String,
    /// Event type being delivered again.
    pub event_type: String,
    /// `pending` immediately after the repeat delivery is queued.
    pub state: String,
    /// Attempt counter, restarted from zero.
    pub attempts: i32,
    /// When the attempt is due, which is immediately.
    pub next_attempt_at: String,
    /// What the repeat delivery rewrites on the delivery record.
    #[serde(default)]
    pub note: Option<String>,
}

/// A wallet on your watched list. Address scoped event types report only on
/// these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookWatchedAddress {
    /// Watched address identifier.
    pub id: String,
    /// The wallet, stored lowercase.
    pub address: String,
    /// Your own label for the wallet. At most 64 characters.
    #[serde(default)]
    pub label: String,
    /// When the wallet was added.
    pub created_at: String,
}

/// Watched wallets with the plan's cap, from
/// `client.webhooks.list_addresses()`.
#[derive(Debug, Clone, PartialEq)]
pub struct WebhookWatchedAddressList {
    /// The watched wallets.
    pub addresses: Vec<WebhookWatchedAddress>,
    /// Watched wallets this plan allows. `Some(0)` on a plan without webhook
    /// delivery.
    pub limit: Option<i64>,
}

/// A watched wallet that was just added, with the plan's cap, from
/// `client.webhooks.add_address()`.
#[derive(Debug, Clone, PartialEq)]
pub struct WebhookWatchedAddressAdded {
    /// The watched wallet.
    pub address: WebhookWatchedAddress,
    /// Watched wallets this plan allows.
    pub limit: Option<i64>,
}

/// One cap: what the plan allows, what is in use, and what is left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookLimitUsage {
    /// In use now.
    pub used: i64,
    /// What the plan allows. Zero on a plan without webhook delivery.
    pub limit: i64,
    /// What is left, never below zero. A plan change can leave an account
    /// over a cap.
    pub remaining: i64,
}

/// Today's delivery budget. It resets on its own; a paused rule does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookDeliveryBudget {
    /// Deliveries today.
    pub used: i64,
    /// Deliveries a day the plan allows. `None` when the plan has no
    /// ceiling.
    #[serde(default)]
    pub limit: Option<i64>,
    /// Deliveries left today, never below zero. `None` when the plan has no
    /// ceiling.
    #[serde(default)]
    pub remaining: Option<i64>,
    /// `true` when the plan has no daily ceiling.
    pub unlimited: bool,
    /// When the budget resets.
    pub resets_at: String,
    /// Sent only while something is paused, to say that `resets_at` is the
    /// budget's reset and not the pause's.
    #[serde(default)]
    pub resets_at_note: Option<String>,
}

/// Paused rules on the account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookPausedSubscriptions {
    /// How many rules are paused and delivering nothing.
    pub count: i64,
    /// Start of the oldest pause still in force.
    #[serde(default)]
    pub earliest_paused_at: Option<String>,
    /// Distinct causes across the paused rules.
    #[serde(default)]
    pub reasons: Vec<String>,
    /// What is paused and what clears it, in plain words. Sent only when
    /// something is paused.
    #[serde(default)]
    pub message: Option<String>,
}

/// What the plan allows for webhooks and what is in use, from
/// `client.webhooks.limits()`.
///
/// Every number is read from the same place the caps are enforced from, so
/// a refusal and this report cannot disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookLimits {
    /// The plan webhook decisions are priced at.
    pub plan: String,
    /// The plan's display name, when known.
    #[serde(default)]
    pub plan_label: Option<String>,
    /// Whether the plan includes webhook delivery at all. `false` on Free,
    /// where every cap is zero.
    pub included: bool,
    /// Whether the estimate and the dry-run are available. `true` on every
    /// plan, Free included.
    pub preview_included: bool,
    /// Delivery endpoints.
    pub endpoints: WebhookLimitUsage,
    /// Subscriptions, counted across every endpoint.
    pub subscriptions: WebhookLimitUsage,
    /// Watched wallets.
    pub watched_addresses: WebhookLimitUsage,
    /// Today's delivery budget.
    pub deliveries_per_day: WebhookDeliveryBudget,
    /// Paused rules on the account.
    pub paused_subscriptions: WebhookPausedSubscriptions,
    /// Why the caps are zero and what to do about it. Sent only on a plan
    /// without webhook delivery.
    #[serde(default)]
    pub notice: Option<String>,
}

/// The window a preview answer covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookPreviewWindow {
    /// Start of the window. It can be later than requested when a scan
    /// reached its row cap.
    pub from: String,
    /// End of the window, the moment of the request.
    pub to: String,
}

/// One occurrence a preview found, in the shape a delivery carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookPreviewOccurrence {
    /// The occurrence's own timestamp. A real delivery's `observed_at` is
    /// this plus the time it takes to see the occurrence.
    pub observed_at_estimate: String,
    /// The `data` a delivery would carry.
    pub data: serde_json::Value,
}

/// Which occurrences a would-be subscription would have delivered, from
/// `client.webhooks.dry_run()`. Nothing is stored and nothing is sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookDryRun {
    /// Event type that was evaluated.
    pub event_type: String,
    /// The window the answer covers.
    pub window: WebhookPreviewWindow,
    /// Occurrences that matched inside the window, before `limit` applies.
    pub matched: i64,
    /// `true` when fewer occurrences are returned than matched, or when a
    /// scan hit its row cap and the window was narrowed.
    pub truncated: bool,
    /// Newest first, at most `limit`.
    #[serde(default)]
    pub occurrences: Vec<WebhookPreviewOccurrence>,
}

/// How often a would-be subscription would have fired, from
/// `client.webhooks.estimate()`. Nothing is stored and nothing is sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEstimate {
    /// Event type that was evaluated.
    pub event_type: String,
    /// The window the answer covers.
    pub window: WebhookPreviewWindow,
    /// Days the answer covers. Shorter than requested when the event type
    /// has a shorter cap.
    pub days: i64,
    /// Occurrences that would have been delivered across the window.
    pub total: i64,
    /// One entry per day, oldest first, zero filled.
    #[serde(default)]
    pub per_day: Vec<WebhookEstimateDayCount>,
    /// Median deliveries a day across the window.
    pub per_day_p50: f64,
    /// Busiest day in the window.
    pub per_day_max: i64,
    /// The metric the ladder and the distribution are about. `None` when the
    /// type has none.
    #[serde(default)]
    pub primary_metric: Option<String>,
    /// Ascending: the daily rate the same config would have had at other
    /// thresholds on `primary_metric`. Empty when there is no primary
    /// metric.
    #[serde(default)]
    pub ladder: Vec<WebhookEstimateRung>,
    /// Quantiles of `primary_metric`. `None` when the type has no primary
    /// metric or nothing matched.
    #[serde(default)]
    pub distribution: Option<WebhookEstimateDistribution>,
    /// Newest matches first, in the dry-run's shape.
    #[serde(default)]
    pub sample: Vec<WebhookPreviewOccurrence>,
    /// How the estimate was produced.
    pub basis: WebhookEstimateBasis,
}

/// One 24 hour bin of an estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEstimateDayCount {
    /// UTC date the bin ends on (`YYYY-MM-DD`).
    pub date: String,
    /// Occurrences that would have been delivered in the bin.
    pub count: i64,
}

/// One rung of the threshold ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEstimateRung {
    /// Threshold on the primary metric.
    pub value: f64,
    /// Deliveries a day at that threshold, everything else unchanged.
    pub per_day: f64,
}

/// Quantiles of an estimate's primary metric over the matched occurrences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEstimateDistribution {
    /// Occurrences the quantiles are computed over.
    pub n: i64,
    /// Median.
    pub p50: f64,
    /// 90th percentile.
    pub p90: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Largest value seen.
    pub max: f64,
}

/// How an estimate was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEstimateBasis {
    /// `exact` (every occurrence counted), `sampled` (scaled from a capped
    /// scan) or `replayed` (a windowed rule re-run over history at your
    /// parameters).
    pub mode: String,
    /// What qualifies the numbers, when anything does.
    #[serde(default)]
    pub note: Option<String>,
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
