//! Account positions: current and as-of positions per wallet or account,
//! hourly history, the position change log, market-wide listings and
//! summaries.
//!
//! Available on `client.hyperliquid.positions`,
//! `client.hyperliquid.hip3.positions` (keyed by `0x` wallet address),
//! `client.lighter.positions` and `client.rh_lighter.positions` (keyed by
//! integer Lighter account index). Every method returns a [`MetaResponse`]
//! whose `meta` carries the snapshot context (`as_of`, `snapshot_ts`,
//! `source`, `quality`, `stale`) and the build and finality boundaries
//! (`built_through`, `finalized_through`, `clamped_to`).
//!
//! Position routes cost one credit per 1,000 rows returned, with a minimum
//! of one credit per request, the same rate as trades. Account summary
//! routes and the Lighter account lookup cost one credit per request.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{
    AccountSummary, LighterL1Accounts, MarketPosition, MarketPositionsSummary, MetaResponse,
    Position, PositionChange, Timestamp, WalletPositions,
};

const HOUR_MS: i64 = 3_600_000;

const DEX_ERROR: &str = "dex applies to HIP-3 routes only";
const INCLUDE_SYSTEM_ERROR: &str = "include_system applies to Lighter routes only";
const HOUR_ERROR: &str = "hour must be an exact UTC hour in epoch milliseconds";

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Parameters for [`PositionsResource::get`] and [`LighterPositionsResource::get`].
#[derive(Debug, Default, Clone)]
pub struct GetPositionsParams {
    /// As-of instant: the state after every event before it. An exact UTC
    /// hour with a committed hourly snapshot returns that snapshot; any other
    /// instant is reconstructed from the change log (`meta.source` is
    /// `reconstructed`) and clamped to `meta.built_through`. Omit for the
    /// latest live snapshot.
    pub timestamp: Option<Timestamp>,
    /// Only this market.
    pub symbol: Option<String>,
    /// HIP-3 only: only this dex. With a dex, the response also carries that
    /// dex's account summary.
    pub dex: Option<String>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Rows per page (default 500, maximum 5,000).
    pub limit: Option<i64>,
}

/// Time range for [`history`](PositionsResource::history) and
/// [`changes`](PositionsResource::changes): `[start, end)`.
#[derive(Debug, Clone)]
pub struct PositionRangeParams {
    /// Inclusive start.
    pub start: Timestamp,
    /// Exclusive end.
    pub end: Timestamp,
    /// Only this market.
    pub symbol: Option<String>,
    /// HIP-3 only: only this dex.
    pub dex: Option<String>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Rows per page (default 500, maximum 5,000).
    pub limit: Option<i64>,
}

impl PositionRangeParams {
    /// A range with no filters, cursor or limit.
    pub fn new(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            symbol: None,
            dex: None,
            cursor: None,
            limit: None,
        }
    }
}

/// Time range for [`PositionsResource::account_history`]: `[start, end)`.
#[derive(Debug, Clone)]
pub struct AccountHistoryParams {
    /// Inclusive start.
    pub start: Timestamp,
    /// Exclusive end.
    pub end: Timestamp,
    /// HIP-3 only: only this dex.
    pub dex: Option<String>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Rows per page (default 500, maximum 5,000).
    pub limit: Option<i64>,
}

impl AccountHistoryParams {
    /// A range with no dex filter, cursor or limit.
    pub fn new(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            dex: None,
            cursor: None,
            limit: None,
        }
    }
}

/// Parameters for `market`: every open position in one market, largest
/// position value first.
#[derive(Debug, Default, Clone)]
pub struct MarketPositionsParams {
    /// A committed hourly snapshot (exact UTC hour). Omit for the latest live
    /// snapshot. Echoed as `meta.snapshot_ts`.
    pub hour: Option<Timestamp>,
    /// `long` or `short`.
    pub side: Option<String>,
    /// Minimum position value in USD.
    pub min_value: Option<f64>,
    /// Lighter only: include the insurance, settlement and system accounts,
    /// which are excluded by default.
    pub include_system: Option<bool>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Rows per page (default 100, maximum 2,000).
    pub limit: Option<i64>,
}

/// Parameters for `market_summary`. Omit `start`, `end` and `cursor` for the
/// latest live summary (one row); set a range for an hourly series.
#[derive(Debug, Default, Clone)]
pub struct MarketSummaryParams {
    /// Inclusive start of the hourly series. Defaults to 24 hours before `end`.
    pub start: Option<Timestamp>,
    /// Exclusive end of the hourly series. Defaults to now.
    pub end: Option<Timestamp>,
    /// Lighter only: include the insurance, settlement and system accounts.
    pub include_system: Option<bool>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Hours per page (default 100, at most 168).
    pub limit: Option<i64>,
}

