use oxarchive::exchanges::Hip4HistoryRange;
use oxarchive::resources::breadth::BreadthHistoryParams;
use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::types::{
    CandleInterval, Hip3BreadthSnapshot, Hip4OpenInterestRecord, OiFundingInterval, Timestamp,
};
use oxarchive::OxArchive;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn hip4_candles_use_the_bare_numeric_path_and_numeric_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/candles/0"))
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
                "next_cursor": "1777712400000"
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
            "0",
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
    assert_eq!(result.next_cursor.as_deref(), Some("1777712400000"));
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
            "0",
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
async fn hyperliquid_hip3_and_lighter_use_the_10000_row_candle_cap() {
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/candles/BTC",
        "/v1/hyperliquid/hip3/candles/BTC",
        "/v1/lighter/candles/BTC",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .and(query_param("limit", "10000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": [],
                "meta": {"count": 0, "request_id": "candle-cap", "next_cursor": null}
            })))
            .expect(1)
            .mount(&server)
            .await;
    }

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let params = |limit| CandleHistoryParams {
        start: Timestamp::from("2026-05-02T08:00:00Z"),
        end: Timestamp::from("2026-05-02T09:00:00Z"),
        cursor: None,
        limit: Some(limit),
        interval: Some(CandleInterval::OneHour),
    };

    for result in [
        client
            .hyperliquid
            .candles
            .history("BTC", params(10_000))
            .await,
        client
            .hyperliquid
            .hip3
            .candles
            .history("BTC", params(10_000))
            .await,
        client.lighter.candles.history("BTC", params(10_000)).await,
    ] {
        result.unwrap();
    }

    for result in [
        client
            .hyperliquid
            .candles
            .history("BTC", params(10_001))
            .await,
        client
            .hyperliquid
            .hip3
            .candles
            .history("BTC", params(10_001))
            .await,
        client.lighter.candles.history("BTC", params(10_001)).await,
    ] {
        let error = result.unwrap_err();
        assert!(error.to_string().contains("1 and 10000"));
    }
}

#[tokio::test]
async fn spot_candles_use_the_route_and_preserve_numeric_string_cursors() {
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
                "next_cursor": "1742644222000"
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
        .and(query_param("cursor", "1742644222000"))
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
    assert_eq!(first.next_cursor.as_deref(), Some("1742644222000"));

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
        .and(path("/v1/hyperliquid/hip4/openinterest/0"))
        .and(query_param("start", "1777708800000"))
        .and(query_param("end", "1777712400000"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [record.clone()],
            "meta": {"count": 1, "request_id": "history-oi", "next_cursor": "1777712400000"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/openinterest/0/current"))
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
            "0",
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
        .get_open_interest_current("0")
        .await
        .unwrap();

    assert_eq!(history.next_cursor.as_deref(), Some("1777712400000"));
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
    assert!(readme.contains("use the bare numeric form (`\"0\"`"));
    assert!(readme.contains("Legacy `\"#0\"`"));
    assert!(readme.contains(
        "| `hip4_orderbook` | HIP-4 outcome-market L2 order book | No | Yes |"
    ));
    assert!(readme.contains(
        "| `hip4_open_interest` | HIP-4 open interest snapshots | No | Yes |"
    ));
    assert!(!readme.contains(
        "| `hip4_orderbook` | HIP-4 outcome-market L2 order book | Yes | Stored replay only"
    ));
    assert!(candles.contains("Hyperliquid, HIP-3, and Lighter"));
    assert!(candles.contains("HIP-4 and Spot accept at most 1,000"));
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

#[tokio::test]
async fn hip3_breadth_current_is_typed_and_uses_the_exact_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/breadth/above-vwap/current"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": {
                "session_date": "2026-08-28",
                "calculated_at": "2026-08-28T20:54:00Z",
                "value_pct": 20.93,
                "coverage_ratio": 0.382,
                "counts": {
                    "candidates": 225,
                    "eligible": 86,
                    "above": 18,
                    "at": 0,
                    "below": 68,
                    "excluded_no_session_volume": 75,
                    "excluded_stale_price": 64
                },
                "namespaces": {
                    "eligible": {"xyz": 41},
                    "above": {"xyz": 9},
                    "at": {},
                    "below": {"xyz": 32}
                }
            },
            "meta": {"count": 1, "request_id": "breadth-current", "next_cursor": null}
        })))
        .mount(&server)
        .await;

    let client = OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let result: Hip3BreadthSnapshot = client
        .hyperliquid
        .hip3
        .breadth
        .current()
        .await
        .unwrap();

    assert_eq!(result.session_date, "2026-08-28");
    assert_eq!(result.value_pct, Some(20.93));
    assert_eq!(result.counts.eligible, 86);
    assert_eq!(result.namespaces.eligible["xyz"], 41);
}

#[tokio::test]
async fn hip3_breadth_history_sends_exact_params_and_preserves_null_and_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/breadth/above-vwap"))
        .and(header("x-api-key", "test-key"))
        .and(query_param("start", "1787961600000"))
        .and(query_param("end", "1788048000000"))
        .and(query_param("interval", "5m"))
        .and(query_param("limit", "1000"))
        .and(query_param("cursor", "1788036600000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true,
            "data": [{
                "session_date": "2026-08-28",
                "calculated_at": "2026-08-28T20:54:00Z",
                "value_pct": null,
                "coverage_ratio": 0.0,
                "counts": {
                    "candidates": 0,
                    "eligible": 0,
                    "above": 0,
                    "at": 0,
                    "below": 0,
                    "excluded_no_session_volume": 0,
                    "excluded_stale_price": 0
                },
                "namespaces": {"eligible": {}, "above": {}, "at": {}, "below": {}}
            }],
            "meta": {
                "count": 1,
                "request_id": "breadth-history",
                "next_cursor": "1788048000000"
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
        .hip3
        .breadth
        .history(BreadthHistoryParams {
            start: Some(1787961600000_i64.into()),
            end: Some(1788048000000_i64.into()),
            interval: Some(OiFundingInterval::FiveMinutes),
            cursor: Some("1788036600000".to_string()),
            limit: Some(1000),
        })
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0].value_pct, None);
    assert_eq!(result.next_cursor.as_deref(), Some("1788048000000"));
}

#[test]
fn current_copy_matches_breadth_cadence_and_funding_unit_contracts() {
    let readme = include_str!("../README.md");
    let normalized_readme = readme.split_whitespace().collect::<Vec<_>>().join(" ");
    let changelog = include_str!("../CHANGELOG.md");
    let normalized_changelog = changelog.split_whitespace().collect::<Vec<_>>().join(" ");
    let funding = include_str!("../src/resources/funding.rs");
    let liquidations = include_str!("../src/resources/liquidations.rs")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let types = include_str!("../src/types.rs")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(normalized_readme.contains("History begins on **2026-08-28**"));
    assert!(normalized_readme.contains("decimal fractions, not percentages and not annualized"));
    assert!(funding.contains("fractional"));
    assert!(funding.contains("non-annualized"));
    assert!(!normalized_changelog.contains("45-minute snapshots"));
    assert!(normalized_changelog.contains("approximately five-minute"));
    assert!(normalized_changelog.contains("fractional and non-annualized"));
    assert!(!liquidations.contains("45 minutes"));
    assert!(liquidations.contains("approximately every five minutes"));
    assert!(!types.contains("45 minutes"));
    assert!(types.contains("approximately every five minutes"));
}
