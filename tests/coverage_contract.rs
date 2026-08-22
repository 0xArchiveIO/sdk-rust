use oxarchive::exchanges::Hip4HistoryRange;
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::{CandleInterval, Hip4OpenInterestRecord, Timestamp};
use oxarchive::OxArchive;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn hip4_candles_use_the_typed_resource_and_encoded_coin_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/candles/%230"))
        .and(header("x-api-key", "test-key"))
        .and(query_param("start", "1777708800000"))
        .and(query_param("end", "1777712400000"))
        .and(query_param("limit", "1000"))
        .and(query_param("interval", "1h"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [{
                "timestamp": "2026-05-02T08:00:00Z",
                "open": "0.2",
                "high": "0.3",
                "low": "0.1",
                "close": "0.25",
                "volume": "10"
            }],
            "meta": {
                "count": 1,
                "request_id": "test-request",
                "next_cursor": "next-cursor"
            }
        })))
        .mount(&server)
        .await;

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let result = client
        .hyperliquid
        .hip4
        .candles
        .history(
            "#0",
            CandleHistoryParams {
                start: Timestamp::from("2026-05-02T08:00:00Z"),
                end: Timestamp::from("2026-05-02T09:00:00Z"),
                cursor: None,
                limit: Some(1000),
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.next_cursor.as_deref(), Some("next-cursor"));
}

