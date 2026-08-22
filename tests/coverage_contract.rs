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
                limit: None,
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.next_cursor.as_deref(), Some("next-cursor"));
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

#[test]
fn hip4_open_interest_has_no_oracle_field() {
    let record: Hip4OpenInterestRecord = serde_json::from_value(serde_json::json!({
        "coin": "#0",
        "timestamp": "2026-05-02T00:00:00Z",
        "open_interest": "10",
        "mark_price": "0.42",
        "mid_price": "0.42"
    }))
    .unwrap();

    assert_eq!(record.mark_price.as_deref(), Some("0.42"));
    assert!(!record.extra.contains_key("oracle_price"));
}

#[test]
fn public_copy_keeps_family_specific_coverage() {
    let readme = include_str!("../README.md");
    let source = include_str!("../src/exchanges.rs");
    let hip4_types = include_str!("../src/types.rs");
    let l3_resource = include_str!("../src/resources/l3_orderbook.rs");

    assert!(readme.contains("client.hyperliquid.hip4.candles.history"));
    assert!(readme.contains("2026-05-02"));
    assert!(readme.contains("~10s"));
    assert!(readme.contains("250 orders per side"));
    assert!(readme.contains("March 5, 2026"));
    assert!(!readme.contains("raw ~1 minute"));
    assert!(!readme.contains("no funding, no liquidations, and no candles"));
    assert!(readme.contains("exact starts vary by market"));
    assert!(readme.contains("live HIP-4 order-book and OI bridges are paused"));
    let hip4_oi_start = hip4_types
        .find("pub struct Hip4OpenInterestRecord")
        .unwrap();
    let hip4_oi_end = hip4_types[hip4_oi_start..].find("\n}\n").unwrap() + hip4_oi_start;
    let hip4_oi_type = &hip4_types[hip4_oi_start..hip4_oi_end];
    assert!(!hip4_oi_type.contains("pub oracle_price: Option<String>"));
    assert!(l3_resource.contains("1 and 250 orders per side"));
    assert!(source.contains("pub candles: CandlesResource"));
}