/// Parameters for `all`: every open position of the venue at one hour.
#[derive(Debug, Default, Clone)]
pub struct BulkPositionsParams {
    /// A committed hourly snapshot (exact UTC hour). Omit for the latest
    /// committed hour.
    pub hour: Option<Timestamp>,
    /// Lighter only: include the insurance, settlement and system accounts.
    pub include_system: Option<bool>,
    /// Cursor from the previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Rows per page (default 1,000, maximum 2,000).
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------------

type Query = Vec<(&'static str, String)>;

fn push_opt(qp: &mut Query, key: &'static str, value: Option<String>) {
    if let Some(v) = value {
        qp.push((key, v));
    }
}

fn hour_millis(hour: &Timestamp) -> Result<i64> {
    let ms = hour.to_millis();
    if ms < 0 || ms % HOUR_MS != 0 {
        return Err(Error::InvalidParam(HOUR_ERROR.to_string()));
    }
    Ok(ms)
}

fn check_address(address: &str, name: &str) -> Result<()> {
    let valid = address.len() == 42
        && address
            .strip_prefix("0x")
            .is_some_and(|hex| hex.chars().all(|c| c.is_ascii_hexdigit()));
    if !valid {
        return Err(Error::InvalidParam(format!(
            "{name} must be a 42-character hex string starting with 0x"
        )));
    }
    Ok(())
}

fn get_query(p: &GetPositionsParams) -> Query {
    let mut qp = Query::new();
    push_opt(
        &mut qp,
        "timestamp",
        p.timestamp.as_ref().map(|t| t.to_millis().to_string()),
    );
    push_opt(&mut qp, "symbol", p.symbol.clone());
    push_opt(&mut qp, "dex", p.dex.clone());
    push_opt(&mut qp, "cursor", p.cursor.clone());
    push_opt(&mut qp, "limit", p.limit.map(|l| l.to_string()));
    qp
}

fn range_query(p: &PositionRangeParams) -> Query {
    let mut qp = vec![
        ("start", p.start.to_millis().to_string()),
        ("end", p.end.to_millis().to_string()),
    ];
    push_opt(&mut qp, "symbol", p.symbol.clone());
    push_opt(&mut qp, "dex", p.dex.clone());
    push_opt(&mut qp, "cursor", p.cursor.clone());
    push_opt(&mut qp, "limit", p.limit.map(|l| l.to_string()));
    qp
}

fn market_query(p: &MarketPositionsParams) -> Result<Query> {
    let mut qp = Query::new();
    if let Some(h) = &p.hour {
        qp.push(("hour", hour_millis(h)?.to_string()));
    }
    push_opt(&mut qp, "side", p.side.clone());
    push_opt(&mut qp, "min_value", p.min_value.map(|v| v.to_string()));
    push_opt(
        &mut qp,
        "include_system",
        p.include_system.map(|b| b.to_string()),
    );
    push_opt(&mut qp, "cursor", p.cursor.clone());
    push_opt(&mut qp, "limit", p.limit.map(|l| l.to_string()));
    Ok(qp)
}

fn summary_query(p: &MarketSummaryParams) -> Query {
    let mut qp = Query::new();
    push_opt(
        &mut qp,
        "start",
        p.start.as_ref().map(|t| t.to_millis().to_string()),
    );
    push_opt(
        &mut qp,
        "end",
        p.end.as_ref().map(|t| t.to_millis().to_string()),
    );
    push_opt(
        &mut qp,
        "include_system",
        p.include_system.map(|b| b.to_string()),
    );
    push_opt(&mut qp, "cursor", p.cursor.clone());
    push_opt(&mut qp, "limit", p.limit.map(|l| l.to_string()));
    qp
}

fn bulk_query(p: &BulkPositionsParams) -> Result<Query> {
    let mut qp = Query::new();
    if let Some(h) = &p.hour {
        qp.push(("hour", hour_millis(h)?.to_string()));
    }
    push_opt(
        &mut qp,
        "include_system",
        p.include_system.map(|b| b.to_string()),
    );
    push_opt(&mut qp, "cursor", p.cursor.clone());
    push_opt(&mut qp, "limit", p.limit.map(|l| l.to_string()));
    Ok(qp)
}

async fn fetch<T: serde::de::DeserializeOwned>(
    http: &HttpClient,
    path: &str,
    qp: &Query,
) -> Result<MetaResponse<T>> {
    let (data, meta) = http.get_with_meta(path, qp).await?;
    Ok(MetaResponse::new(data, meta))
}

// ---------------------------------------------------------------------------
// Hyperliquid and HIP-3
// ---------------------------------------------------------------------------

/// Account positions on Hyperliquid core (`client.hyperliquid.positions`) and
/// HIP-3 (`client.hyperliquid.hip3.positions`), keyed by `0x` wallet address.
///
/// Coverage: the core change log (and as-of reads) from 2025-05-25, HIP-3
/// from 2025-10-13, hourly snapshots from 2026-06-07, and a live snapshot
/// every 5 minutes. `dex` filters apply to HIP-3 only and are rejected on
/// core before a request is sent.
///
/// A cursor for a market listing pins its snapshot. When that snapshot has
/// been replaced or has expired, the server answers HTTP 409; restart
/// pagination without a cursor.
#[derive(Debug, Clone)]
pub struct PositionsResource {
    http: HttpClient,
    prefix: String,
    hip3: bool,
}

impl PositionsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
            hip3: prefix.ends_with("/hip3"),
        }
    }

    fn check_dex(&self, dex: Option<&String>) -> Result<()> {
        if dex.is_some() && !self.hip3 {
            return Err(Error::InvalidParam(DEX_ERROR.to_string()));
        }
        Ok(())
    }

    fn check_include_system(include_system: Option<bool>) -> Result<()> {
        if include_system == Some(true) {
            return Err(Error::InvalidParam(INCLUDE_SYSTEM_ERROR.to_string()));
        }
        Ok(())
    }

    /// Open positions of a wallet now (the latest live snapshot), or as of
    /// `params.timestamp`.
    ///
    /// On the first page, `data.account` carries the account summary (core,
    /// or HIP-3 with a `dex`). When there are no open positions,
    /// `data.account_seen` says whether the wallet is `flat`, `never_seen`,
    /// or `outside_coverage`.
    pub async fn get(
        &self,
        address: &str,
        params: Option<GetPositionsParams>,
    ) -> Result<MetaResponse<WalletPositions>> {
        let p = params.unwrap_or_default();
        check_address(address, "address")?;
        self.check_dex(p.dex.as_ref())?;
        let path = format!("{}/wallets/{}/positions", self.prefix, address);
        fetch(&self.http, &path, &get_query(&p)).await
    }

    /// Hourly position snapshots of a wallet in `[start, end)`, each row
    /// stamped with its `snapshot_ts`. Hourly history begins 2026-06-07.
    pub async fn history(
        &self,
        address: &str,
        params: PositionRangeParams,
    ) -> Result<MetaResponse<Vec<Position>>> {
        check_address(address, "address")?;
        self.check_dex(params.dex.as_ref())?;
        let path = format!("{}/wallets/{}/positions/history", self.prefix, address);
        fetch(&self.http, &path, &range_query(&params)).await
    }

    /// Position change log of a wallet in `[start, end)`: one row per fill
    /// leg with the position before and after, oldest first. `end` is clamped
    /// to `meta.built_through`.
    pub async fn changes(
        &self,
        address: &str,
        params: PositionRangeParams,
    ) -> Result<MetaResponse<Vec<PositionChange>>> {
        check_address(address, "address")?;
        self.check_dex(params.dex.as_ref())?;
        let path = format!("{}/wallets/{}/positions/changes", self.prefix, address);
        fetch(&self.http, &path, &range_query(&params)).await
    }

    /// Current account summary of a wallet: one per clearinghouse, so one on
    /// core and one per dex on HIP-3 (or only `dex`).
    pub async fn account(
        &self,
        address: &str,
        dex: Option<&str>,
    ) -> Result<MetaResponse<Vec<AccountSummary>>> {
        check_address(address, "address")?;
        let dex = dex.map(str::to_string);
        self.check_dex(dex.as_ref())?;
        let mut qp = Query::new();
        push_opt(&mut qp, "dex", dex);
        let path = format!("{}/wallets/{}/account", self.prefix, address);
        fetch(&self.http, &path, &qp).await
    }

    /// Hourly account summaries of a wallet in `[start, end)`.
    pub async fn account_history(
        &self,
        address: &str,
        params: AccountHistoryParams,
    ) -> Result<MetaResponse<Vec<AccountSummary>>> {
        check_address(address, "address")?;
        self.check_dex(params.dex.as_ref())?;
        let mut qp = vec![
            ("start", params.start.to_millis().to_string()),
            ("end", params.end.to_millis().to_string()),
        ];
        push_opt(&mut qp, "dex", params.dex);
        push_opt(&mut qp, "cursor", params.cursor);
        push_opt(&mut qp, "limit", params.limit.map(|l| l.to_string()));
        let path = format!("{}/wallets/{}/account/history", self.prefix, address);
        fetch(&self.http, &path, &qp).await
    }

    /// Every open position in one market, largest position value first, at
    /// the latest live snapshot or at `params.hour`.
    ///
    /// The first page carries totals over the whole filtered set in
    /// `meta.totals`; read them with `meta.position_totals()`.
    pub async fn market(
        &self,
        symbol: &str,
        params: Option<MarketPositionsParams>,
    ) -> Result<MetaResponse<Vec<MarketPosition>>> {
        let p = params.unwrap_or_default();
        Self::check_include_system(p.include_system)?;
        let qp = market_query(&p)?;
        let path = format!("{}/positions/{}", self.prefix, symbol);
        fetch(&self.http, &path, &qp).await
    }

    /// Long and short aggregates of one market: the latest live summary, or
    /// an hourly series when `start` or `end` is set.
    pub async fn market_summary(
        &self,
        symbol: &str,
        params: Option<MarketSummaryParams>,
    ) -> Result<MetaResponse<Vec<MarketPositionsSummary>>> {
        let p = params.unwrap_or_default();
        Self::check_include_system(p.include_system)?;
        let path = format!("{}/positions/{}/summary", self.prefix, symbol);
        fetch(&self.http, &path, &summary_query(&p)).await
    }

    /// Every open position across the venue's markets at one committed hour,
    /// each row stamped with its `snapshot_ts`.
    pub async fn all(
        &self,
        params: Option<BulkPositionsParams>,
    ) -> Result<MetaResponse<Vec<MarketPosition>>> {
        let p = params.unwrap_or_default();
        Self::check_include_system(p.include_system)?;
        let qp = bulk_query(&p)?;
        let path = format!("{}/positions", self.prefix);
        fetch(&self.http, &path, &qp).await
    }
}