#[tokio::test]
async fn hip4_candles_reject_limits_above_the_route_maximum() {
    let client = OxArchive::builder("test-key")
        .base_url("http://127.0.0.1:9")
        .build()
        .unwrap();

    let error = client
        .hyperliquid
        .hip4
        .candles
        .history(
            "#0",
            CandleHistoryParams {
                start: Timestamp::from("2026-05-02T08:00:00Z"),
                end: Timestamp::from("2026-05-02T09:00:00Z"),
                cursor: None,
                limit: Some(1001),
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("1 and 1000"));
}

#[tokio::test]
async fn spot_candles_use_the_route_and_preserve_opaque_cursors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/spot/candles/HYPE-USDC"))
        .and(header("x-api-key", "test-key"))
        .and(query_param("start", "1742640622000"))
        .and(query_param("end", "1742644222000"))
        .and(query_param("limit", "1000"))
        .and(query_param("interval", "1m"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [{
                "timestamp": "2025-03-22T10:50:22Z",
                "open": 0.25,
                "high": "0.3",
                "low": 0.2,
                "close": "0.26",
                "volume": 12,
                "quote_volume": "3.12",
                "trade_count": 4
            }],
            "meta": {
                "count": 1,
                "request_id": "spot-candles-1",
                "next_cursor": "opaque-cursor:page/2"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/spot/candles/HYPE-USDC"))
        .and(header("x-api-key", "test-key"))
        .and(query_param("start", "1742640622000"))
        .and(query_param("end", "1742644222000"))
        .and(query_param("cursor", "opaque-cursor:page/2"))
        .and(query_param("interval", "1m"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [],
            "meta": {
                "count": 0,
                "request_id": "spot-candles-2",
                "next_cursor": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let first = client
        .hyperliquid
        .spot
        .candles
        .history(
            "HYPE-USDC",
            CandleHistoryParams {
                start: Timestamp::from("2025-03-22T10:50:22Z"),
                end: Timestamp::from("2025-03-22T11:50:22Z"),
                cursor: None,
                limit: Some(1000),
                interval: Some(CandleInterval::OneMinute),
            },
        )
        .await
        .unwrap();

    assert_eq!(first.data.len(), 1);
    assert_eq!(first.data[0].close, "0.26");
    assert_eq!(first.data[0].quote_volume.as_deref(), Some("3.12"));
    assert_eq!(first.data[0].trade_count, Some(4));
    assert_eq!(first.next_cursor.as_deref(), Some("opaque-cursor:page/2"));

    let second = client
        .hyperliquid
        .spot
        .candles
        .history(
            "HYPE-USDC",
            CandleHistoryParams {
                start: Timestamp::from("2025-03-22T10:50:22Z"),
                end: Timestamp::from("2025-03-22T11:50:22Z"),
                cursor: first.next_cursor,
                limit: None,
                interval: Some(CandleInterval::OneMinute),
            },
        )
        .await
        .unwrap();
    assert!(second.data.is_empty());
    assert!(second.next_cursor.is_none());
}

#[tokio::test]
async fn spot_candles_reject_limits_above_the_route_maximum() {
    let client = OxArchive::builder("test-key")
        .base_url("http://127.0.0.1:9")
        .build()
        .unwrap();

    let error = client
        .hyperliquid
        .spot
        .candles
        .history(
            "HYPE-USDC",
            CandleHistoryParams {
                start: Timestamp::from("2025-03-22T10:50:22Z"),
                end: Timestamp::from("2025-03-22T11:50:22Z"),
                cursor: None,
                limit: Some(1001),
                interval: Some(CandleInterval::OneDay),
            },
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("1 and 1000"));
}

#[tokio::test]
async fn lighter_candles_remain_available_from_august_2025() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/candles/BTC"))
        .and(header("x-api-key", "test-key"))
        .and(query_param("start", "1754006400000"))
        .and(query_param("end", "1754010000000"))
        .and(query_param("limit", "1000"))
        .and(query_param("interval", "1h"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [],
            "meta": {
                "count": 0,
                "request_id": "lighter-candles",
                "next_cursor": null
            }
        })))
        .mount(&server)
        .await;

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let result = client
        .lighter
        .candles
        .history(
            "BTC",
            CandleHistoryParams {
                start: Timestamp::from("2025-08-01T00:00:00Z"),
                end: Timestamp::from("2025-08-01T01:00:00Z"),
                cursor: None,
                limit: Some(1000),
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await
        .unwrap();

    assert!(result.data.is_empty());
}

#[tokio::test]
async fn lighter_l3_rejects_more_than_250_orders_per_side() {
    let client = OxArchive::builder("test-key")
        .base_url("http://127.0.0.1:9")
        .build()
        .unwrap();

    let error = client
        .lighter
        .l3_orderbook
        .get("BTC", Some(251))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("1 and 250 orders per side"));
}

#[tokio::test]
async fn hip4_open_interest_uses_the_family_model_for_history_and_current() {
    let server = MockServer::start().await;
    let record = serde_json::json!({
        "coin": "#0",
        "symbol": "#0",
        "outcome_id": 0,
        "side": 0,
        "timestamp": "2026-05-02T08:00:00Z",
        "open_interest": "10",
        "mark_price": "0.42",
        "mid_price": "0.42"
    });
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/openinterest/%230"))
        .and(query_param("start", "1777708800000"))
        .and(query_param("end", "1777712400000"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [record.clone()],
            "meta": {"count": 1, "request_id": "history-oi", "next_cursor": "next-oi"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/openinterest/%230/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": record
        })))
        .mount(&server)
        .await;

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let history = client
        .hyperliquid
        .hip4
        .get_open_interest(
            "#0",
            Hip4HistoryRange {
                start: Timestamp::from("2026-05-02T08:00:00Z"),
                end: Timestamp::from("2026-05-02T09:00:00Z"),
                cursor: None,
                limit: Some(100),
            },
        )
        .await
        .unwrap();
    let current = client
        .hyperliquid
        .hip4
        .get_open_interest_current("#0")
        .await
        .unwrap();

    assert_eq!(history.next_cursor.as_deref(), Some("next-oi"));
    for record in history.data.into_iter().chain(std::iter::once(current)) {
        assert_eq!(record.symbol.as_deref(), Some("#0"));
        assert_eq!(record.outcome_id, Some(0));
        assert_eq!(record.side, Some(0));
        assert_eq!(record.mark_price.as_deref(), Some("0.42"));
        assert!(!record.extra.contains_key("oracle_price"));
    }
}

#[test]
fn hip4_open_interest_preserves_identity_fields_without_oracle_field() {
    let record: Hip4OpenInterestRecord = serde_json::from_value(serde_json::json!({
        "coin": "#0",
        "symbol": "#0",
        "outcome_id": 0,
        "side": 0,
        "timestamp": "2026-05-02T00:00:00Z",
        "open_interest": "10",
        "mark_price": "0.42",
        "mid_price": "0.42"
    }))
    .unwrap();

    assert_eq!(record.symbol.as_deref(), Some("#0"));
    assert_eq!(record.outcome_id, Some(0));
    assert_eq!(record.side, Some(0));
    assert_eq!(record.mark_price.as_deref(), Some("0.42"));
    assert!(!record.extra.contains_key("oracle_price"));
}

#[test]
fn public_copy_keeps_family_specific_coverage() {
    let readme = include_str!("../README.md");
    let source = include_str!("../src/exchanges.rs");
    let hip4_types = include_str!("../src/types.rs");
    let candles = include_str!("../src/resources/candles.rs");
    let l3_resource = include_str!("../src/resources/l3_orderbook.rs");
    let reconstructor = include_str!("../src/orderbook_reconstructor.rs");
    let spot_example = include_str!("../examples/spot.rs");
    let websocket_example = include_str!("../examples/websocket.rs");

    assert!(readme.contains("client.hyperliquid.hip4.candles.history"));
    assert!(readme.contains("oxarchive = \"1.9\""));
    assert!(!readme.contains("oxarchive = \"1.8\""));
    assert!(readme.contains("2026-05-02"));
    assert!(readme.contains("~10s"));
    assert!(readme.contains("250 orders per side"));
    assert!(readme.contains("March 5, 2026"));
    assert!(!readme.contains("raw ~1 minute"));
    assert!(!readme.contains("no funding, no liquidations, and no candles"));
    assert!(readme.contains("exact starts vary by market"));
    assert!(readme.contains("live HIP-4 order-book and OI bridges are paused"));
    assert!(readme.contains("candles from exactly 2025-03-22T10:50:22Z"));
    assert!(readme.contains("Spot candle history starts exactly at"));
    assert!(readme.contains("`1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`"));
    assert!(readme.contains("client.hyperliquid.spot.candles.history"));
    assert!(readme.contains("| `candles` | Yes | Yes | Yes | Yes |"));
    assert!(spot_example.contains(".spot\n        .candles"));
    assert!(spot_example.contains("2025-03-22T10:50:22Z"));
    assert!(readme.contains("Candles from 2025-08-01"));
    assert!(!readme.contains("SDK passes `symbol` straight through to the URL path"));
    assert!(readme.contains("percent-encodes `#` only for URL wire transport"));
    assert!(readme.contains(
        "| `hip4_orderbook` | HIP-4 outcome-market L2 order book | No | Stored replay only |"
    ));
    assert!(readme.contains(
        "| `hip4_open_interest` | HIP-4 open interest snapshots | No | Stored replay only |"
    ));
    assert!(!readme.contains(
        "| `hip4_orderbook` | HIP-4 outcome-market L2 order book | Yes | Stored replay only"
    ));
    assert!(candles.contains("HIP-4 candles accept at most 1,000 rows per page"));
    assert!(!candles.contains("max 10,000 for candles"));
    let lighter_section_start = readme.find("### Lighter Orderbook Granularity").unwrap();
    let trades_section_start = readme.find("### Trades").unwrap();
    let lighter_section = &readme[lighter_section_start..trades_section_start];
    assert!(!lighter_section.contains("1704067200000"));
    assert!(!lighter_section.contains("2024-"));
    let hip4_section_start = readme.find("### HIP-4 Outcome Markets").unwrap();
    let l3_section_start = readme.find("### L3 Orderbook").unwrap();
    let l3_section = &readme[l3_section_start..hip4_section_start];
    assert!(!l3_section.contains("1704067200000"));
    assert!(!l3_section.contains("2024-"));
    let spot_section_start = readme.find("### Hyperliquid Spot").unwrap();
    let hip4_section = &readme[hip4_section_start..spot_section_start];
    assert!(!hip4_section.contains("1714694400000"));
    assert!(!hip4_section.contains("2024-"));
    assert!(!reconstructor.contains("1704067200000_i64"));
    assert!(!websocket_example.contains("subscribe(\"hip4_orderbook\""));
    let hip4_oi_start = hip4_types
        .find("pub struct Hip4OpenInterestRecord")
        .unwrap();
    let hip4_oi_end = hip4_types[hip4_oi_start..].find("\n}\n").unwrap() + hip4_oi_start;
    let hip4_oi_type = &hip4_types[hip4_oi_start..hip4_oi_end];
    assert!(!hip4_oi_type.contains("pub oracle_price: Option<String>"));
    assert!(l3_resource.contains("1 and 250 orders per side"));
    assert!(source.contains("pub candles: CandlesResource"));
}
