//! Times that describe market data can be null when the time of the data is
//! unknown: `TriggerLevels::as_of` and `as_of_block`, the summary's
//! `timestamp`, `L2OrderBookSnapshot::timestamp`, and the `timestamp` of an
//! `l4_snapshot` frame. Each accepts a value, `null` and a missing field, so
//! responses with or without these fields parse.

use oxarchive::{L2OrderBookSnapshot, OxArchive, TriggerLevels, TriggerLevelsParams};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> OxArchive {
    OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap()
}

fn ok(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(json!({"success": true, "data": data, "meta": {"count": 1, "request_id": "r"}}))
}

fn without(mut record: Value, keys: &[&str]) -> Value {
    for key in keys {
        record.as_object_mut().unwrap().remove(*key);
    }
    record
}

fn with(mut record: Value, key: &str, value: Value) -> Value {
    record.as_object_mut().unwrap().insert(key.to_string(), value);
    record
}

/// The three shapes every field is checked in: a value, `null`, missing.
fn shapes(record: &Value, key: &str) -> [(&'static str, Value); 3] {
    [
        ("value", record.clone()),
        ("null", with(record.clone(), key, Value::Null)),
        ("missing", without(record.clone(), &[key])),
    ]
}

// ---------------------------------------------------------------------------
// Trigger levels
// ---------------------------------------------------------------------------

fn trigger_levels() -> Value {
    json!({
        "mid_price": 98123.5,
        "as_of": "2026-11-13T22:21:05.695Z",
        "as_of_block": 1220113000_u64,
        "total_bid_size": 1.5,
        "total_ask_size": 4.0,
        "levels": [
            {"price_bucket": 97000.0, "bid_count": 1, "bid_size": 1.5, "ask_count": 0, "ask_size": 0.0},
            {"price_bucket": 99000.0, "bid_count": 0, "bid_size": 0.0, "ask_count": 2, "ask_size": 4.0}
        ]
    })
}

#[test]
fn trigger_levels_as_of_and_block_accept_value_null_and_missing() {
    let parse = |v: Value| serde_json::from_value::<TriggerLevels>(v).expect("trigger levels must parse");

    let full = parse(trigger_levels());
    assert_eq!(full.as_of.as_deref(), Some("2026-11-13T22:21:05.695Z"));
    assert_eq!(full.as_of_block, Some(1220113000));
    assert_eq!(full.levels.len(), 2);

    let nulls = parse(with(
        with(trigger_levels(), "as_of", Value::Null),
        "as_of_block",
        Value::Null,
    ));
    assert_eq!(nulls.as_of, None);
    assert_eq!(nulls.as_of_block, None);

    let missing = parse(without(trigger_levels(), &["as_of", "as_of_block"]));
    assert_eq!(missing.as_of, None);
    assert_eq!(missing.as_of_block, None);
    assert_eq!(missing.mid_price, 98123.5);

    // An answer that predates as_of_block: a whole-second as_of and no block.
    let older = parse(with(
        without(trigger_levels(), &["as_of_block"]),
        "as_of",
        json!("2026-11-13T22:21:05Z"),
    ));
    assert_eq!(older.as_of.as_deref(), Some("2026-11-13T22:21:05Z"));
    assert_eq!(older.as_of_block, None);
}

#[tokio::test]
async fn trigger_levels_with_null_times_come_back_from_both_products() {
    let body = with(
        with(trigger_levels(), "as_of", Value::Null),
        "as_of_block",
        Value::Null,
    );
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/orders/BTC/trigger-levels",
        "/v1/hyperliquid/hip3/orders/xyz:TSLA/trigger-levels",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ok(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
    }
    let c = client(&server);
    let core = c
        .hyperliquid
        .orders
        .trigger_levels("BTC", TriggerLevelsParams::default())
        .await
        .unwrap();
    let hip3 = c
        .hyperliquid
        .hip3
        .orders
        .trigger_levels("xyz:TSLA", TriggerLevelsParams::default())
        .await
        .unwrap();
    for levels in [core, hip3] {
        assert_eq!(levels.as_of, None);
        assert_eq!(levels.as_of_block, None);
        assert_eq!(levels.levels.len(), 2);
    }
}

// ---------------------------------------------------------------------------
// Market summary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn summary_timestamp_accepts_value_null_and_missing_on_every_venue() {
    let summary = json!({
        "coin": "BTC",
        "timestamp": "2026-11-13T22:21:00Z",
        "mark_price": "98123.5",
        "oracle_price": "98120.0",
        "open_interest": "31234.5",
        "funding_rate": "0.0000125"
    });
    for (name, body) in shapes(&summary, "timestamp") {
        let server = MockServer::start().await;
        for route in [
            "/v1/hyperliquid/summary/BTC",
            "/v1/hyperliquid/hip3/summary/xyz:TSLA",
            "/v1/hyperliquid/hip4/summary/0",
            "/v1/lighter/summary/BTC",
            "/v1/rh-lighter/summary/BTC",
        ] {
            Mock::given(method("GET"))
                .and(path(route))
                .respond_with(ok(body.clone()))
                .expect(1)
                .mount(&server)
                .await;
        }
        let c = client(&server);
        let all = [
            c.hyperliquid.summary("BTC").await.unwrap(),
            c.hyperliquid.hip3.summary("xyz:TSLA").await.unwrap(),
            c.hyperliquid.hip4.get_summary("0").await.unwrap(),
            c.lighter.summary("BTC").await.unwrap(),
            c.rh_lighter.summary("BTC").await.unwrap(),
        ];
        for s in all {
            assert_eq!(s.mark_price.as_deref(), Some("98123.5"), "{name}");
            let expected = match name {
                "value" => Some(json!("2026-11-13T22:21:00Z")),
                "null" => Some(Value::Null),
                _ => None,
            };
            assert_eq!(s.extra.get("timestamp").cloned(), expected, "{name}");
        }
    }
}