// ---------------------------------------------------------------------------
// Lighter (mainnet and Robinhood Chain)
// ---------------------------------------------------------------------------

/// Account positions on Lighter (`client.lighter.positions`) and Lighter on
/// Robinhood Chain (`client.rh_lighter.positions`), keyed by integer account
/// index. Perp markets only.
///
/// Coverage: mainnet from 2025-01-17 and Robinhood Chain from 2026-06-26,
/// with hourly snapshots over the same span and a live snapshot every 2
/// minutes. Rows carry `account_kind`; market listings exclude the
/// insurance, settlement and system accounts unless `include_system` is set.
/// To find the account indices of an L1 address on mainnet, use
/// `client.lighter.accounts.by_l1(...)`.
///
/// A cursor for a market listing pins its snapshot. When that snapshot has
/// been replaced or has expired, the server answers HTTP 409; restart
/// pagination without a cursor.
#[derive(Debug, Clone)]
pub struct LighterPositionsResource {
    http: HttpClient,
    prefix: String,
}

impl LighterPositionsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    fn check_no_dex(dex: Option<&String>) -> Result<()> {
        if dex.is_some() {
            return Err(Error::InvalidParam(DEX_ERROR.to_string()));
        }
        Ok(())
    }

    /// Open positions of an account now (the latest live snapshot), or as of
    /// `params.timestamp`.
    ///
    /// On the first page without a `symbol`, `data.account` carries the
    /// account's position aggregates. When there are no open positions,
    /// `data.account_seen` says whether the account is `flat`, `never_seen`,
    /// or `outside_coverage`.
    pub async fn get(
        &self,
        account_index: u64,
        params: Option<GetPositionsParams>,
    ) -> Result<MetaResponse<WalletPositions>> {
        let p = params.unwrap_or_default();
        Self::check_no_dex(p.dex.as_ref())?;
        let path = format!("{}/accounts/{}/positions", self.prefix, account_index);
        fetch(&self.http, &path, &get_query(&p)).await
    }

    /// Hourly position snapshots of an account in `[start, end)`, each row
    /// stamped with its `snapshot_ts`.
    pub async fn history(
        &self,
        account_index: u64,
        params: PositionRangeParams,
    ) -> Result<MetaResponse<Vec<Position>>> {
        Self::check_no_dex(params.dex.as_ref())?;
        let path = format!(
            "{}/accounts/{}/positions/history",
            self.prefix, account_index
        );
        fetch(&self.http, &path, &range_query(&params)).await
    }

    /// Position change log of an account in `[start, end)`: one row per fill
    /// leg with the position before and after, oldest first. `end` is clamped
    /// to `meta.built_through`; legs after `meta.finalized_through` are
    /// preliminary (`finalized: false`).
    pub async fn changes(
        &self,
        account_index: u64,
        params: PositionRangeParams,
    ) -> Result<MetaResponse<Vec<PositionChange>>> {
        Self::check_no_dex(params.dex.as_ref())?;
        let path = format!(
            "{}/accounts/{}/positions/changes",
            self.prefix, account_index
        );
        fetch(&self.http, &path, &range_query(&params)).await
    }

    /// Every open position in one market, largest position value first, at
    /// the latest live snapshot or at `params.hour`.
    ///
    /// The first page carries totals over the whole filtered set in
    /// `meta.totals`; read them with `meta.position_totals()`.
    pub async fn market(
        &self,
        symbol: &str,
        params: Option<MarketPositionsParams>,
    ) -> Result<MetaResponse<Vec<MarketPosition>>> {
        let p = params.unwrap_or_default();
        let qp = market_query(&p)?;
        let path = format!("{}/positions/{}", self.prefix, symbol);
        fetch(&self.http, &path, &qp).await
    }

    /// Long and short aggregates of one market: the latest live summary, or
    /// an hourly series when `start` or `end` is set.
    pub async fn market_summary(
        &self,
        symbol: &str,
        params: Option<MarketSummaryParams>,
    ) -> Result<MetaResponse<Vec<MarketPositionsSummary>>> {
        let p = params.unwrap_or_default();
        let path = format!("{}/positions/{}/summary", self.prefix, symbol);
        fetch(&self.http, &path, &summary_query(&p)).await
    }

    /// Every open position across the venue's perp markets at one committed
    /// hour, each row stamped with its `snapshot_ts`.
    pub async fn all(
        &self,
        params: Option<BulkPositionsParams>,
    ) -> Result<MetaResponse<Vec<MarketPosition>>> {
        let p = params.unwrap_or_default();
        let qp = bulk_query(&p)?;
        let path = format!("{}/positions", self.prefix);
        fetch(&self.http, &path, &qp).await
    }
}

