//! The 2026-10 API contract as the SDK sees it: the version header, the
//! envelope and time shapes it selects, typed error codes, `has_more` paging,
//! `meta.symbol` / `meta.venue`, `/v1/capabilities`, the filters the API
//! honours, and the verb aliases.

use oxarchive::exchanges::{
    Hip4HistoryRange, Hip4OrderBookHistoryParams, Hip4OrderHistoryParams, Hip4TradesParams,
};
use oxarchive::resources::l2_orderbook::L2HistoryParams;
use oxarchive::resources::l4_orderbook::L4OrderBookParams;
use oxarchive::resources::orderbook::{OrderBookHistoryParams, TickPageParams};
use oxarchive::resources::orders::OrderHistoryParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::{
    Capability, Error, ErrorCode, LevelsHistoryParams, LiquidationLevelsParams, OxArchive,
    RecentTradesParams, TradeSide, API_VERSION,
};
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> OxArchive {
    OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap()
}

fn ok(data: Value, meta: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"success": true, "data": data, "meta": meta}))
}

fn trades_params() -> GetTradesParams {
    GetTradesParams {
        start: 1790553600000_i64.into(),
        end: 1790557200000_i64.into(),
        cursor: None,
        limit: Some(2),
        side: None,
    }
}

fn trade(side: &str) -> Value {
    json!({
        "coin": "BTC", "symbol": "BTC", "side": side, "price": "84289", "size": "0.02554",
        "timestamp": "2026-09-29T13:57:55.246Z", "trade_id": 166, "crossed": true
    })
}

// ---------------------------------------------------------------------------
// Version selector
// ---------------------------------------------------------------------------

#[tokio::test]
async fn every_request_sends_the_api_version() {
    assert_eq!(API_VERSION, "2026-10-01");
    let server = MockServer::start().await;
    Mock::given(header("0xArchive-Version", "2026-10-01"))
        .respond_with(ok(
            json!([]),
            json!({"count": 0, "request_id": "r", "has_more": false}),
        ))
        .expect(3)
        .mount(&server)
        .await;
    let c = client(&server);
    c.hyperliquid
        .trades
        .list("BTC", trades_params())
        .await
        .unwrap();
    c.lighter.trades.recent("BTC", Some(1)).await.unwrap();
    c.data_quality.positions_freshness().await.unwrap();
}

// ---------------------------------------------------------------------------
// Envelope outliers: data quality, /v1/status/coverage, /v1/symbols
// ---------------------------------------------------------------------------