// ---------------------------------------------------------------------------
// Full-depth L2 snapshot (REST)
// ---------------------------------------------------------------------------

fn l2_snapshot() -> Value {
    json!({
        "coin": "BTC",
        "timestamp": "2026-11-13T22:21:05.695Z",
        "bid_levels": 1,
        "ask_levels": 1,
        "total_bid_size": 1.2,
        "total_ask_size": 0.4,
        "mid_price": 98123.5,
        "spread": 1.0,
        "spread_bps": 0.1,
        "bids": [{"px": 98123.0, "sz": 1.2, "n": 3}],
        "asks": [{"px": 98124.0, "sz": 0.4, "n": 1}]
    })
}

#[test]
fn l2_snapshot_timestamp_accepts_value_null_and_missing() {
    for (name, body) in shapes(&l2_snapshot(), "timestamp") {
        let snapshot: L2OrderBookSnapshot =
            serde_json::from_value(body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = (name == "value").then(|| "2026-11-13T22:21:05.695Z".to_string());
        assert_eq!(snapshot.timestamp, expected, "{name}");
        assert_eq!(snapshot.bids.len(), 1, "{name}");
    }
    // The format the current book used before it was RFC 3339 still parses.
    let legacy: L2OrderBookSnapshot =
        serde_json::from_value(with(l2_snapshot(), "timestamp", json!("2026-11-13 22:21:05.695")))
            .unwrap();
    assert_eq!(legacy.timestamp.as_deref(), Some("2026-11-13 22:21:05.695"));
}

#[tokio::test]
async fn l2_current_with_null_timestamp_comes_back_from_both_products() {
    let body = with(l2_snapshot(), "timestamp", Value::Null);
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/orderbook/BTC/l2",
        "/v1/hyperliquid/hip3/orderbook/xyz:TSLA/l2",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ok(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
    }
    let c = client(&server);
    let core = c.hyperliquid.l2_orderbook.get("BTC", None).await.unwrap();
    let hip3 = c.hyperliquid.hip3.l2_orderbook.get("xyz:TSLA", None).await.unwrap();
    assert_eq!(core.timestamp, None);
    assert_eq!(hip3.timestamp, None);
}

// ---------------------------------------------------------------------------
// WebSocket l4_snapshot
// ---------------------------------------------------------------------------

#[cfg(feature = "websocket")]
mod ws_frames {
    use super::{shapes, with};
    use oxarchive::ws::ServerMsg;
    use serde_json::json;

    const CHANNELS: [&str; 6] = [
        "l4_diffs",
        "hip3_l4_diffs",
        "spot_l4_diffs",
        "hip4_l4_diffs",
        "orderbook_full",
        "hip3_orderbook_full",
    ];

    #[test]
    fn l4_snapshot_timestamp_accepts_value_null_and_missing_on_every_channel() {
        for channel in CHANNELS {
            let frame = json!({
                "type": "l4_snapshot",
                "channel": channel,
                "coin": "BTC",
                "symbol": "BTC",
                "last_block_number": 1220113000_u64,
                "timestamp": 1794608465695_i64,
                "data": {"bids": [], "asks": []}
            });
            for (name, body) in shapes(&frame, "timestamp") {
                let text = serde_json::to_string(&body).unwrap();
                match serde_json::from_str::<ServerMsg>(&text)
                    .unwrap_or_else(|e| panic!("{channel} {name}: {e}"))
                {
                    ServerMsg::L4Snapshot {
                        channel: got,
                        last_block_number,
                        timestamp,
                        ..
                    } => {
                        assert_eq!(got, channel);
                        assert_eq!(last_block_number, 1220113000);
                        let expected = (name == "value").then_some(1794608465695_i64);
                        assert_eq!(timestamp, expected, "{channel} {name}");
                    }
                    other => panic!("{channel} {name}: wrong variant {other:?}"),
                }
            }
        }
    }

    #[test]
    fn l4_snapshot_still_refuses_a_non_integer_timestamp() {
        let frame = json!({
            "type": "l4_snapshot", "channel": "l4_diffs", "coin": "BTC", "symbol": "BTC",
            "last_block_number": 1_u64, "data": {}
        });
        let text = serde_json::to_string(&with(frame, "timestamp", json!("2026-11-13"))).unwrap();
        assert!(serde_json::from_str::<ServerMsg>(&text).is_err());
    }
}
