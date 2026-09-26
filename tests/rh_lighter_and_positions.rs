use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::liquidations::{LiquidationHistoryParams, LiquidationVolumeParams};
use oxarchive::resources::positions::{
    AccountHistoryParams, BulkPositionsParams, GetPositionsParams, MarketPositionsParams,
    MarketSummaryParams, PositionRangeParams,
};
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::types::{CandleInterval, Timestamp};
use oxarchive::{Error, OxArchive};
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WALLET: &str = "0x0123456789abcdef0123456789abcdef01234567";

fn client(server: &MockServer) -> OxArchive {
    OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap()
}

fn offline_client() -> OxArchive {
    OxArchive::builder("test-key")
        .base_url("http://127.0.0.1:9")
        .build()
        .unwrap()
}

fn ok(data: Value, meta: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"success": true, "data": data, "meta": meta}))
}

fn meta(count: usize) -> Value {
    json!({"count": count, "request_id": "req-1"})
}

// ---------------------------------------------------------------------------
// Lighter on Robinhood Chain
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rh_lighter_routes_live_under_their_own_prefix() {
    let server = MockServer::start().await;
    let instrument = json!({"symbol": "AAPL-USDG", "market_id": 2048, "market_type": "spot"});
    let book = json!({"coin": "BTC", "timestamp": "2026-09-01T00:00:00Z", "bids": [], "asks": []});
    let routes: Vec<(&str, Value)> = vec![
        ("/v1/rh-lighter/instruments", json!([instrument.clone()])),
        ("/v1/rh-lighter/instruments/AAPL-USDG", instrument),
        ("/v1/rh-lighter/orderbook/BTC", book),
        (
            "/v1/rh-lighter/funding/BTC/current",
            json!({"coin": "BTC", "timestamp": "2026-09-01T00:00:00Z", "funding_rate": "0.0001"}),
        ),
        (
            "/v1/rh-lighter/openinterest/BTC/current",
            json!({"coin": "BTC", "timestamp": "2026-09-01T00:00:00Z", "open_interest": "12.5"}),
        ),
        (
            "/v1/rh-lighter/freshness/BTC",
            json!({"coin": "BTC", "exchange": "rh-lighter"}),
        ),
        (
            "/v1/rh-lighter/summary/BTC",
            json!({"coin": "BTC", "volume_24h": 1234.5}),
        ),
        (
            "/v1/rh-lighter/trades/BTC/recent",
            json!([{"coin": "BTC", "side": "B", "price": "100", "size": "1", "timestamp": "2026-09-01T00:00:00Z"}]),
        ),
    ];
    for (route, data) in routes {
        Mock::given(method("GET"))
            .and(path(route))
            .and(header("x-api-key", "test-key"))
            .respond_with(ok(data, meta(1)))
            .expect(1)
            .mount(&server)
            .await;
    }

    let c = client(&server);
    let rh = &c.rh_lighter;
    assert_eq!(rh.instruments.list().await.unwrap()[0].symbol, "AAPL-USDG");
    assert_eq!(
        rh.instruments.get("AAPL-USDG").await.unwrap().market_id,
        2048
    );
    assert_eq!(rh.orderbook.get("BTC", None).await.unwrap().coin, "BTC");
    assert_eq!(
        rh.funding.current("BTC").await.unwrap().funding_rate,
        "0.0001"
    );
    assert_eq!(
        rh.open_interest.current("BTC").await.unwrap().open_interest,
        "12.5"
    );
    assert_eq!(
        rh.freshness("BTC").await.unwrap().exchange.as_deref(),
        Some("rh-lighter")
    );
    assert_eq!(
        rh.summary("BTC").await.unwrap().volume_24h.as_deref(),
        Some("1234.5")
    );
    assert_eq!(rh.trades.recent("BTC", None).await.unwrap().len(), 1);
}