/// Lighter account lookup (`client.lighter.accounts`, mainnet only).
#[derive(Debug, Clone)]
pub struct LighterAccountsResource {
    http: HttpClient,
    prefix: String,
}

impl LighterAccountsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Account indices owned by an L1 (`0x`) address, with the total across
    /// every page in `data.total_accounts`. Rows per page default to 500
    /// (maximum 5,000).
    pub async fn by_l1(
        &self,
        l1_address: &str,
        cursor: Option<&str>,
        limit: Option<i64>,
    ) -> Result<MetaResponse<LighterL1Accounts>> {
        check_address(l1_address, "l1_address")?;
        let mut qp: Query = vec![("l1_address", l1_address.to_string())];
        push_opt(&mut qp, "cursor", cursor.map(str::to_string));
        push_opt(&mut qp, "limit", limit.map(|l| l.to_string()));
        let path = format!("{}/accounts", self.prefix);
        fetch(&self.http, &path, &qp).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_must_be_an_exact_utc_hour() {
        assert_eq!(
            hour_millis(&Timestamp::from(1_790_294_400_000_i64)).unwrap(),
            1_790_294_400_000
        );
        assert!(hour_millis(&Timestamp::from(1_790_294_400_001_i64)).is_err());
        assert_eq!(
            hour_millis(&Timestamp::from("2026-09-25T12:00:00Z")).unwrap(),
            1_790_337_600_000
        );
    }

    #[test]
    fn addresses_are_checked_before_sending() {
        assert!(check_address("0x0123456789abcdef0123456789ABCDEF01234567", "address").is_ok());
        for bad in [
            "",
            "0x123",
            "0123456789abcdef0123456789abcdef0123456789",
            "0xZZ23456789abcdef0123456789abcdef01234567",
        ] {
            assert!(check_address(bad, "address").is_err(), "{bad}");
        }
    }
}