#[tokio::test]
async fn data_quality_status_and_coverage_read_the_envelope() {
    let server = MockServer::start().await;
    // The versioned bodies: `data` is the old body, `meta` has no `count`.
    Mock::given(method("GET"))
        .and(path("/v1/data-quality/status"))
        .respond_with(ok(
            json!({"status": "operational", "updated_at": "2026-09-29T14:56:01Z",
                   "exchanges": {"hyperliquid": {"status": "operational"}},
                   "data_types": {}, "active_incidents": 0}),
            json!({"request_id": "req-status"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let coverage = json!({"exchanges": [{"exchange": "hyperliquid", "data_types": {
        "orderbook": {"earliest": "2023-04-15T00:00:07Z", "latest": "2026-09-29T14:56:01Z",
                      "total_records": 28098760700_i64, "completeness": 100.0}}}]});
    for route in ["/v1/data-quality/coverage", "/v1/status/coverage"] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ok(coverage.clone(), json!({"request_id": "req-cov"})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let c = client(&server);
    let status = c.data_quality.status().await.unwrap();
    assert_eq!(status.status, "operational");
    assert_eq!(status.active_incidents, Some(0));
    for cov in [
        c.data_quality.coverage().await.unwrap(),
        c.data_quality.status_coverage().await.unwrap(),
    ] {
        assert_eq!(cov.exchanges.len(), 1);
        assert_eq!(cov.exchanges[0].exchange, "hyperliquid");
        assert_eq!(
            cov.exchanges[0].data_types["orderbook"].total_records,
            Some(28098760700)
        );
    }
}

#[tokio::test]
async fn data_quality_still_reads_the_unversioned_body() {
    let server = MockServer::start().await;
    // Without the version the body is unchanged apart from `success` and a
    // trailing `meta`.
    Mock::given(method("GET"))
        .and(path("/v1/data-quality/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "status": "degraded", "exchanges": {}, "data_types": {},
            "meta": {"request_id": "req-1"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let status = client(&server).data_quality.status().await.unwrap();
    assert_eq!(status.status, "degraded");
}

#[tokio::test]
async fn symbols_read_the_envelope_data_array() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/symbols"))
        .respond_with(ok(
            json!([
                {"symbol": "#0", "exchange": "hip4", "coverage_from": "2026-05-02T00:00:00Z",
                 "data_types": ["trades"], "coverage_by_type": {"trades": "2026-05-02T00:00:00Z"},
                 "size_per_day": {"trades": 3.93}, "is_settled": true},
                {"symbol": "BTC", "exchange": "hyperliquid", "data_types": ["orderbook"]}
            ]),
            json!({"count": 2, "request_id": "req-sym"}),
        ))
        .expect(2)
        .mount(&server)
        .await;
    let c = client(&server);
    let symbols = c.symbols().await.unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].symbol, "#0");
    assert_eq!(symbols[0].is_settled, Some(true));
    assert_eq!(symbols[1].exchange, "hyperliquid");
    assert_eq!(c.list_symbols().await.unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Time outliers: RFC 3339 with *_ms twins
// ---------------------------------------------------------------------------

#[tokio::test]
async fn l4_snapshot_orders_carry_queue_time() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/orderbook/BTC/l4"))
        .and(query_param("depth", "2"))
        .respond_with(ok(
            json!({
                "coin": "BTC", "timestamp": "2026-09-29T14:55:29.345Z",
                "checkpoint_timestamp": "2026-09-29T14:55:29.345Z", "diffs_applied": 0,
                "last_block_number": 1165511696_u64, "bid_count": 1, "ask_count": 1,
                "total_bid_size": 0.1, "total_ask_size": 6.5,
                "bids": [{"oid": 560239056846_u64, "price": 83849.0, "side": "B", "size": 0.1,
                          "timestamp": "2026-09-29T14:55:26.174Z", "timestamp_ms": 1790693726174_i64,
                          "user_address": "0x6540"}],
                "asks": [{"oid": 560233980357_u64, "price": 83850.0, "side": "A", "size": 6.5,
                          "timestamp": null, "timestamp_ms": null, "user_address": "0x66f8"}]
            }),
            json!({"count": 2, "request_id": "r", "symbol": "BTC", "venue": "hyperliquid"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let book = client(&server)
        .hyperliquid
        .l4_orderbook
        .get(
            "BTC",
            Some(L4OrderBookParams {
                timestamp: None,
                depth: Some(2),
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        book.bids[0].timestamp.as_deref(),
        Some("2026-09-29T14:55:26.174Z")
    );
    assert_eq!(book.bids[0].timestamp_ms, Some(1790693726174));
    // Queue time unknown: null, never the epoch.
    assert_eq!(book.asks[0].timestamp, None);
    assert_eq!(book.asks[0].timestamp_ms, None);
}

#[tokio::test]
async fn l4_checkpoint_history_orders_read_integer_queue_times() {
    let server = MockServer::start().await;
    // Checkpoint history rows carry the queue time as integer milliseconds.
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/orderbook/BTC/l4/history"))
        .respond_with(ok(
            json!([{
                "coin": "BTC", "timestamp": "2026-09-29T13:42:26.211Z",
                "checkpoint_timestamp": "2026-09-29T13:42:26.211Z", "diffs_applied": 0,
                "last_block_number": 1165450000_u64, "bid_count": 1, "ask_count": 1,
                "total_bid_size": 0.0022, "total_ask_size": 0.05934,
                "bids": [{"oid": 560154109569_u64, "user_address": "0x31de", "side": "B",
                          "price": 84218.0, "size": 0.0022, "timestamp": 1790689430487_i64}],
                "asks": [{"oid": 560154121856_u64, "user_address": "0xa1d2", "side": "A",
                          "price": 84219.0, "size": 0.05934, "timestamp": 0}]
            }]),
            json!({"count": 1, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .hyperliquid
        .l4_orderbook
        .history(
            "BTC",
            oxarchive::resources::l4_orderbook::L4HistoryParams {
                start: 1.into(),
                end: 2.into(),
                cursor: None,
                limit: Some(1),
            },
        )
        .await
        .unwrap();
    let bid = &page.data[0].bids[0];
    assert_eq!(bid.timestamp.as_deref(), Some("2026-09-29T13:43:50.487Z"));
    assert_eq!(bid.timestamp_ms, Some(1790689430487));
    let ask = &page.data[0].asks[0];
    assert_eq!(ask.timestamp, None);
    assert_eq!(ask.timestamp_ms, None);
}

#[tokio::test]
async fn levels_and_trigger_levels_carry_snapshot_ts_ms() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/liquidations/BTC/levels"))
        .respond_with(ok(
            json!({"mid_price": 83787.0, "snapshot_ts": "2026-09-29T14:51:47.000Z",
                   "snapshot_ts_ms": 1790693507000_i64, "block_number": 1165450000_u64,
                   "total_long": 1.0, "total_short": 2.0, "flagged_notional": 0.5,
                   "source": "raw", "levels": []}),
            json!({"count": 0, "request_id": "r", "symbol": "BTC", "venue": "hyperliquid"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/liquidations/BTC/levels/history"))
        .respond_with(ok(
            json!([{"snapshot_ts": "2026-09-29T12:56:47.000Z", "snapshot_ts_ms": 1790686607000_i64,
                    "block_number": 1165370000_u64, "mid_price": 84332.1, "total_long": 1.0,
                    "total_short": 2.0, "flagged_notional": 0.5}]),
            json!({"count": 1, "request_id": "r", "has_more": true,
                   "next_cursor": "1790686909000", "symbol": "BTC", "venue": "hyperliquid"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/orders/BTC/trigger-levels/history"))
        .respond_with(ok(
            json!([{"snapshot_ts": "2026-09-29T13:15:01.000Z", "snapshot_ts_ms": 1790687701000_i64,
                    "mid_price": 84374.0, "total_bid_size": 3.0, "total_ask_size": 2.0}]),
            json!({"count": 1, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let now = c
        .hyperliquid
        .liquidations
        .levels("BTC", LiquidationLevelsParams::default())
        .await
        .unwrap();
    assert_eq!(now.snapshot_ts, "2026-09-29T14:51:47.000Z");
    assert_eq!(now.snapshot_ts_ms, Some(1790693507000));

    let history = c
        .hyperliquid
        .liquidations
        .levels_history("BTC", LevelsHistoryParams::default())
        .await
        .unwrap();
    assert_eq!(history.data[0].snapshot_ts_ms, Some(1790686607000));
    assert!(history.has_more);
    assert_eq!(history.meta.symbol.as_deref(), Some("BTC"));

    let triggers = c
        .hyperliquid
        .orders
        .trigger_levels_history("BTC", LevelsHistoryParams::default())
        .await
        .unwrap();
    assert_eq!(triggers.data[0].snapshot_ts, "2026-09-29T13:15:01.000Z");
    assert_eq!(triggers.data[0].snapshot_ts_ms, Some(1790687701000));
    assert!(!triggers.has_more);
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn api_errors_expose_the_contract_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/trades/BTC"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "success": false, "code": 400, "error_code": "invalid_parameter",
            "error": "Invalid side 'B'. Use buy or sell.", "request_id": "req-400",
            "param": "side", "valid_values": ["buy", "sell"]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/orders/0/flow"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "success": false, "code": 404, "error_code": "unsupported_for_venue",
            "error": "Order flow is not offered on HIP-4.", "request_id": "req-404",
            "datatype": "order_flow", "venue": "hip4",
            "available_on": [{"route": "/v1/hyperliquid/orders/{symbol}/flow", "venue": "hyperliquid"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/funding/BTC/current"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "success": false, "code": 401, "error_code": "unauthorized",
            "error": "Missing authentication credentials.", "request_id": "req-401"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/openinterest/BTC/current"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "success": false, "code": 429, "error_code": "rate_limited", "error": "Slow down"
        })))
        .mount(&server)
        .await;
    let c = client(&server);

    match c
        .hyperliquid
        .trades
        .list("BTC", trades_params())
        .await
        .unwrap_err()
    {
        Error::Api {
            message,
            code,
            request_id,
            error_code,
            param,
            valid_values,
        } => {
            assert_eq!(code, 400);
            assert_eq!(error_code, Some(ErrorCode::InvalidParameter));
            assert_eq!(request_id.as_deref(), Some("req-400"));
            assert_eq!(param.as_deref(), Some("side"));
            assert_eq!(
                valid_values.as_deref(),
                Some(&["buy".to_string(), "sell".to_string()][..])
            );
            assert!(message.contains("buy or sell"));
        }
        other => panic!("expected an API error, got {other:?}"),
    }

    let unsupported = c
        .hyperliquid
        .hip4
        .get_order_flow("0", Default::default())
        .await
        .unwrap_err();
    assert_eq!(
        unsupported.error_code(),
        Some(&ErrorCode::UnsupportedForVenue)
    );
    assert_eq!(unsupported.status(), Some(404));
    assert!(unsupported
        .to_string()
        .ends_with("(HTTP 404, unsupported_for_venue)"));

    let unauthorized = c.hyperliquid.funding.current("BTC").await.unwrap_err();
    assert_eq!(unauthorized.error_code(), Some(&ErrorCode::Unauthorized));
    assert_eq!(unauthorized.request_id(), Some("req-401"));
    assert_eq!(unauthorized.param(), None);

    let limited = c
        .hyperliquid
        .open_interest
        .current("BTC")
        .await
        .unwrap_err();
    assert!(matches!(
        limited,
        Error::Api {
            error_code: Some(ErrorCode::RateLimited),
            code: 429,
            ..
        }
    ));
}

#[test]
fn every_contract_code_round_trips() {
    let codes = [
        ("invalid_parameter", ErrorCode::InvalidParameter),
        ("invalid_symbol", ErrorCode::InvalidSymbol),
        ("invalid_interval", ErrorCode::InvalidInterval),
        ("invalid_cursor", ErrorCode::InvalidCursor),
        ("invalid_time_range", ErrorCode::InvalidTimeRange),
        ("range_before_coverage", ErrorCode::RangeBeforeCoverage),
        (
            "historical_range_exceeded",
            ErrorCode::HistoricalRangeExceeded,
        ),
        (
            "historical_depth_exceeded",
            ErrorCode::HistoricalDepthExceeded,
        ),
        ("unsupported_for_venue", ErrorCode::UnsupportedForVenue),
        ("route_not_found", ErrorCode::RouteNotFound),
        ("not_found", ErrorCode::NotFound),
        ("unauthorized", ErrorCode::Unauthorized),
        ("forbidden", ErrorCode::Forbidden),
        ("insufficient_credits", ErrorCode::InsufficientCredits),
        ("rate_limited", ErrorCode::RateLimited),
        ("conflict", ErrorCode::Conflict),
        ("upstream_unavailable", ErrorCode::UpstreamUnavailable),
        ("internal_error", ErrorCode::InternalError),
        ("endpoint_unsupported", ErrorCode::EndpointUnsupported),
        ("slow_consumer", ErrorCode::SlowConsumer),
        ("positions_unavailable", ErrorCode::PositionsUnavailable),
        ("api_key_limit_reached", ErrorCode::ApiKeyLimitReached),
        ("oauth_not_permitted", ErrorCode::OauthNotPermitted),
    ];
    for (wire, code) in codes {
        assert_eq!(ErrorCode::from(wire), code);
        assert_eq!(code.as_str(), wire);
        assert_eq!(code.to_string(), wire);
        assert_eq!(serde_json::to_value(&code).unwrap(), json!(wire));
        let parsed: ErrorCode = serde_json::from_value(json!(wire)).unwrap();
        assert_eq!(parsed, code);
        assert_eq!(wire.parse::<ErrorCode>().unwrap(), code);
    }
    let unknown: ErrorCode = serde_json::from_value(json!("invalid_query_params")).unwrap();
    assert_eq!(
        unknown,
        ErrorCode::Other("invalid_query_params".to_string())
    );
    assert_eq!(unknown.as_str(), "invalid_query_params");
}

#[test]
fn non_api_errors_have_no_contract_fields() {
    let err = Error::InvalidParam("bad".to_string());
    assert_eq!(err.error_code(), None);
    assert_eq!(err.status(), None);
    assert_eq!(err.request_id(), None);
    assert_eq!(err.param(), None);
    assert_eq!(err.valid_values(), None);
}

// ---------------------------------------------------------------------------
// Paging: has_more, next_cursor, meta.symbol and meta.venue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn paging_follows_has_more_and_exposes_symbol_and_venue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/trades/0"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ok(
            json!([trade("B"), trade("A")]),
            json!({"count": 2, "request_id": "r1", "has_more": true,
                   "next_cursor": "1790536551191_470887163636273",
                   "symbol": "#0", "venue": "hip4"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    // The page after an exactly full one can be empty; has_more is false.
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/trades/0"))
        .and(query_param("cursor", "1790536551191_470887163636273"))
        .respond_with(ok(
            json!([]),
            json!({"count": 0, "request_id": "r2", "has_more": false,
                   "symbol": "#0", "venue": "hip4"}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let mut cursor = None;
    let mut rows = 0;
    let mut pages = 0;
    loop {
        let page = c
            .hyperliquid
            .hip4
            .get_trades(
                "0",
                Hip4TradesParams {
                    start: 1790553600000_i64.into(),
                    end: 1790640000000_i64.into(),
                    cursor,
                    limit: Some(2),
                    side: None,
                },
            )
            .await
            .unwrap();
        pages += 1;
        rows += page.data.len();
        // The path input "0" is answered as the canonical "#0".
        assert_eq!(page.meta.symbol.as_deref(), Some("#0"));
        assert_eq!(page.meta.venue.as_deref(), Some("hip4"));
        assert_eq!(page.has_more, page.next_cursor.is_some());
        assert_eq!(page.meta.has_more, Some(page.has_more));
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    assert_eq!((pages, rows), (2, 2));
}

#[tokio::test]
async fn has_more_falls_back_to_the_cursor_when_absent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/trades/BTC"))
        .respond_with(ok(
            json!([trade("B")]),
            json!({"count": 1, "request_id": "r", "next_cursor": "c-2"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/trades/BTC"))
        .respond_with(ok(
            json!([trade("A")]),
            json!({"count": 1, "request_id": "r"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let with_cursor = c
        .hyperliquid
        .trades
        .list("BTC", trades_params())
        .await
        .unwrap();
    assert!(with_cursor.has_more);
    assert_eq!(with_cursor.meta.has_more, None);
    let without = c.lighter.trades.list("BTC", trades_params()).await.unwrap();
    assert!(!without.has_more);
    assert_eq!(without.next_cursor, None);
}

#[tokio::test]
async fn tick_history_collection_follows_the_cursor_until_has_more_is_false() {
    let server = MockServer::start().await;
    let checkpoint = json!({"coin": "BTC", "timestamp": "2026-09-29T14:54:36.787Z",
        "bids": [{"px": "83774.9", "sz": "3.9", "n": 1}],
        "asks": [{"px": "83775.9", "sz": "0.1", "n": 1}]});
    let delta = |ts: i64, seq: i64| json!({"timestamp": ts, "side": "bid", "price": 83775.8, "size": 0.2, "sequence": seq});
    // The first page: the checkpoint and a page of deltas shorter than
    // 1,000, with more to follow.
    Mock::given(method("GET"))
        .and(path("/v1/lighter/orderbook/BTC/history"))
        .and(query_param("granularity", "tick"))
        .and(query_param("start", "1790693600000"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ok(
            json!({"checkpoint": checkpoint, "deltas": [delta(1790693599100, 1), delta(1790693600200, 2)],
                   "granularity": "tick", "next_cursor": "1790693600200", "next_cursor_seq": 2}),
            json!({"count": 3, "request_id": "r1", "has_more": true, "next_cursor": "1790693600200"}),
        ))
        .expect(2)
        .mount(&server)
        .await;
    // The next page keeps `start` and sends the cursor; it has no checkpoint.
    Mock::given(method("GET"))
        .and(path("/v1/lighter/orderbook/BTC/history"))
        .and(query_param("granularity", "tick"))
        .and(query_param("start", "1790693600000"))
        .and(query_param("cursor", "1790693600200"))
        .and(query_param("cursor_seq", "2"))
        .respond_with(ok(
            json!({"checkpoint": null, "deltas": [delta(1790693600200, 3)], "granularity": "tick",
                   "next_cursor": null, "next_cursor_seq": null}),
            json!({"count": 1, "request_id": "r2", "has_more": false}),
        ))
        .expect(2)
        .mount(&server)
        .await;
    let book = &client(&server).lighter.orderbook;
    let snapshots = book
        .collect_tick_history("BTC", 1790693600000_i64, 1790693700000_i64, None)
        .await
        .unwrap();
    // The checkpoint plus one snapshot per delta across both pages.
    assert_eq!(snapshots.len(), 4);

    // The same pages by hand.
    let params = TickPageParams::new(1790693600000_i64, 1790693700000_i64);
    let first = book.history_tick_page("BTC", params.clone()).await.unwrap();
    assert!(first.checkpoint.is_some());
    assert!(first.has_more);
    assert_eq!(first.next_cursor.as_deref(), Some("1790693600200"));
    assert_eq!(first.next_cursor_seq, Some(2));
    let second = book
        .history_tick_page("BTC", params.after(&first))
        .await
        .unwrap();
    assert!(second.checkpoint.is_none());
    assert_eq!(second.deltas.len(), 1);
    assert!(!second.has_more);
    assert_eq!(second.next_cursor, None);
}

#[tokio::test]
async fn triggered_order_rows_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/orders/BTC/history"))
        .and(query_param("triggered", "true"))
        .respond_with(ok(
            json!([{"block_number": 1164345047_u64, "block_time": "2026-09-28T15:37:54.359Z",
                    "coin": "BTC", "is_position_tpsl": false, "is_trigger": true,
                    "limit_price": 84000.0, "oid": 1_u64, "order_type": "Stop Market",
                    "orig_size": 0.1, "reduce_only": true, "seq": 2165, "side": "A", "size": 0.1,
                    "status": "triggered", "timestamp": "2026-09-28T15:37:54.359Z",
                    "trigger_condition": "Price below 84000", "trigger_price": 84000.0,
                    "user_address": "0xbda3"}]),
            json!({"count": 1, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .hyperliquid
        .orders
        .history(
            "BTC",
            OrderHistoryParams {
                triggered: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let row = &page.data[0];
    assert_eq!(row.status, "triggered");
    assert_eq!(row.tif, "");
    assert_eq!(row.cloid, None);
    assert_eq!(row.trigger_condition.as_deref(), Some("Price below 84000"));
    assert_eq!(row.trigger_price, Some(84000.0));
}

#[tokio::test]
async fn spot_twap_reads_numeric_sizes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/spot/twap/HYPE-USDC"))
        .respond_with(ok(
            json!([{"timestamp": "2026-09-28T15:44:41.426Z", "block_time": "2026-09-28T15:44:41.426Z",
                    "block_number": 1164350670_u64, "twap_id": 2264348, "status": "activated",
                    "coin": "HYPE-USDC", "user_address": "0xa41b", "side": "A", "size": 20.0,
                    "executed_size": 2.86, "executed_notional": 0.0, "minutes": 10,
                    "reduce_only": false, "randomize": false,
                    "started_at": "2026-09-28T15:44:41.426Z"}]),
            json!({"count": 1, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .hyperliquid
        .spot
        .twap
        .by_symbol("HYPE-USDC", Default::default())
        .await
        .unwrap();
    assert_eq!(page.data[0].executed_size.as_deref(), Some("2.86"));
    assert_eq!(page.data[0].executed_notional.as_deref(), Some("0"));
    assert_eq!(page.data[0].twap_id, 2264348);
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn capabilities_are_typed_rows() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/capabilities"))
        .respond_with(ok(
            json!([
                {"venue": "hyperliquid", "datatype": "l2_full_depth",
                 "rest_routes": ["/v1/hyperliquid/orderbook/{symbol}/l2",
                                 "/v1/hyperliquid/orderbook/{symbol}/l2/history"],
                 "ws_channels": ["orderbook_full"], "live": true, "replay": true,
                 "available_from": "2026-03-11T01:03:00.000Z", "cadence": "snapshot",
                 "page_limit": 100, "intervals": [],
                 "notes": "Derived from L4. WebSocket replay is bulk (speed is ignored) and single-channel only, anchored at L4 checkpoints."},
                {"venue": "spot", "datatype": "trades",
                 "rest_routes": ["/v1/hyperliquid/spot/trades/{symbol}"],
                 "ws_channels": ["spot_trades"], "live": true, "replay": false,
                 "available_from": "2025-03-22T10:50:22.000Z", "cadence": "event",
                 "page_limit": 1000, "intervals": [], "notes": null},
                {"venue": "hyperliquid", "datatype": "candles", "rest_routes": ["/v1/hyperliquid/candles/{symbol}"],
                 "ws_channels": ["candles"], "live": false, "replay": true,
                 "available_from": "2025-03-01T00:00:00.000Z", "cadence": "interval",
                 "page_limit": 10000, "intervals": ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"],
                 "notes": null, "a_future_field": 1},
                {"venue": "hyperliquid", "datatype": "summary", "rest_routes": ["/v1/hyperliquid/summary/{symbol}"],
                 "ws_channels": [], "live": false, "replay": false, "available_from": null,
                 "cadence": "snapshot", "page_limit": null, "intervals": [], "notes": null}
            ]),
            json!({"count": 4, "request_id": "r"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let rows = client(&server).capabilities().await.unwrap();
    assert_eq!(rows.len(), 4);
    let full = &rows[0];
    assert_eq!(full.venue, "hyperliquid");
    assert_eq!(full.datatype, "l2_full_depth");
    assert_eq!(full.rest_routes.len(), 2);
    assert!(full.live && full.replay);
    assert_eq!(
        full.available_from.as_deref(),
        Some("2026-03-11T01:03:00.000Z")
    );
    assert_eq!(full.page_limit, Some(100));
    assert!(full.notes.as_deref().unwrap().contains("single-channel"));
    assert_eq!(rows[2].intervals.len(), 8);
    assert_eq!(rows[3].available_from, None);
    assert_eq!(rows[3].page_limit, None);

    let spot = Capability::for_channel(&rows, "spot_trades").unwrap();
    assert!(spot.live && !spot.replay);
    assert!(
        Capability::for_channel(&rows, "orderbook_full")
            .unwrap()
            .replay
    );
    assert!(Capability::for_channel(&rows, "nope").is_none());
}

// ---------------------------------------------------------------------------
// Filters the API honours
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trades_side_is_sent_on_every_venue_and_on_recent() {
    let server = MockServer::start().await;
    for prefix in [
        "/v1/hyperliquid",
        "/v1/hyperliquid/hip3",
        "/v1/hyperliquid/spot",
        "/v1/lighter",
        "/v1/rh-lighter",
    ] {
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/trades/SYM")))
            .and(query_param("side", "buy"))
            .respond_with(ok(
                json!([trade("B")]),
                json!({"count": 1, "request_id": "r", "has_more": false}),
            ))
            .expect(2)
            .mount(&server)
            .await;
    }
    for prefix in [
        "/v1/hyperliquid/hip3",
        "/v1/hyperliquid/spot",
        "/v1/lighter",
        "/v1/rh-lighter",
    ] {
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/trades/SYM/recent")))
            .and(query_param("side", "sell"))
            .and(query_param("limit", "5"))
            .respond_with(ok(
                json!([trade("A")]),
                json!({"count": 1, "request_id": "r"}),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/trades/0"))
        .and(query_param("side", "sell"))
        .respond_with(ok(
            json!([trade("A")]),
            json!({"count": 1, "request_id": "r"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/trades/0/recent"))
        .and(query_param("side", "buy"))
        .respond_with(ok(
            json!([trade("B")]),
            json!({"count": 1, "request_id": "r"}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let buys = || GetTradesParams {
        side: Some(TradeSide::Buy),
        ..trades_params()
    };
    let venues = [
        &c.hyperliquid.trades,
        &c.hyperliquid.hip3.trades,
        &c.hyperliquid.spot.trades,
        &c.lighter.trades,
        &c.rh_lighter.trades,
    ];
    for trades in venues {
        assert_eq!(trades.list("SYM", buys()).await.unwrap().data[0].side, "B");
        assert_eq!(
            trades.history("SYM", buys()).await.unwrap().data[0].side,
            "B"
        );
    }
    for trades in &venues[1..] {
        let sells = trades
            .recent_with(
                "SYM",
                RecentTradesParams {
                    limit: Some(5),
                    side: Some(TradeSide::Sell),
                },
            )
            .await
            .unwrap();
        assert_eq!(sells.data[0].side, "A");
    }
    // Hyperliquid core has no recent route, with or without a filter.
    assert!(matches!(
        c.hyperliquid
            .trades
            .recent_with("BTC", RecentTradesParams::default())
            .await,
        Err(Error::InvalidParam(_))
    ));

    let hip4 = c
        .hyperliquid
        .hip4
        .get_trades(
            "0",
            Hip4TradesParams {
                start: 1.into(),
                end: 2.into(),
                cursor: None,
                limit: None,
                side: Some(TradeSide::Sell),
            },
        )
        .await
        .unwrap();
    assert_eq!(hip4.data[0].side, "A");
    let hip4_recent = c
        .hyperliquid
        .hip4
        .get_trades_recent_with(
            "0",
            RecentTradesParams {
                limit: None,
                side: Some(TradeSide::Buy),
            },
        )
        .await
        .unwrap();
    assert_eq!(hip4_recent.data[0].side, "B");
    assert_eq!(TradeSide::Buy.as_str(), "buy");
    assert_eq!(TradeSide::Sell.as_str(), "sell");
}

#[tokio::test]
async fn order_history_sends_triggered() {
    let server = MockServer::start().await;
    for (route, value) in [
        ("/v1/hyperliquid/orders/BTC/history", "true"),
        ("/v1/hyperliquid/hip3/orders/xyz:XYZ100/history", "false"),
        ("/v1/hyperliquid/hip4/orders/0/history", "true"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .and(query_param("triggered", value))
            .respond_with(ok(
                json!([]),
                json!({"count": 0, "request_id": "r", "has_more": false}),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/orders/ETH/history"))
        .and(query_param_is_missing("triggered"))
        .respond_with(ok(
            json!([]),
            json!({"count": 0, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    c.hyperliquid
        .orders
        .history(
            "BTC",
            OrderHistoryParams {
                triggered: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    c.hyperliquid
        .hip3
        .orders
        .history(
            "xyz:XYZ100",
            OrderHistoryParams {
                triggered: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    c.hyperliquid
        .hip4
        .get_order_history(
            "0",
            Hip4OrderHistoryParams {
                triggered: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    c.hyperliquid
        .orders
        .history("ETH", OrderHistoryParams::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn order_book_histories_send_depth() {
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/orderbook/BTC/l2/history",
        "/v1/hyperliquid/hip3/orderbook/xyz:XYZ100/l2/history",
        "/v1/hyperliquid/hip3/orderbook/xyz:XYZ100/history",
        "/v1/hyperliquid/spot/orderbook/HYPE-USDC/history",
        "/v1/hyperliquid/hip4/orderbook/0/history",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .and(query_param("depth", "3"))
            .respond_with(ok(
                json!([]),
                json!({"count": 0, "request_id": "r", "has_more": false}),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }
    // A plain range still works for HIP-4 and sends no depth.
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/orderbook/1/history"))
        .and(query_param_is_missing("depth"))
        .respond_with(ok(
            json!([]),
            json!({"count": 0, "request_id": "r", "has_more": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let l2 = || L2HistoryParams {
        start: 1.into(),
        end: 2.into(),
        cursor: None,
        limit: Some(1),
        depth: Some(3),
    };
    c.hyperliquid
        .l2_orderbook
        .history("BTC", l2())
        .await
        .unwrap();
    c.hyperliquid
        .hip3
        .l2_orderbook
        .history("xyz:XYZ100", l2())
        .await
        .unwrap();
    let book = || OrderBookHistoryParams {
        start: 1.into(),
        end: 2.into(),
        cursor: None,
        limit: Some(1),
        depth: Some(3),
        granularity: None,
    };
    c.hyperliquid
        .hip3
        .orderbook
        .history("xyz:XYZ100", book())
        .await
        .unwrap();
    c.hyperliquid
        .spot
        .orderbook
        .history("HYPE-USDC", book())
        .await
        .unwrap();
    c.hyperliquid
        .hip4
        .get_orderbook_history(
            "0",
            Hip4OrderBookHistoryParams {
                start: 1.into(),
                end: 2.into(),
                cursor: None,
                limit: Some(1),
                depth: Some(3),
            },
        )
        .await
        .unwrap();
    c.hyperliquid
        .hip4
        .get_orderbook_history(
            "1",
            Hip4HistoryRange {
                start: 1.into(),
                end: 2.into(),
                cursor: None,
                limit: Some(1),
            },
        )
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Verb aliases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hip4_aliases_call_the_same_routes() {
    let server = MockServer::start().await;
    let empty_page = || {
        ok(
            json!([]),
            json!({"count": 0, "request_id": "r", "has_more": false}),
        )
    };
    for route in [
        "/v1/hyperliquid/hip4/trades/0",
        "/v1/hyperliquid/hip4/openinterest/0",
        "/v1/hyperliquid/hip4/prices/0",
        "/v1/hyperliquid/hip4/instruments",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(empty_page())
            .expect(2)
            .mount(&server)
            .await;
    }
    let hip4 = &client(&server).hyperliquid.hip4;
    let trades = || Hip4TradesParams {
        start: 1.into(),
        end: 2.into(),
        cursor: None,
        limit: None,
        side: None,
    };
    hip4.get_trades("0", trades()).await.unwrap();
    hip4.get_trades_history("0", trades()).await.unwrap();
    let range = || Hip4HistoryRange {
        start: 1.into(),
        end: 2.into(),
        cursor: None,
        limit: None,
    };
    hip4.get_open_interest("0", range()).await.unwrap();
    hip4.get_open_interest_history("0", range()).await.unwrap();
    hip4.get_prices("0", 1, 2, Some("1h"), None, None)
        .await
        .unwrap();
    hip4.get_price_history("0", 1, 2, Some("1h"), None, None)
        .await
        .unwrap();
    hip4.get_instruments().await.unwrap();
    hip4.list_instruments().await.unwrap();
}

// ---------------------------------------------------------------------------
// Venue name
// ---------------------------------------------------------------------------

#[test]
fn the_venue_is_named_lighter() {
    for (name, text) in [
        ("README.md", include_str!("../README.md")),
        ("src/lib.rs", include_str!("../src/lib.rs")),
        ("src/client.rs", include_str!("../src/client.rs")),
        ("src/types.rs", include_str!("../src/types.rs")),
        ("src/exchanges.rs", include_str!("../src/exchanges.rs")),
        (
            "src/resources/trades.rs",
            include_str!("../src/resources/trades.rs"),
        ),
        (
            "src/resources/orderbook.rs",
            include_str!("../src/resources/orderbook.rs"),
        ),
        (
            "src/resources/instruments.rs",
            include_str!("../src/resources/instruments.rs"),
        ),
        (
            "src/resources/l3_orderbook.rs",
            include_str!("../src/resources/l3_orderbook.rs"),
        ),
        (
            "src/orderbook_reconstructor.rs",
            include_str!("../src/orderbook_reconstructor.rs"),
        ),
        ("src/ws/client.rs", include_str!("../src/ws/client.rs")),
        ("examples/basic.rs", include_str!("../examples/basic.rs")),
        (
            "examples/websocket.rs",
            include_str!("../examples/websocket.rs"),
        ),
    ] {
        assert!(
            !text.contains("Lighter.xyz"),
            "{name} still says Lighter.xyz"
        );
    }
}