#[tokio::test]
async fn rh_lighter_history_routes_send_the_same_params_as_mainnet() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/rh-lighter/trades/AAPL-USDG"))
        .and(query_param("start", "1788220800000"))
        .and(query_param("end", "1788307200000"))
        .and(query_param("limit", "1000"))
        .respond_with(ok(
            json!([]),
            json!({"count": 0, "request_id": "t", "next_cursor": "1788300000000_42"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/rh-lighter/candles/BTC"))
        .and(query_param("limit", "10000"))
        .and(query_param("interval", "1h"))
        .respond_with(ok(json!([]), meta(0)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/rh-lighter/prices/BTC"))
        .and(query_param("interval", "1h"))
        .respond_with(ok(json!([]), meta(0)))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let trades = c
        .rh_lighter
        .trades
        .list(
            "AAPL-USDG",
            GetTradesParams {
                start: 1788220800000_i64.into(),
                end: 1788307200000_i64.into(),
                cursor: None,
                limit: Some(1000),
                side: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(trades.next_cursor.as_deref(), Some("1788300000000_42"));

    let candle_params = |limit| CandleHistoryParams {
        start: Timestamp::from(1788220800000_i64),
        end: Timestamp::from(1788307200000_i64),
        cursor: None,
        limit: Some(limit),
        interval: Some(CandleInterval::OneHour),
    };
    c.rh_lighter
        .candles
        .history("BTC", candle_params(10_000))
        .await
        .unwrap();
    let error = c
        .rh_lighter
        .candles
        .history("BTC", candle_params(10_001))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("1 and 10000"));

    c.rh_lighter
        .price_history(
            "BTC",
            1788220800000_i64,
            1788307200000_i64,
            Some("1h"),
            None,
            None,
        )
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Lighter liquidations (both deployments)
// ---------------------------------------------------------------------------

fn liquidation_row(source: &str, raw_json: &str) -> Value {
    json!({
        "symbol": "BTC",
        "timestamp": 1788221000123_i64,
        "transaction_time_us": 1788221000123456_i64,
        "trade_id": 987654321,
        "liquidation_type": "liquidation",
        "price": 108250.5,
        "size": 0.25,
        "usd_amount": 27062.625,
        "ask_account": "281474976710654",
        "bid_account": "713845",
        "ask_order_id": 11,
        "bid_order_id": 12,
        "is_maker_ask": false,
        "taker_position_size_before": -0.25,
        "maker_position_size_before": 3.5,
        "taker_entry_quote_before": 27500.0,
        "maker_entry_quote_before": 1.0,
        "taker_initial_margin_fraction_before": 500,
        "maker_initial_margin_fraction_before": 500,
        "taker_allocated_margin_usdc_before": 0,
        "taker_allocated_margin_usdc_after": 0,
        "maker_allocated_margin_usdc_before": 0,
        "maker_allocated_margin_usdc_after": 0,
        "taker_fee": 0,
        "maker_fee": 0,
        "taker_position_sign_changed": true,
        "maker_position_sign_changed": false,
        "block_height": 5550001,
        "tx_hash": "0xabc",
        "raw_json": raw_json,
        "source": source
    })
}

#[tokio::test]
async fn both_lighter_clients_expose_liquidation_history_and_volume() {
    let server = MockServer::start().await;
    for prefix in ["/v1/lighter", "/v1/rh-lighter"] {
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/liquidations/BTC")))
            .and(query_param("start", "1788220800000"))
            .and(query_param("end", "1788307200000"))
            .and(query_param("limit", "500"))
            .respond_with(ok(
                json!([
                    liquidation_row("bucket", ""),
                    liquidation_row("ws", "{\"trade_id\":1}")
                ]),
                json!({"count": 2, "request_id": "liq", "next_cursor": "1788221000123_987654321"}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/liquidations/BTC/volume")))
            .and(query_param("interval", "1h"))
            .respond_with(ok(
                json!([{"symbol": "BTC", "timestamp": 1788220800000_i64, "total_usd": 27062.625, "count": 1}]),
                meta(1),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }

    let c = client(&server);
    for liquidations in [&c.lighter.liquidations, &c.rh_lighter.liquidations] {
        let page = liquidations
            .history(
                "BTC",
                LiquidationHistoryParams {
                    start: 1788220800000_i64.into(),
                    end: 1788307200000_i64.into(),
                    cursor: None,
                    limit: Some(500),
                },
            )
            .await
            .unwrap();
        assert_eq!(page.next_cursor.as_deref(), Some("1788221000123_987654321"));
        let backfilled = &page.data[0];
        assert_eq!(backfilled.source.as_deref(), Some("bucket"));
        assert_eq!(backfilled.raw_json, "");
        assert_eq!(backfilled.timestamp, 1788221000123);
        assert_eq!(backfilled.price, "108250.5");
        assert_eq!(backfilled.size, "0.25");
        assert_eq!(backfilled.usd_amount.as_deref(), Some("27062.625"));
        assert_eq!(backfilled.ask_account.as_deref(), Some("281474976710654"));
        assert_eq!(backfilled.taker_position_size_before, Some(-0.25));
        assert_eq!(page.data[1].source.as_deref(), Some("ws"));
        assert!(!page.data[1].raw_json.is_empty());

        let volume = liquidations
            .volume(
                "BTC",
                LiquidationVolumeParams {
                    start: 1788220800000_i64.into(),
                    end: 1788307200000_i64.into(),
                    interval: Some("1h".to_string()),
                    cursor: None,
                    limit: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(volume.data[0].total_usd, "27062.625");
        assert_eq!(volume.data[0].count, 1);
    }
}

// ---------------------------------------------------------------------------
// Account positions: Hyperliquid and HIP-3
// ---------------------------------------------------------------------------

fn core_position() -> Value {
    json!({
        "symbol": "BTC",
        "coin": "BTC",
        "size": "-0.5",
        "side": "short",
        "entry_price": "108000",
        "mark_price": "107500",
        "mark_time": "2026-09-25T00:00:00.000Z",
        "position_value": "53750",
        "unrealized_pnl": "250",
        "return_on_equity": "0.0463",
        "leverage": {"type": "cross", "value": "10"},
        "max_leverage": 40,
        "margin_used": "5375",
        "liquidation_price": "150000",
        "liquidation_price_status": "exact",
        "cum_funding": {"all_time": "-12.5", "since_open": "-3.1", "since_change": "0"},
        "opened_at": "2026-09-20T08:15:00.123Z",
        "snapshot_as_of": "2026-09-24T23:58:00.000Z",
        "quality": "complete"
    })
}

fn core_account() -> Value {
    json!({
        "account_value": "10000",
        "cross_account_value": "10000",
        "collateral": "9750",
        "total_margin_used": "5375",
        "cross_maintenance_margin_used": "1343.75",
        "withdrawable": null,
        "total_position_value": "53750",
        "total_unrealized_pnl": "250",
        "long_value": "0",
        "short_value": "53750",
        "n_positions": 1,
        "account_mode": "standard",
        "snapshot_as_of": "2026-09-24T23:58:00.000Z",
        "quality": "complete"
    })
}

#[tokio::test]
async fn hyperliquid_get_returns_positions_account_and_snapshot_meta() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/hyperliquid/wallets/{WALLET}/positions")))
        .and(header("x-api-key", "test-key"))
        .and(query_param_is_missing("timestamp"))
        .respond_with(ok(
            json!({"positions": [core_position()], "account": core_account()}),
            json!({
                "count": 1,
                "request_id": "pos-live",
                "as_of": "2026-09-25T00:00:00.000Z",
                "snapshot_ts": "2026-09-25T00:00:00.000Z",
                "source": "snapshot",
                "quality": "complete",
                "stale": false
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .hyperliquid
        .positions
        .get(WALLET, None)
        .await
        .unwrap();
    let p = &page.data.positions[0];
    assert_eq!(p.side, "short");
    assert_eq!(p.size, "-0.5");
    assert_eq!(p.leverage.kind, "cross");
    assert_eq!(p.leverage.value.as_deref(), Some("10"));
    assert_eq!(p.max_leverage, Some(40));
    assert_eq!(p.cum_funding.since_open.as_deref(), Some("-3.1"));
    assert_eq!(p.liquidation_price_status, "exact");
    assert_eq!(p.dex, None);
    let account = page.data.account.as_ref().unwrap();
    assert_eq!(account.n_positions, 1);
    assert_eq!(account.withdrawable, None);
    assert_eq!(account.account_mode.as_deref(), Some("standard"));
    assert_eq!(page.data.account_seen, None);
    assert_eq!(page.meta.request_id, "pos-live");
    assert_eq!(page.meta.as_of.as_deref(), Some("2026-09-25T00:00:00.000Z"));
    assert_eq!(
        page.meta.snapshot_ts.as_deref(),
        Some("2026-09-25T00:00:00.000Z")
    );
    assert_eq!(page.meta.source.as_deref(), Some("snapshot"));
    assert_eq!(page.meta.quality.as_deref(), Some("complete"));
    assert_eq!(page.meta.stale, Some(false));
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn as_of_reads_send_timestamp_and_surface_the_clamp() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/hyperliquid/wallets/{WALLET}/positions")))
        .and(query_param("timestamp", "1790337600123"))
        .and(query_param("symbol", "BTC"))
        .and(query_param("limit", "50"))
        .respond_with(ok(
            json!({"positions": [], "account": null, "account_seen": "never_seen"}),
            json!({
                "count": 0,
                "request_id": "pos-asof",
                "as_of": "2026-09-25T11:00:00.000Z",
                "source": "reconstructed",
                "quality": "complete",
                "built_through": "2026-09-25T11:00:00.000Z",
                "finalized_through": "2026-09-25T09:00:00.000Z",
                "requested_end": "2026-09-25T12:00:00.123Z",
                "clamped_to": "2026-09-25T11:00:00.000Z",
                "notice": "No activity is recorded for this address since 2025-05-25 15:00 (UTC).",
                "coverage_from": "2025-05-25T15:00:00.000Z"
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .hyperliquid
        .positions
        .get(
            WALLET,
            Some(GetPositionsParams {
                timestamp: Some(1790337600123_i64.into()),
                symbol: Some("BTC".to_string()),
                limit: Some(50),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    assert!(page.data.positions.is_empty());
    assert_eq!(page.data.account_seen.as_deref(), Some("never_seen"));
    let m = &page.meta;
    assert_eq!(m.source.as_deref(), Some("reconstructed"));
    assert_eq!(m.built_through.as_deref(), Some("2026-09-25T11:00:00.000Z"));
    assert_eq!(
        m.finalized_through.as_deref(),
        Some("2026-09-25T09:00:00.000Z")
    );
    assert_eq!(m.requested_end.as_deref(), Some("2026-09-25T12:00:00.123Z"));
    assert_eq!(m.clamped_to.as_deref(), Some("2026-09-25T11:00:00.000Z"));
    assert_eq!(m.coverage_from.as_deref(), Some("2025-05-25T15:00:00.000Z"));
    assert!(m.notice.as_deref().unwrap().contains("No activity"));
}

#[tokio::test]
async fn history_changes_and_account_routes_map_their_params() {
    let server = MockServer::start().await;
    let change = json!({
        "timestamp": "2026-09-25T01:02:03.456Z",
        "symbol": "xyz:TSLA",
        "coin": "xyz:TSLA",
        "dex": "xyz",
        "side": "B",
        "price": "410.5",
        "size": "2",
        "start_position": "0",
        "end_position": "2",
        "entry_price_after": "410.5",
        "event_type": "open",
        "cause": "trade",
        "direction": "Open Long",
        "closed_pnl": "0",
        "fee": "0.41",
        "fee_token": "USDC",
        "crossed": true,
        "trade_id": 123456789,
        "order_id": 987654321,
        "opened_at": "2026-09-25T01:02:03.456Z",
        "seq": 0,
        "block_number": 1100000000,
        "event_index": 7,
        "continuity": "ok",
        "finalized": true
    });
    let prefix = format!("/v1/hyperliquid/hip3/wallets/{WALLET}");
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/positions/changes")))
        .and(query_param("start", "1790294400000"))
        .and(query_param("end", "1790380800000"))
        .and(query_param("dex", "xyz"))
        .and(query_param("symbol", "xyz:TSLA"))
        .and(query_param("cursor", "abc"))
        .respond_with(ok(
            json!([change]),
            json!({
                "count": 1,
                "request_id": "chg",
                "next_cursor": "def",
                "source": "changes",
                "built_through": "2026-09-25T23:00:00.000Z",
                "finalized_through": "2026-09-25T20:00:00.000Z"
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let mut history_row = core_position();
    history_row["snapshot_ts"] = json!("2026-09-25T00:00:00.000Z");
    history_row["dex"] = json!("xyz");
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/positions/history")))
        .and(query_param("start", "1790294400000"))
        .and(query_param("end", "1790380800000"))
        .and(query_param("limit", "5000"))
        .respond_with(ok(json!([history_row]), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    let mut account = core_account();
    account["dex"] = json!("xyz");
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/account")))
        .and(query_param("dex", "xyz"))
        .respond_with(ok(json!([account.clone()]), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    account["snapshot_ts"] = json!("2026-09-25T00:00:00.000Z");
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/account/history")))
        .and(query_param("start", "1790294400000"))
        .and(query_param("end", "1790380800000"))
        .respond_with(ok(json!([account]), meta(1)))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let hip3 = &c.hyperliquid.hip3.positions;
    let changes = hip3
        .changes(
            WALLET,
            PositionRangeParams {
                symbol: Some("xyz:TSLA".to_string()),
                dex: Some("xyz".to_string()),
                cursor: Some("abc".to_string()),
                ..PositionRangeParams::new(1790294400000_i64, 1790380800000_i64)
            },
        )
        .await
        .unwrap();
    assert_eq!(changes.next_cursor.as_deref(), Some("def"));
    assert_eq!(changes.meta.source.as_deref(), Some("changes"));
    let leg = &changes.data[0];
    assert_eq!(leg.event_type, "open");
    assert_eq!(leg.cause, "trade");
    assert_eq!(leg.crossed, Some(true));
    assert_eq!(leg.is_maker, None);
    assert_eq!(leg.block_number, Some(1100000000));
    assert_eq!(leg.dex.as_deref(), Some("xyz"));
    assert_eq!(leg.continuity, "ok");

    let history = hip3
        .history(
            WALLET,
            PositionRangeParams {
                limit: Some(5000),
                ..PositionRangeParams::new(
                    Timestamp::from("2026-09-25T00:00:00Z"),
                    Timestamp::from("2026-09-26T00:00:00Z"),
                )
            },
        )
        .await
        .unwrap();
    assert_eq!(
        history.data[0].snapshot_ts.as_deref(),
        Some("2026-09-25T00:00:00.000Z")
    );

    let accounts = hip3.account(WALLET, Some("xyz")).await.unwrap();
    assert_eq!(accounts.data[0].dex.as_deref(), Some("xyz"));
    let account_history = hip3
        .account_history(
            WALLET,
            AccountHistoryParams::new(1790294400000_i64, 1790380800000_i64),
        )
        .await
        .unwrap();
    assert_eq!(
        account_history.data[0].snapshot_ts.as_deref(),
        Some("2026-09-25T00:00:00.000Z")
    );
}

#[tokio::test]
async fn market_listing_carries_typed_totals_and_summary_series_pages() {
    let server = MockServer::start().await;
    let summary = json!({
        "snapshot_ts": "2026-09-25T12:00:00.000Z",
        "symbol": "BTC",
        "coin": "BTC",
        "long_count": 1200,
        "short_count": 900,
        "long_size": "1500.5",
        "short_size": "1200.25",
        "long_value": "162000000",
        "short_value": "129600000",
        "long_avg_entry_price": "104000",
        "short_avg_entry_price": "109000",
        "long_positions_with_entry": 1199,
        "short_positions_with_entry": 900,
        "long_top10_value_share": "0.42",
        "short_top10_value_share": "0.51",
        "top10_value_share": "0.33",
        "quality": "complete"
    });
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/positions/BTC"))
        .and(query_param("hour", "1790337600000"))
        .and(query_param("side", "long"))
        .and(query_param("min_value", "100000"))
        .and(query_param("limit", "2000"))
        .respond_with(ok(
            json!([{
                "user_address": WALLET,
                "symbol": "BTC",
                "coin": "BTC",
                "size": "12",
                "side": "long",
                "entry_price": "104000",
                "mark_price": "108000",
                "position_value": "1296000",
                "unrealized_pnl": "48000",
                "leverage_type": "cross",
                "liquidation_price": null,
                "quality": "complete"
            }]),
            json!({
                "count": 1,
                "request_id": "mkt",
                "next_cursor": "page-2",
                "snapshot_ts": "2026-09-25T12:00:00.000Z",
                "source": "snapshot",
                "quality": "complete",
                "totals": summary
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/positions/BTC/summary"))
        .and(query_param("start", "1790294400000"))
        .and(query_param("end", "1790380800000"))
        .and(query_param("limit", "168"))
        .respond_with(ok(json!([summary]), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/positions/ETH/summary"))
        .and(query_param_is_missing("start"))
        .and(query_param_is_missing("end"))
        .respond_with(ok(json!([]), meta(0)))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let page = c
        .hyperliquid
        .positions
        .market(
            "BTC",
            Some(MarketPositionsParams {
                hour: Some(Timestamp::from("2026-09-25T12:00:00Z")),
                side: Some("long".to_string()),
                min_value: Some(100_000.0),
                limit: Some(2000),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    assert_eq!(page.data[0].user_address.as_deref(), Some(WALLET));
    assert_eq!(page.data[0].leverage_type, "cross");
    assert_eq!(page.next_cursor.as_deref(), Some("page-2"));
    let totals = page
        .meta
        .position_totals()
        .expect("first page carries totals");
    assert_eq!(totals.long_count, 1200);
    assert_eq!(totals.top10_value_share.as_deref(), Some("0.33"));

    let series = c
        .hyperliquid
        .positions
        .market_summary(
            "BTC",
            Some(MarketSummaryParams {
                start: Some(1790294400000_i64.into()),
                end: Some(1790380800000_i64.into()),
                limit: Some(168),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    assert_eq!(series.data[0].long_positions_with_entry, 1199);
    assert!(series.meta.position_totals().is_none());

    let now = c
        .hyperliquid
        .positions
        .market_summary("ETH", None)
        .await
        .unwrap();
    assert!(now.data.is_empty());
}

#[tokio::test]
async fn bulk_positions_use_an_exact_hour() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/positions"))
        .and(query_param("hour", "1790337600000"))
        .and(query_param("limit", "2000"))
        .respond_with(ok(
            json!([{
                "snapshot_ts": "2026-09-25T12:00:00.000Z",
                "user_address": WALLET,
                "symbol": "xyz:TSLA",
                "coin": "xyz:TSLA",
                "dex": "xyz",
                "size": "2",
                "side": "long",
                "entry_price": "410.5",
                "mark_price": "412",
                "position_value": "824",
                "unrealized_pnl": "3",
                "leverage_type": "isolated",
                "liquidation_price": "300",
                "quality": "partial"
            }]),
            json!({"count": 1, "request_id": "bulk", "next_cursor": "n"}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let page = c
        .hyperliquid
        .hip3
        .positions
        .all(Some(BulkPositionsParams {
            hour: Some(1790337600000_i64.into()),
            limit: Some(2000),
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(page.data[0].dex.as_deref(), Some("xyz"));
    assert_eq!(page.next_cursor.as_deref(), Some("n"));

    let error = offline_client()
        .hyperliquid
        .positions
        .all(Some(BulkPositionsParams {
            hour: Some(1790337600001_i64.into()),
            ..Default::default()
        }))
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::InvalidParam(ref m) if m.contains("exact UTC hour")),
        "{error}"
    );
}

#[tokio::test]
async fn venue_specific_filters_are_rejected_before_sending() {
    let c = offline_client();

    let dex_on_core = c
        .hyperliquid
        .positions
        .get(
            WALLET,
            Some(GetPositionsParams {
                dex: Some("xyz".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(dex_on_core, Error::InvalidParam(ref m) if m == "dex applies to HIP-3 routes only")
    );

    let dex_on_lighter = c
        .lighter
        .positions
        .changes(
            7,
            PositionRangeParams {
                dex: Some("xyz".into()),
                ..PositionRangeParams::new(1790294400000_i64, 1790380800000_i64)
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(dex_on_lighter, Error::InvalidParam(_)));

    let system_on_hl = c
        .hyperliquid
        .positions
        .market(
            "BTC",
            Some(MarketPositionsParams {
                include_system: Some(true),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(system_on_hl, Error::InvalidParam(ref m) if m == "include_system applies to Lighter routes only")
    );

    let bad_address = c
        .hyperliquid
        .positions
        .get("0x1234", None)
        .await
        .unwrap_err();
    assert!(matches!(bad_address, Error::InvalidParam(_)));
    let bad_l1 = c
        .lighter
        .accounts
        .by_l1("not-an-address", None, None)
        .await
        .unwrap_err();
    assert!(matches!(bad_l1, Error::InvalidParam(ref m) if m.contains("l1_address")));
}

#[tokio::test]
async fn an_advanced_snapshot_surfaces_as_a_409() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/positions/BTC"))
        .and(query_param("cursor", "stale"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "success": false,
            "error": "The snapshot this cursor was paging has been replaced or has expired. Restart pagination without a cursor.",
            "error_code": "snapshot_advanced"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .lighter
        .positions
        .market(
            "BTC",
            Some(MarketPositionsParams {
                cursor: Some("stale".to_string()),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
    match error {
        Error::Api { code, message, .. } => {
            assert_eq!(code, 409);
            assert!(message.contains("Restart pagination"));
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Account positions: Lighter and Robinhood Chain
// ---------------------------------------------------------------------------

fn lighter_position(account_index: &str) -> Value {
    json!({
        "account_index": account_index,
        "account_kind": "user",
        "symbol": "ETH",
        "coin": "ETH",
        "size": "3.25",
        "side": "long",
        "entry_price": "4012.55",
        "mark_price": "4100.1",
        "mark_time": "2026-09-25T00:00:00.000Z",
        "position_value": "13325.325",
        "unrealized_pnl": "284.5375",
        "return_on_equity": null,
        "leverage": {"type": "cross", "value": null},
        "max_leverage": null,
        "margin_used": null,
        "liquidation_price": null,
        "liquidation_price_status": "unavailable",
        "cum_funding": {"all_time": null, "since_open": null, "since_change": null},
        "opened_at": null,
        "snapshot_as_of": null,
        "quality": "preliminary",
        "initial_margin_fraction": "0.05",
        "allocated_margin": null,
        "margin_mode": "cross",
        "mark_source": "mark",
        "finalized": false
    })
}

#[tokio::test]
async fn lighter_and_rh_lighter_positions_are_keyed_by_account_index() {
    let server = MockServer::start().await;
    for (prefix, index) in [
        ("/v1/lighter", "713845"),
        ("/v1/rh-lighter", "281474976710654"),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/accounts/{index}/positions")))
            .and(query_param("timestamp", "1790294400000"))
            .respond_with(ok(
                json!({
                    "positions": [lighter_position(index)],
                    "account": {
                        "account_index": index,
                        "total_position_value": "13325.325",
                        "total_unrealized_pnl": "284.5375",
                        "long_value": "13325.325",
                        "short_value": "0",
                        "n_positions": 1,
                        "quality": "complete"
                    }
                }),
                json!({
                    "count": 1,
                    "request_id": "lp",
                    "as_of": "2026-09-25T00:00:00.000Z",
                    "snapshot_ts": "2026-09-25T00:00:00.000Z",
                    "source": "snapshot",
                    "quality": "complete"
                }),
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/accounts/{index}/positions/changes")))
            .and(query_param("start", "1790294400000"))
            .and(query_param("end", "1790380800000"))
            .and(query_param("symbol", "ETH"))
            .respond_with(ok(
                json!([{
                    "timestamp": "2026-09-25T03:00:00.000Z",
                    "account_index": index,
                    "account_kind": "user",
                    "symbol": "ETH",
                    "coin": "ETH",
                    "side": "B",
                    "price": "4012.55",
                    "size": "3.25",
                    "start_position": "0",
                    "end_position": "3.25",
                    "entry_price_after": "4012.55",
                    "event_type": "open",
                    "cause": "trade",
                    "realized_pnl": "0",
                    "fee": "0.652",
                    "fee_token": if prefix == "/v1/rh-lighter" { "USDG" } else { "USDC" },
                    "is_maker": false,
                    "trade_id": 31944180930_i64,
                    "order_id": 844421425107071_i64,
                    "opened_at": "2026-09-25T03:00:00.000Z",
                    "continuity": "ok",
                    "position_size_before": "0",
                    "position_size_after": "3.25",
                    "fee_rate": "0.00005",
                    "fee_usdc": "0.652",
                    "usdc_amount": "13040.7875",
                    "finalized": false
                }]),
                json!({"count": 1, "request_id": "lc", "source": "changes", "finalized_through": "2026-09-25T00:00:00.000Z"}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/positions/ETH")))
            .and(query_param("include_system", "true"))
            .respond_with(ok(json!([]), meta(0)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/positions")))
            .and(query_param("include_system", "true"))
            .and(query_param("hour", "1790337600000"))
            .respond_with(ok(json!([]), meta(0)))
            .expect(1)
            .mount(&server)
            .await;
    }

    let c = client(&server);
    for (positions, index, token) in [
        (&c.lighter.positions, 713845_u64, "USDC"),
        (&c.rh_lighter.positions, 281474976710654_u64, "USDG"),
    ] {
        let page = positions
            .get(
                index,
                Some(GetPositionsParams {
                    timestamp: Some(1790294400000_i64.into()),
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
        let p = &page.data.positions[0];
        assert_eq!(p.account_index.as_deref(), Some(index.to_string().as_str()));
        assert_eq!(p.quality, "preliminary");
        assert_eq!(p.finalized, Some(false));
        assert_eq!(p.initial_margin_fraction.as_deref(), Some("0.05"));
        assert_eq!(p.leverage.value, None);
        let account = page.data.account.as_ref().unwrap();
        assert_eq!(account.account_value, None);
        assert_eq!(account.total_position_value.as_deref(), Some("13325.325"));

        let changes = positions
            .changes(
                index,
                PositionRangeParams {
                    symbol: Some("ETH".to_string()),
                    ..PositionRangeParams::new(1790294400000_i64, 1790380800000_i64)
                },
            )
            .await
            .unwrap();
        let leg = &changes.data[0];
        assert_eq!(leg.fee_token, token);
        assert_eq!(leg.is_maker, Some(false));
        assert_eq!(leg.realized_pnl.as_deref(), Some("0"));
        assert_eq!(leg.position_size_after.as_deref(), Some("3.25"));
        assert_eq!(leg.crossed, None);
        assert_eq!(
            changes.meta.finalized_through.as_deref(),
            Some("2026-09-25T00:00:00.000Z")
        );

        positions
            .market(
                "ETH",
                Some(MarketPositionsParams {
                    include_system: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
        positions
            .all(Some(BulkPositionsParams {
                hour: Some(1790337600000_i64.into()),
                include_system: Some(true),
                ..Default::default()
            }))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn lighter_accounts_resolve_an_l1_address() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/accounts"))
        .and(query_param("l1_address", WALLET))
        .and(query_param("limit", "2"))
        .respond_with(ok(
            json!({
                "l1_address": WALLET,
                "total_accounts": 3,
                "accounts": [
                    {"account_index": "713845", "account_type": 0, "first_seen": "2025-02-01T00:00:00.000Z"},
                    {"account_index": "713846", "account_type": 1, "first_seen": null}
                ]
            }),
            json!({"count": 2, "request_id": "l1", "next_cursor": "more"}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .lighter
        .accounts
        .by_l1(WALLET, None, Some(2))
        .await
        .unwrap();
    assert_eq!(page.data.total_accounts, 3);
    assert_eq!(page.data.accounts.len(), 2);
    assert_eq!(page.data.accounts[1].account_type, 1);
    assert_eq!(page.next_cursor.as_deref(), Some("more"));
}

#[test]
fn public_copy_describes_robinhood_chain_as_a_lighter_deployment() {
    let readme = include_str!("../README.md");
    let normalized = readme.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized.contains("Lighter has two deployments: mainnet"));
    assert!(normalized.contains("2026-06-26 20:10:26 UTC"));
    assert!(normalized.contains("2026-08-22 18:43 UTC"));
    assert!(normalized
        .contains("| `rh_lighter_orderbook` | Lighter on Robinhood Chain L2 order book | Yes |"));
    assert!(normalized.contains("| `positions` | Yes | Yes | -- | Yes | Yes |"));
    assert!(normalized.contains("| `l3_orderbook` | -- | -- | -- | Yes | -- |"));
    assert!(!normalized.contains("third venue"));
    for doc in [
        include_str!("../src/exchanges.rs"),
        include_str!("../src/resources/positions.rs"),
    ] {
        assert!(!doc.contains("third venue"));
    }
}
