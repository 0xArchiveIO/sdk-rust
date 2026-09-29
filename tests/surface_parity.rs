//! Request and response contracts for CVD, the HIP-3 oracle, HIP-4
//! questions, wallet classification, the webhook management routes and the
//! webhook signature verifier.

use hmac::{Hmac, Mac};
use oxarchive::exchanges::{Hip4L4HistoryParams, Hip4TradesParams};
use oxarchive::resources::breadth::BreadthHistoryParams;
use oxarchive::resources::data_quality::{ListIncidentsParams, SymbolCoverageParams};
use oxarchive::resources::l2_orderbook::L2HistoryParams;
use oxarchive::resources::l3_orderbook::{L3HistoryParams, L3OrderBookParams};
use oxarchive::resources::l4_orderbook::L4HistoryParams;
use oxarchive::resources::liquidations::{LiquidationHistoryParams, LiquidationVolumeParams};
use oxarchive::resources::orders::SpotOrderHistoryParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::types::{
    BreadthSnapshot, CandleInterval, OiFundingInterval, WebhookSubscriptionCondition,
    WebhookSubscriptionConfig, WebhookVenueFilter,
};
use oxarchive::{
    CreateEndpointParams, CreateSubscriptionParams, CvdParams, DryRunParams, Error, EstimateParams,
    Hip4ListQuestionsParams, OxArchive, SignatureError, UpdateSubscriptionParams,
    WalletClassifyParams, WebhookVerifier,
};
use serde_json::{json, Value};
use sha2::Sha256;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
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

fn body(value: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(value)
}

fn meta(count: usize) -> Value {
    json!({"count": count, "request_id": "req-1"})
}

// ---------------------------------------------------------------------------
// Public symbol universe
// ---------------------------------------------------------------------------

#[tokio::test]
async fn symbols_parse_the_unwrapped_response() {
    let server = MockServer::start().await;
    // This route answers `{"symbols": [...]}`, with no `success` or `data`.
    Mock::given(method("GET"))
        .and(path("/v1/symbols"))
        .respond_with(body(json!({"symbols": [
            {
                "symbol": "BTC", "exchange": "hyperliquid",
                "coverage_from": "2023-04-15T00:00:00Z",
                "data_types": ["orderbook", "trades"],
                "coverage_by_type": {"orderbook": "2023-04-15T00:00:00Z"},
                "size_per_day": {"orderbook": 812.5}
            },
            {
                "symbol": "#0", "exchange": "hip4",
                "coverage_from": "2026-05-02T00:00:00Z", "coverage_to": "2026-05-03T06:00:05Z",
                "data_types": ["trades"], "coverage_by_type": {},
                "slug": "btc-above-78213-yes-may-03-0600", "outcome_pair": ["#0", "#1"],
                "display_title": "BTC above 78,213 on May 3 at 06:00 UTC? Yes",
                "is_settled": true
            }
        ]})))
        .expect(1)
        .mount(&server)
        .await;

    let symbols = client(&server).symbols().await.unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].exchange, "hyperliquid");
    assert_eq!(symbols[0].size_per_day["orderbook"], 812.5);
    assert_eq!(symbols[0].coverage_to, None);
    assert_eq!(
        symbols[1].outcome_pair.as_deref(),
        Some(&["#0".to_string(), "#1".to_string()][..])
    );
    assert_eq!(symbols[1].is_settled, Some(true));
    assert!(symbols[1].size_per_day.is_empty());
}

// ---------------------------------------------------------------------------
// Cumulative volume delta
// ---------------------------------------------------------------------------

fn cvd_row(timestamp: i64, buy: f64, sell: f64, cumulative: f64) -> Value {
    json!({
        "timestamp": timestamp,
        "buy_volume": buy,
        "sell_volume": sell,
        "delta": buy - sell,
        "cumulative_delta": cumulative
    })
}

#[tokio::test]
async fn cvd_sends_its_window_and_returns_the_cursor_and_notice() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/cvd/BTC"))
        .and(header("X-API-Key", "test-key"))
        .and(query_param("start", "1790000000000"))
        .and(query_param("end", "1790086400000"))
        .and(query_param("interval", "5m"))
        .and(query_param("cursor", "1790000300000"))
        .and(query_param("limit", "2"))
        .respond_with(ok(
            json!([
                cvd_row(1790000600000, 10.5, 4.0, 6.5),
                cvd_row(1790000900000, 1.0, 3.0, 4.5)
            ]),
            json!({
                "count": 2,
                "request_id": "req-1",
                "next_cursor": "1790000900000",
                "notice": "cumulative_delta runs from the first bucket of this page; to join pages, rebuild it from delta"
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .hyperliquid
        .cvd(
            "BTC",
            CvdParams {
                start: Some(1790000000000_i64.into()),
                end: Some(1790086400000_i64.into()),
                interval: Some(CandleInterval::FiveMinutes),
                cursor: Some("1790000300000".to_string()),
                limit: Some(2),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data.len(), 2);
    assert_eq!(page.data[0].timestamp, 1790000600000);
    assert_eq!(page.data[0].delta, 6.5);
    assert_eq!(page.data[1].cumulative_delta, 4.5);
    assert_eq!(page.next_cursor.as_deref(), Some("1790000900000"));
    assert!(page
        .meta
        .notice
        .as_deref()
        .unwrap()
        .contains("rebuild it from delta"));
}

#[tokio::test]
async fn hip3_cvd_keeps_the_builder_prefix_and_sends_nothing_unset() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/cvd/xyz:XYZ100"))
        .and(query_param_is_missing("start"))
        .and(query_param_is_missing("end"))
        .and(query_param_is_missing("interval"))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("limit"))
        .respond_with(ok(json!([cvd_row(1790643600000, 5.0, 6.0, -1.0)]), meta(1)))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .hyperliquid
        .hip3
        .cvd("xyz:XYZ100", CvdParams::default())
        .await
        .unwrap();
    assert_eq!(page.data[0].sell_volume, 6.0);
    assert_eq!(page.next_cursor, None);
    assert_eq!(page.meta.notice, None);
}

// ---------------------------------------------------------------------------
// Hyperliquid core breadth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn core_breadth_uses_the_hyperliquid_prefix() {
    let server = MockServer::start().await;
    let snapshot = json!({
        "session_date": "2026-09-29",
        "calculated_at": "2026-09-29T03:03:00Z",
        "value_pct": 13.5593,
        "coverage_ratio": 0.7564102564102564,
        "counts": {
            "candidates": 234, "eligible": 177, "above": 24, "at": 0, "below": 153,
            "excluded_no_session_volume": 56, "excluded_stale_price": 1
        },
        "namespaces": {"eligible": {}, "above": {}, "at": {}, "below": {}}
    });
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/breadth/above-vwap/current"))
        .respond_with(ok(snapshot.clone(), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/breadth/above-vwap"))
        .and(query_param("interval", "1h"))
        .and(query_param("limit", "24"))
        .respond_with(ok(
            json!([snapshot]),
            json!({"count": 1, "request_id": "req-1", "next_cursor": "1790650980000"}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let current: BreadthSnapshot = client.hyperliquid.breadth.current().await.unwrap();
    assert_eq!(current.value_pct, Some(13.5593));
    assert_eq!(current.counts.eligible, 177);
    assert!(current.namespaces.eligible.is_empty());

    let history = client
        .hyperliquid
        .breadth
        .history(BreadthHistoryParams {
            interval: Some(OiFundingInterval::OneHour),
            limit: Some(24),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(history.data.len(), 1);
    assert_eq!(history.next_cursor.as_deref(), Some("1790650980000"));
}

// ---------------------------------------------------------------------------
// HIP-3 oracle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hip3_oracle_reads_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/hyperliquid/hip3/oracle/external-price/xyz:XYZ100",
        ))
        .respond_with(ok(
            json!({
                "symbol": "xyz:XYZ100", "external_price": 30198.0, "mark_price": 30203.0,
                "block_number": 1164898902_i64, "timestamp": 1790649697779_i64
            }),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/oracle/external-price/km:US500"))
        .respond_with(ok(
            json!({
                "symbol": "km:US500", "external_price": null, "mark_price": 749.64,
                "block_number": 1, "timestamp": 2
            }),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/hyperliquid/hip3/oracle/discovery-bounds/xyz:XYZ100",
        ))
        .respond_with(ok(
            json!({
                "symbol": "xyz:XYZ100", "reference_price": 30198.0,
                "reference_source": "external", "max_leverage": 30,
                "bound_fraction": 0.03333333333333333, "lower_bound": 29191.4,
                "upper_bound": 31204.600000000002, "block_number": 1164898902_i64,
                "timestamp": 1790649697779_i64
            }),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let price = client
        .hyperliquid
        .hip3
        .oracle
        .external_price("xyz:XYZ100")
        .await
        .unwrap();
    assert_eq!(price.external_price, Some(30198.0));
    assert_eq!(price.mark_price, Some(30203.0));
    assert_eq!(price.block_number, 1164898902);

    let no_external = client
        .hyperliquid
        .hip3
        .oracle
        .external_price("km:US500")
        .await
        .unwrap();
    assert_eq!(no_external.external_price, None);

    let bounds = client
        .hyperliquid
        .hip3
        .oracle
        .discovery_bounds("xyz:XYZ100")
        .await
        .unwrap();
    assert_eq!(bounds.reference_source, "external");
    assert_eq!(bounds.max_leverage, 30);
    assert!(bounds.lower_bound < bounds.reference_price);
    assert!(bounds.upper_bound > bounds.reference_price);
}

// ---------------------------------------------------------------------------
// HIP-4 questions
// ---------------------------------------------------------------------------

fn question(id: i64) -> Value {
    json!({
        "question_id": id,
        "name": "Recurring",
        "description": "class:priceBucket|underlying:BTC|expiry:20260509-0600|priceThresholds:77991,81174|period:1d",
        "fallback_outcome_id": 11,
        "named_outcome_ids": [12, 13, 14],
        "settled_named_outcomes": [12],
        "first_seen_at": "2026-05-09T05:57:28.335Z",
        "last_updated_at": "2026-05-09T05:57:28.335Z"
    })
}

#[tokio::test]
async fn hip4_questions_page_and_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/questions"))
        .and(query_param("cursor", "0"))
        .and(query_param("limit", "1"))
        .respond_with(ok(
            json!([question(1)]),
            json!({"count": 1, "request_id": "req-1", "next_cursor": "1"}),
        ))
        .expect(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/questions"))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("limit"))
        .respond_with(ok(json!([question(0), question(1)]), meta(2)))
        .expect(1)
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/questions/1"))
        .respond_with(ok(question(1), meta(1)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let all = client.hyperliquid.hip4.list_questions(None).await.unwrap();
    assert_eq!(all.data.len(), 2);
    assert_eq!(all.next_cursor, None);

    let page = client
        .hyperliquid
        .hip4
        .list_questions(Some(Hip4ListQuestionsParams {
            cursor: Some("0".to_string()),
            limit: Some(1),
        }))
        .await
        .unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("1"));
    assert_eq!(page.data[0].named_outcome_ids, vec![12, 13, 14]);

    let one = client.hyperliquid.hip4.get_question(1).await.unwrap();
    assert_eq!(one.question_id, 1);
    assert_eq!(one.fallback_outcome_id, 11);
    assert_eq!(one.settled_named_outcomes, vec![12]);
    assert!(one.description.starts_with("class:priceBucket"));
}

// ---------------------------------------------------------------------------
// Wallet classification
// ---------------------------------------------------------------------------

fn classified(address: &str) -> Value {
    json!({
        "address": address,
        "metrics": {
            "total_orders": 99693611_i64,
            "cancel_rate": 0.4983595688995557,
            "fill_rate": 0.0016403358084802445,
            "maker_ratio": 1.0,
            "uses_twap": false,
            "total_fees_usd": -1855.2356810011863,
            "active_hours": 24,
            "a_metric_added_later": 7
        },
        "period": "24h"
    })
}

#[tokio::test]
async fn wallet_classify_sends_every_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/wallets/classify"))
        .and(query_param("min_orders", "500"))
        .and(query_param("min_volume_usd", "100000"))
        .and(query_param("sort", "total_volume_usd"))
        .and(query_param("order", "asc"))
        .and(query_param("limit", "2"))
        .and(query_param("offset", "100"))
        .and(query_param("uses_twap", "true"))
        .and(query_param("uses_priority_gas", "false"))
        .and(query_param("min_cancel_rate", "0.1"))
        .and(query_param("max_cancel_rate", "0.9"))
        .and(query_param("date", "2026-09-28"))
        .respond_with(ok(
            json!({"wallets": [classified("0x4e60e3a4a32d63d245aa4e4ce304b4254b088f95")], "total": 6434, "date": "2026-09-28"}),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let result = client(&server)
        .hyperliquid
        .wallets
        .classify(WalletClassifyParams {
            min_orders: Some(500),
            min_volume_usd: Some(100_000.0),
            sort: Some("total_volume_usd".to_string()),
            order: Some("asc".to_string()),
            limit: Some(2),
            offset: Some(100),
            uses_twap: Some(true),
            uses_priority_gas: Some(false),
            min_cancel_rate: Some(0.1),
            max_cancel_rate: Some(0.9),
            date: chrono::NaiveDate::from_ymd_opt(2026, 9, 28),
        })
        .await
        .unwrap();
    assert_eq!(result.total, 6434);
    assert_eq!(result.date, "2026-09-28");
    let wallet = &result.wallets[0];
    assert_eq!(wallet.period, "24h");
    assert_eq!(wallet.metrics.total_orders, Some(99693611));
    assert_eq!(wallet.metrics.uses_twap, Some(false));
    assert_eq!(wallet.metrics.active_hours, Some(24));
    // Sent without a value, so absent rather than a default.
    assert_eq!(wallet.metrics.top_builder, None);
    assert_eq!(
        wallet.metrics.extra.get("a_metric_added_later"),
        Some(&json!(7))
    );
}

#[tokio::test]
async fn hip3_wallet_classify_uses_its_own_prefix() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip3/wallets/classify"))
        .and(query_param_is_missing("min_orders"))
        .and(query_param_is_missing("date"))
        .respond_with(ok(
            json!({"wallets": [], "total": 0, "date": "2026-09-28"}),
            meta(0),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let result = client(&server)
        .hyperliquid
        .hip3
        .wallets
        .classify(WalletClassifyParams::default())
        .await
        .unwrap();
    assert!(result.wallets.is_empty());
    assert_eq!(result.total, 0);
}

// ---------------------------------------------------------------------------
// Webhooks: catalog and limits
// ---------------------------------------------------------------------------

const ENDPOINT_ID: &str = "b9807e65-8952-4d32-a877-a45c8b54c5aa";
const SUBSCRIPTION_ID: &str = "f5064079-0163-41c6-b606-b04d17e25a04";
const DELIVERY_ID: &str = "11111111-2222-4333-8444-555555555555";
const EVENT_ID: &str = "66666666-7777-4888-8999-000000000000";
const ADDRESS_ID: &str = "cebc5b17-a907-4f0a-bbf4-4de2d8c399a9";

#[tokio::test]
async fn webhook_event_types_decode_the_catalog() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/event-types"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(body(json!({"success": true, "data": [
            {
                "type": "market.liquidation", "schema_version": 1, "live": true,
                "scope": "public", "venues": ["hyperliquid", "hip3"],
                "filters": ["venue", "symbols"],
                "params": {"max_age_s": {"type": "integer", "unit": "s", "default": 3600, "min": 60, "max": 86400}},
                "metrics": {
                    "notional_usd": {"type": "number", "unit": "USD"},
                    "side": {"type": "enum", "values": ["A", "B"]}
                },
                "cost_floor": {"metric": "notional_usd", "min": 100},
                "latency_class": "seconds",
                "description": "A liquidation fill.",
                "filters_example": {"venue": "hyperliquid", "min_notional_usd": 100000},
                "operators": {"any": ["is_empty", "is_not_empty"], "number": ["greater_than"]}
            },
            {
                "type": "market.pga_payment", "schema_version": 1, "live": true,
                "scope": "public", "venues": [], "filters": [],
                "params": {"window_s": {"type": "integer", "enum": [60, 300]}},
                "metrics": {},
                "cost_floor": {"metric": "amount", "min": 0.5, "unit": "HYPE", "note": "scan floor"},
                "latency_class": "minutes", "description": "A priority gas payment."
            }
        ]})))
        .expect(1)
        .mount(&server)
        .await;

    let catalog = client(&server).webhooks.event_types().await.unwrap();
    assert_eq!(catalog.len(), 2);
    let liq = &catalog[0];
    assert_eq!(liq.event_type, "market.liquidation");
    assert_eq!(liq.params["max_age_s"].default, Some(json!(3600)));
    assert_eq!(liq.params["max_age_s"].max, Some(86400.0));
    assert_eq!(
        liq.metrics["side"].values.as_deref(),
        Some(&["A".to_string(), "B".to_string()][..])
    );
    assert_eq!(liq.cost_floor.as_ref().unwrap().min, 100.0);
    assert_eq!(liq.operators["any"], vec!["is_empty", "is_not_empty"]);
    let pga = &catalog[1];
    assert_eq!(
        pga.params["window_s"].allowed,
        Some(vec![json!(60), json!(300)])
    );
    assert_eq!(
        pga.cost_floor.as_ref().unwrap().unit.as_deref(),
        Some("HYPE")
    );
    assert!(pga.operators.is_empty());
}

#[tokio::test]
async fn webhook_limits_decode_paid_and_free_plans() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/limits"))
        .respond_with(body(json!({"success": true, "data": {
            "plan": "free", "plan_label": "Free", "included": false, "preview_included": true,
            "endpoints": {"used": 0, "limit": 0, "remaining": 0},
            "subscriptions": {"used": 0, "limit": 0, "remaining": 0},
            "watched_addresses": {"used": 0, "limit": 0, "remaining": 0},
            "deliveries_per_day": {"used": 0, "limit": 0, "remaining": 0, "unlimited": false, "resets_at": "2026-09-30T00:00:00Z"},
            "paused_subscriptions": {"count": 2, "earliest_paused_at": "2026-09-29T01:00:00Z", "reasons": ["plan_no_webhooks"], "message": "Two rules are paused."},
            "notice": "Webhook delivery starts on the Build plan."
        }})))
        .expect(1)
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/limits"))
        .respond_with(body(json!({"success": true, "data": {
            "plan": "enterprise", "plan_label": "Enterprise", "included": true, "preview_included": true,
            "endpoints": {"used": 2, "limit": 100, "remaining": 98},
            "subscriptions": {"used": 28, "limit": 2000, "remaining": 1972},
            "watched_addresses": {"used": 3, "limit": 250, "remaining": 247},
            "deliveries_per_day": {"used": 0, "limit": null, "remaining": null, "unlimited": true, "resets_at": "2026-09-30T00:00:00Z"},
            "paused_subscriptions": {"count": 0, "earliest_paused_at": null, "reasons": []}
        }})))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client(&server);
    let free = client.webhooks.limits().await.unwrap();
    assert!(!free.included);
    assert!(free.preview_included);
    assert_eq!(free.endpoints.limit, 0);
    assert_eq!(free.deliveries_per_day.limit, Some(0));
    assert_eq!(free.paused_subscriptions.count, 2);
    assert_eq!(free.paused_subscriptions.reasons, vec!["plan_no_webhooks"]);
    assert!(free.notice.is_some());

    let paid = client.webhooks.limits().await.unwrap();
    assert!(paid.included);
    assert_eq!(paid.subscriptions.remaining, 1972);
    assert!(paid.deliveries_per_day.unlimited);
    assert_eq!(paid.deliveries_per_day.limit, None);
    assert_eq!(paid.paused_subscriptions.earliest_paused_at, None);
    assert_eq!(paid.notice, None);
}

// ---------------------------------------------------------------------------
// Webhooks: endpoints and deliveries
// ---------------------------------------------------------------------------

fn endpoint_json() -> Value {
    json!({
        "id": ENDPOINT_ID, "url": "https://example.com/hooks/0xarchive",
        "description": "receiver", "status": "active", "consecutive_failures": 0,
        "created_at": "2026-09-21T02:02:04.460276Z"
    })
}

#[tokio::test]
async fn webhook_endpoint_routes() {
    let server = MockServer::start().await;
    let mut listed = endpoint_json();
    listed["format"] = json!("json");
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/endpoints"))
        .respond_with(body(json!({"success": true, "data": [listed]})))
        .expect(1)
        .mount(&server)
        .await;
    let mut created = endpoint_json();
    created["secret"] =
        json!("whsec_0000000000000000000000000000000000000000000000000000000000000000");
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/endpoints"))
        .and(body_json(
            json!({"url": "https://example.com/hooks/0xarchive", "description": "receiver"}),
        ))
        .respond_with(body(json!({
            "success": true, "data": created,
            "note": "Store the secret now; it is not shown again."
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}")))
        .respond_with(body(json!({"success": true})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}/enable")))
        .respond_with(body(json!({"success": true})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}/rotate")))
        .respond_with(body(json!({
            "success": true,
            "data": {"secret": "whsec_1111111111111111111111111111111111111111111111111111111111111111"},
            "note": "Previous secret remains valid for 24 hours."
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}/test")))
        .respond_with(body(json!({
            "success": true,
            "data": {"delivery_id": DELIVERY_ID, "event_id": EVENT_ID}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let endpoints = client.webhooks.list_endpoints().await.unwrap();
    assert_eq!(endpoints[0].id, ENDPOINT_ID);
    assert_eq!(endpoints[0].status, "active");
    // A field this release does not name is kept rather than dropped.
    assert_eq!(endpoints[0].extra.get("format"), Some(&json!("json")));

    let created = client
        .webhooks
        .create_endpoint(
            CreateEndpointParams::new("https://example.com/hooks/0xarchive")
                .description("receiver"),
        )
        .await
        .unwrap();
    assert!(created.secret.starts_with("whsec_"));
    assert_eq!(created.id, ENDPOINT_ID);
    assert_eq!(
        created.note.as_deref(),
        Some("Store the secret now; it is not shown again.")
    );

    client.webhooks.delete_endpoint(ENDPOINT_ID).await.unwrap();
    client.webhooks.enable_endpoint(ENDPOINT_ID).await.unwrap();

    let rotated = client.webhooks.rotate_secret(ENDPOINT_ID).await.unwrap();
    assert!(rotated.secret.starts_with("whsec_1111"));
    assert_eq!(
        rotated.note.as_deref(),
        Some("Previous secret remains valid for 24 hours.")
    );

    let queued = client.webhooks.test_endpoint(ENDPOINT_ID).await.unwrap();
    assert_eq!(queued.delivery_id, DELIVERY_ID);
    assert_eq!(queued.event_id, EVENT_ID);
}

#[tokio::test]
async fn webhook_refusals_surface_as_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}/test")))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "code": 409,
            "error": "Today's delivery budget is already spent.",
            "request_id": "req-409"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/endpoints"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "code": 400,
            "error": "Webhook delivery starts on the Build plan.",
            "request_id": "req-400"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    match client
        .webhooks
        .test_endpoint(ENDPOINT_ID)
        .await
        .unwrap_err()
    {
        Error::Api {
            code,
            message,
            request_id,
        } => {
            assert_eq!(code, 409);
            assert!(message.contains("budget"));
            assert_eq!(request_id.as_deref(), Some("req-409"));
        }
        other => panic!("expected an API error, got {other:?}"),
    }
    match client
        .webhooks
        .create_endpoint(CreateEndpointParams::new("https://example.com/hooks"))
        .await
        .unwrap_err()
    {
        Error::Api { code, .. } => assert_eq!(code, 400),
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn webhook_delivery_routes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/webhooks/endpoints/{ENDPOINT_ID}/deliveries")))
        .and(query_param("limit", "10"))
        .respond_with(body(json!({"success": true, "data": [{
            "id": DELIVERY_ID, "event_id": EVENT_ID, "event_type": "webhook.test",
            "state": "failed", "attempts": 3, "last_status_code": 500,
            "last_error": "HTTP 500", "last_latency_ms": 42,
            "next_attempt_at": "2026-09-29T01:10:00Z", "delivered_at": null,
            "created_at": "2026-09-29T01:00:00Z",
            "payload": {"id": EVENT_ID, "type": "webhook.test", "schema_version": 1, "observed_at": "2026-09-29T01:00:00Z", "data": {}}
        }]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/webhooks/endpoints/{ENDPOINT_ID}/deliveries"
        )))
        .and(query_param_is_missing("limit"))
        .respond_with(body(json!({"success": true, "data": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/webhooks/deliveries/{DELIVERY_ID}/redeliver"
        )))
        .respond_with(body(json!({
            "success": true,
            "data": {
                "delivery_id": DELIVERY_ID, "event_id": EVENT_ID, "event_type": "webhook.test",
                "state": "pending", "attempts": 0, "next_attempt_at": "2026-09-29T02:00:00Z"
            },
            "note": "The same delivery record is re-queued."
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let log = client
        .webhooks
        .list_deliveries(ENDPOINT_ID, Some(10))
        .await
        .unwrap();
    assert_eq!(log[0].state, "failed");
    assert_eq!(log[0].last_status_code, Some(500));
    assert_eq!(log[0].delivered_at, None);
    assert_eq!(log[0].payload["type"], "webhook.test");
    assert!(client
        .webhooks
        .list_deliveries(ENDPOINT_ID, None)
        .await
        .unwrap()
        .is_empty());

    let again = client.webhooks.redeliver(DELIVERY_ID).await.unwrap();
    assert_eq!(again.delivery_id, DELIVERY_ID);
    assert_eq!(again.event_id, EVENT_ID);
    assert_eq!(again.state, "pending");
    assert_eq!(again.attempts, 0);
    assert_eq!(
        again.note.as_deref(),
        Some("The same delivery record is re-queued.")
    );
}

// ---------------------------------------------------------------------------
// Webhooks: subscriptions
// ---------------------------------------------------------------------------

fn subscription_json(status: &str) -> Value {
    let paused = status == "auto_paused";
    json!({
        "id": SUBSCRIPTION_ID, "endpoint_id": ENDPOINT_ID, "event_type": "account.fill",
        "filters": {
            "addresses": ["0xcf3f419d08a5bdc2c6e5fbd9ad70904c5420f95f"],
            "conditions": [{"metric": "notional_usd", "op": "greater_than_or_equal", "value": 250000}],
            "min_notional_usd": 250000.0,
            "params": {"max_age_s": 3600}
        },
        "enabled": true, "created_at": "2026-09-21T02:04:52.698775Z",
        "status": status,
        "pause_message": if paused { json!("Paused at the daily delivery limit.") } else { Value::Null },
        "paused_at": if paused { json!("2026-09-29T01:00:00Z") } else { Value::Null },
        "pause_reason": if paused { json!("deliveries_per_day_cap") } else { Value::Null },
        "suppressed_count": if paused { 12 } else { 0 },
        "suppressed_first_at": null, "suppressed_last_at": null,
        "last_paused_at": null, "last_resumed_at": null, "last_pause_reason": null,
        "last_suppressed_count": 0,
        "last_suppressed_first_at": null, "last_suppressed_last_at": null
    })
}

#[tokio::test]
async fn webhook_subscription_crud() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/subscriptions"))
        .respond_with(body(
            json!({"success": true, "data": [subscription_json("auto_paused")]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions"))
        .and(body_json(json!({
            "endpoint_id": ENDPOINT_ID,
            "event_type": "account.fill",
            "filters": {
                "addresses": ["0xcf3f419d08a5bdc2c6e5fbd9ad70904c5420f95f"],
                "params": {"max_age_s": 3600},
                "conditions": [{"metric": "notional_usd", "op": ">=", "value": 250000}]
            }
        })))
        .respond_with(body(
            json!({"success": true, "data": subscription_json("active")}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!(
            "/v1/webhooks/subscriptions/{SUBSCRIPTION_ID}"
        )))
        .and(body_json(json!({"enabled": false})))
        .respond_with(body(
            json!({"success": true, "data": subscription_json("active")}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!(
            "/v1/webhooks/subscriptions/{SUBSCRIPTION_ID}"
        )))
        .and(body_json(json!({"filters": {"venue": "hyperliquid"}})))
        .respond_with(body(
            json!({"success": true, "data": subscription_json("active")}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/v1/webhooks/subscriptions/{SUBSCRIPTION_ID}"
        )))
        .respond_with(body(json!({"success": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let subs = client.webhooks.list_subscriptions().await.unwrap();
    let paused = &subs[0];
    assert!(paused.is_paused());
    assert_eq!(
        paused.pause_reason.as_deref(),
        Some("deliveries_per_day_cap")
    );
    assert_eq!(paused.suppressed_count, 12);
    assert_eq!(paused.filters.min_notional_usd, Some(250000.0));
    assert_eq!(paused.filters.params["max_age_s"], json!(3600));
    assert_eq!(paused.filters.conditions[0].op, "greater_than_or_equal");

    let created = client
        .webhooks
        .create_subscription(
            CreateSubscriptionParams::new(ENDPOINT_ID, "account.fill").filters(
                WebhookSubscriptionConfig::default()
                    .addresses(["0xcf3f419d08a5bdc2c6e5fbd9ad70904c5420f95f"])
                    .param("max_age_s", 3600)
                    .condition(WebhookSubscriptionCondition::new(
                        "notional_usd",
                        ">=",
                        250_000,
                    )),
            ),
        )
        .await
        .unwrap();
    assert!(!created.is_paused());
    assert_eq!(created.status, "active");

    client
        .webhooks
        .update_subscription(
            SUBSCRIPTION_ID,
            UpdateSubscriptionParams::default().enabled(false),
        )
        .await
        .unwrap();
    client
        .webhooks
        .update_subscription(
            SUBSCRIPTION_ID,
            UpdateSubscriptionParams::default()
                .filters(WebhookSubscriptionConfig::default().venue("hyperliquid")),
        )
        .await
        .unwrap();
    client
        .webhooks
        .delete_subscription(SUBSCRIPTION_ID)
        .await
        .unwrap();
}

#[tokio::test]
async fn webhook_resume_returns_the_gap() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/webhooks/subscriptions/{SUBSCRIPTION_ID}/resume")))
        .respond_with(body(json!({
            "success": true,
            "data": subscription_json("active"),
            "gap": {
                "paused_at": "2026-09-29T01:00:00Z", "resumed_at": "2026-09-29T03:00:00Z",
                "replay_window": {"start": "2026-09-29T01:00:00Z", "end": "2026-09-29T03:00:00Z"},
                "reason": "deliveries_per_day_cap", "pause_message": "Paused at the daily delivery limit.",
                "suppressed_count": 12, "counted": true,
                "suppressed_first_at": "2026-09-29T01:05:00Z", "suppressed_last_at": "2026-09-29T02:55:00Z",
                "note": "Re-read the window from the REST routes."
            }
        })))
        .expect(1)
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/webhooks/subscriptions/{SUBSCRIPTION_ID}/resume"
        )))
        .respond_with(body(json!({
            "success": true,
            "data": subscription_json("active"),
            "gap": null,
            "note": "This subscription was already active, so nothing changed."
        })))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions/resume"))
        .respond_with(body(json!({
            "success": true,
            "data": [subscription_json("active"), subscription_json("active")],
            "resumed_count": 2,
            "gap": {
                "paused_at": "2026-09-29T01:00:00Z", "resumed_at": "2026-09-29T03:00:00Z",
                "replay_window": {"start": "2026-09-29T01:00:00Z", "end": "2026-09-29T03:00:00Z"},
                "reason": null, "reasons": ["deliveries_per_day_cap", "plan_no_webhooks"],
                "suppressed_count": null, "counted": false, "uncounted_subscriptions": 2,
                "note": "Address scoped rules were not counted."
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let resumed = client
        .webhooks
        .resume_subscription(SUBSCRIPTION_ID)
        .await
        .unwrap();
    let gap = resumed
        .gap
        .expect("a resume of a paused rule returns the gap");
    assert_eq!(
        gap.replay_window.start.as_deref(),
        Some("2026-09-29T01:00:00Z")
    );
    assert_eq!(gap.suppressed_count, Some(12));
    assert!(gap.counted);
    assert_eq!(resumed.note, None);
    assert_eq!(resumed.subscription.id, SUBSCRIPTION_ID);

    let unchanged = client
        .webhooks
        .resume_subscription(SUBSCRIPTION_ID)
        .await
        .unwrap();
    assert!(unchanged.gap.is_none());
    assert!(unchanged.note.unwrap().contains("nothing changed"));

    let all = client.webhooks.resume_all_subscriptions().await.unwrap();
    assert_eq!(all.resumed_count, 2);
    assert_eq!(all.subscriptions.len(), 2);
    let gap = all.gap.unwrap();
    assert_eq!(gap.reason, None);
    assert_eq!(gap.reasons.unwrap().len(), 2);
    assert_eq!(gap.suppressed_count, None);
    assert!(!gap.counted);
    assert_eq!(gap.uncounted_subscriptions, Some(2));
}

#[tokio::test]
async fn webhook_resume_all_with_nothing_paused() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions/resume"))
        .respond_with(body(json!({
            "success": true, "data": [], "resumed_count": 0, "gap": null,
            "note": "Nothing on this account is paused, so nothing changed."
        })))
        .expect(1)
        .mount(&server)
        .await;
    let all = client(&server)
        .webhooks
        .resume_all_subscriptions()
        .await
        .unwrap();
    assert_eq!(all.resumed_count, 0);
    assert!(all.subscriptions.is_empty());
    assert!(all.gap.is_none());
    assert!(all.note.is_some());
}

// ---------------------------------------------------------------------------
// Webhooks: previews
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_previews_send_config_and_decode() {
    let server = MockServer::start().await;
    let occurrence = json!({
        "observed_at_estimate": "2026-09-08T13:41:01.586Z",
        "data": {"venue": "hyperliquid", "symbol": "BTC", "notional_usd": 562419.19}
    });
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions/dry-run"))
        .and(body_json(json!({
            "event_type": "market.liquidation",
            "config": {
                "venue": "hyperliquid",
                "symbols": ["BTC", "ETH"],
                "conditions": [{"metric": "notional_usd", "op": ">=", "value": 250000}]
            },
            "lookback_s": 21600,
            "limit": 5
        })))
        .respond_with(body(json!({"success": true, "data": {
            "event_type": "market.liquidation",
            "window": {"from": "2026-09-08T08:00:00.000Z", "to": "2026-09-08T14:00:00.000Z"},
            "matched": 12, "truncated": true, "occurrences": [occurrence.clone()]
        }})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions/estimate"))
        .and(body_json(json!({"event_type": "market.liquidation", "config": {}, "lookback_days": 7})))
        .respond_with(body(json!({"success": true, "data": {
            "event_type": "market.liquidation",
            "window": {"from": "2026-09-01T14:00:00Z", "to": "2026-09-08T14:00:00Z"},
            "days": 7, "total": 210,
            "per_day": [{"date": "2026-09-08", "count": 30}],
            "per_day_p50": 29.0, "per_day_max": 44,
            "primary_metric": "notional_usd",
            "ladder": [{"value": 100000.0, "per_day": 30.0}, {"value": 250000.0, "per_day": 8.5}],
            "distribution": {"n": 210, "p50": 180000.0, "p90": 700000.0, "p99": 2500000.0, "max": 9000000.0},
            "sample": [occurrence],
            "basis": {"mode": "exact", "note": null}
        }})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/subscriptions/estimate"))
        .and(body_json(
            json!({"event_type": "hip4.settlement", "config": {}}),
        ))
        .respond_with(body(json!({"success": true, "data": {
            "event_type": "hip4.settlement",
            "window": {"from": "2026-09-01T14:00:00Z", "to": "2026-09-08T14:00:00Z"},
            "days": 7, "total": 0, "per_day": [], "per_day_p50": 0.0, "per_day_max": 0,
            "primary_metric": null, "ladder": [], "distribution": null, "sample": [],
            "basis": {"mode": "replayed", "note": "Re-run over history."}
        }})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let dry = client
        .webhooks
        .dry_run(
            DryRunParams::new("market.liquidation")
                .config(
                    WebhookSubscriptionConfig::default()
                        .venue("hyperliquid")
                        .symbols(["BTC", "ETH"])
                        .condition(WebhookSubscriptionCondition::new(
                            "notional_usd",
                            ">=",
                            250_000,
                        )),
                )
                .lookback_s(21600)
                .limit(5),
        )
        .await
        .unwrap();
    assert_eq!(dry.matched, 12);
    assert!(dry.truncated);
    assert_eq!(dry.occurrences[0].data["symbol"], "BTC");
    assert_eq!(dry.window.from, "2026-09-08T08:00:00.000Z");

    let est = client
        .webhooks
        .estimate(EstimateParams::new("market.liquidation").lookback_days(7))
        .await
        .unwrap();
    assert_eq!(est.total, 210);
    assert_eq!(est.ladder[1].per_day, 8.5);
    assert_eq!(est.distribution.unwrap().p90, 700000.0);
    assert_eq!(est.basis.mode, "exact");
    assert_eq!(est.per_day[0].count, 30);

    let none = client
        .webhooks
        .estimate(EstimateParams::new("hip4.settlement"))
        .await
        .unwrap();
    assert!(none.distribution.is_none());
    assert!(none.primary_metric.is_none());
    assert_eq!(none.basis.note.as_deref(), Some("Re-run over history."));
}

// ---------------------------------------------------------------------------
// Webhooks: watched wallets
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_watched_address_routes() {
    let server = MockServer::start().await;
    let row = json!({
        "id": ADDRESS_ID, "address": "0xcf3f419d08a5bdc2c6e5fbd9ad70904c5420f95f",
        "label": "whale-A", "created_at": "2026-09-21T02:04:12.377291Z"
    });
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/addresses"))
        .respond_with(body(
            json!({"success": true, "data": [row.clone()], "limit": 250}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/addresses"))
        .and(body_json(
            json!({"address": "0xCF3F419D08A5BDC2C6E5FBD9AD70904C5420F95F", "label": "whale-A"}),
        ))
        .respond_with(body(
            json!({"success": true, "data": row.clone(), "limit": 250}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/addresses"))
        .and(body_json(
            json!({"address": "0x0000000000000000000000000000000000000001"}),
        ))
        .respond_with(body(json!({"success": true, "data": {
            "id": ADDRESS_ID, "address": "0x0000000000000000000000000000000000000001",
            "label": "", "created_at": "2026-09-21T02:04:12Z"
        }, "limit": 2})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/webhooks/addresses/{ADDRESS_ID}")))
        .respond_with(body(json!({"success": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let list = client.webhooks.list_addresses().await.unwrap();
    assert_eq!(list.limit, Some(250));
    assert_eq!(list.addresses[0].label, "whale-A");

    let added = client
        .webhooks
        .add_address(
            "0xCF3F419D08A5BDC2C6E5FBD9AD70904C5420F95F",
            Some("whale-A"),
        )
        .await
        .unwrap();
    assert_eq!(
        added.address.address,
        "0xcf3f419d08a5bdc2c6e5fbd9ad70904c5420f95f"
    );
    assert_eq!(added.limit, Some(250));

    let unlabeled = client
        .webhooks
        .add_address("0x0000000000000000000000000000000000000001", None)
        .await
        .unwrap();
    assert_eq!(unlabeled.address.label, "");
    assert_eq!(unlabeled.limit, Some(2));

    client.webhooks.delete_address(ADDRESS_ID).await.unwrap();
}

#[test]
fn a_subscription_config_accepts_one_or_many_venues() {
    let one: WebhookSubscriptionConfig =
        serde_json::from_value(json!({"venue": "hyperliquid|hip3"})).unwrap();
    assert_eq!(
        one.venue,
        Some(WebhookVenueFilter::One("hyperliquid|hip3".into()))
    );
    let many: WebhookSubscriptionConfig =
        serde_json::from_value(json!({"venue": ["hyperliquid", "hip3"]})).unwrap();
    assert_eq!(
        many.venue,
        Some(WebhookVenueFilter::Many(vec![
            "hyperliquid".into(),
            "hip3".into()
        ]))
    );
}

// ---------------------------------------------------------------------------
// Webhook signature verification
// ---------------------------------------------------------------------------

const SECRET: &str = "whsec_3333333333333333333333333333333333333333333333333333333333333333";
const PREVIOUS: &str = "whsec_4444444444444444444444444444444444444444444444444444444444444444";
const DELIVERY: &[u8] = br#"{"id": "66666666-7777-4888-8999-000000000000", "data": {"message": "Test event from 0xArchive."}, "type": "webhook.test", "observed_at": "2026-09-29T00:00:00+00:00", "schema_version": 1}"#;

/// Sign the way deliveries are signed: hex HMAC-SHA256 over `<t>.<body>`,
/// keyed with the whole secret string.
fn sign(secret: &str, t: i64, raw_body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{t}.").as_bytes());
    mac.update(raw_body);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn a_fresh_delivery_verifies_against_the_system_clock() {
    let t = now();
    let header = format!("t={t},v1={}", sign(SECRET, t, DELIVERY));
    let parsed = WebhookVerifier::new(SECRET)
        .verify(DELIVERY, &header)
        .unwrap();
    assert_eq!(parsed.timestamp, t);
    assert_eq!(parsed.signatures.len(), 1);
}

#[test]
fn a_tampered_body_or_a_wrong_secret_is_refused() {
    let t = now();
    let header = format!("t={t},v1={}", sign(SECRET, t, DELIVERY));
    let tampered = String::from_utf8(DELIVERY.to_vec())
        .unwrap()
        .replace("Test event", "Test evenT");
    assert_eq!(
        WebhookVerifier::new(SECRET)
            .verify(tampered.as_bytes(), &header)
            .unwrap_err(),
        SignatureError::Mismatch
    );
    assert_eq!(
        WebhookVerifier::new(PREVIOUS)
            .verify(DELIVERY, &header)
            .unwrap_err(),
        SignatureError::Mismatch
    );
    // Moving `t` breaks the signature too, because `t` is signed.
    let moved = format!("t={},v1={}", t - 1, sign(SECRET, t, DELIVERY));
    assert_eq!(
        WebhookVerifier::new(SECRET)
            .verify(DELIVERY, &moved)
            .unwrap_err(),
        SignatureError::Mismatch
    );
}

#[test]
fn during_a_rotation_every_v1_is_tried() {
    let t = now();
    // New secret first, previous second, as sent during the overlap.
    let header = format!(
        "t={t},v1={},v1={}",
        sign(SECRET, t, DELIVERY),
        sign(PREVIOUS, t, DELIVERY)
    );
    // A receiver that has not rolled yet holds only the previous secret,
    // which signed the second value: reading only the first would fail here.
    WebhookVerifier::new(PREVIOUS)
        .verify(DELIVERY, &header)
        .expect("the previous secret keeps verifying during the overlap");
    WebhookVerifier::new(SECRET)
        .verify(DELIVERY, &header)
        .expect("the new secret verifies");
    let both = WebhookVerifier::with_secrets([SECRET, PREVIOUS])
        .verify(DELIVERY, &header)
        .unwrap();
    assert_eq!(both.signatures.len(), 2);
}

#[test]
fn an_expired_delivery_is_refused_even_with_a_valid_signature() {
    let t = now() - 3600;
    let header = format!("t={t},v1={}", sign(SECRET, t, DELIVERY));
    match WebhookVerifier::new(SECRET).verify(DELIVERY, &header) {
        Err(SignatureError::Stale {
            skew_secs,
            tolerance_secs,
        }) => {
            assert!(skew_secs >= 3600);
            assert_eq!(tolerance_secs, oxarchive::DEFAULT_TOLERANCE_SECS);
        }
        other => panic!("expected a stale refusal, got {other:?}"),
    }
    // A wider window accepts the same delivery.
    WebhookVerifier::new(SECRET)
        .tolerance_secs(7200)
        .verify(DELIVERY, &header)
        .unwrap();
}

// ---------------------------------------------------------------------------
// Routes and parameters reconciled with the live API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spot_order_history_sends_only_the_range_and_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/spot/orders/HYPE-USDC/history"))
        .and(query_param("start", "1790553600000"))
        .and(query_param("end", "1790557200000"))
        .and(query_param("cursor", "abc"))
        .and(query_param("limit", "20"))
        .and(query_param_is_missing("user"))
        .and(query_param_is_missing("status"))
        .and(query_param_is_missing("order_type"))
        .respond_with(ok(json!([]), meta(0)))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .hyperliquid
        .spot
        .orders
        .history(
            "HYPE-USDC",
            SpotOrderHistoryParams {
                start: Some(1790553600000_i64.into()),
                end: Some(1790557200000_i64.into()),
                cursor: Some("abc".to_string()),
                limit: Some(20),
            },
        )
        .await
        .unwrap();
    assert!(page.data.is_empty());
}

#[tokio::test]
async fn hip3_liquidations_keep_history_volume_and_levels() {
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/hip3/liquidations/km:US500",
        "/v1/hyperliquid/hip3/liquidations/km:US500/volume",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ok(json!([]), meta(0)))
            .expect(1)
            .mount(&server)
            .await;
    }
    let client = client(&server);
    client
        .hyperliquid
        .hip3
        .liquidations
        .history(
            "km:US500",
            LiquidationHistoryParams {
                start: 1790553600000_i64.into(),
                end: 1790557200000_i64.into(),
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();
    client
        .hyperliquid
        .hip3
        .liquidations
        .volume(
            "km:US500",
            LiquidationVolumeParams {
                start: 1790553600000_i64.into(),
                end: 1790557200000_i64.into(),
                interval: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn trades_and_history_reads_send_no_ignored_parameters() {
    let server = MockServer::start().await;
    for route in [
        "/v1/hyperliquid/trades/BTC",
        "/v1/hyperliquid/hip4/trades/0",
        "/v1/hyperliquid/orderbook/BTC/l2/history",
        "/v1/hyperliquid/orderbook/BTC/l4/history",
        "/v1/hyperliquid/hip4/orderbook/0/l4/history",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .and(query_param("cursor", "opaque:cursor/1"))
            .and(query_param_is_missing("side"))
            .and(query_param_is_missing("depth"))
            .respond_with(ok(json!([]), meta(0)))
            .expect(1)
            .mount(&server)
            .await;
    }
    let client = client(&server);
    let cursor = || Some("opaque:cursor/1".to_string());
    client
        .hyperliquid
        .trades
        .list(
            "BTC",
            GetTradesParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: cursor(),
                limit: None,
            },
        )
        .await
        .unwrap();
    client
        .hyperliquid
        .hip4
        .get_trades(
            "0",
            Hip4TradesParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: cursor(),
                limit: None,
            },
        )
        .await
        .unwrap();
    client
        .hyperliquid
        .l2_orderbook
        .history(
            "BTC",
            L2HistoryParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: cursor(),
                limit: None,
            },
        )
        .await
        .unwrap();
    client
        .hyperliquid
        .l4_orderbook
        .history(
            "BTC",
            L4HistoryParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: cursor(),
                limit: None,
            },
        )
        .await
        .unwrap();
    client
        .hyperliquid
        .hip4
        .get_l4_history(
            "0",
            Hip4L4HistoryParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: cursor(),
                limit: None,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn l3_reads_send_account_and_timestamp() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/l3orderbook/BTC"))
        .and(query_param("timestamp", "1790553600000"))
        .and(query_param("account", "726714"))
        .and(query_param("depth", "50"))
        .respond_with(ok(json!({"coin": "BTC", "orders": []}), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/lighter/l3orderbook/BTC/history"))
        .and(query_param("account", "726714"))
        .respond_with(ok(json!([]), meta(0)))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);
    client
        .lighter
        .l3_orderbook
        .get_with_params(
            "BTC",
            L3OrderBookParams {
                timestamp: Some(1790553600000_i64.into()),
                account: Some(726714),
                depth: Some(50),
            },
        )
        .await
        .unwrap();
    client
        .lighter
        .l3_orderbook
        .history(
            "BTC",
            L3HistoryParams {
                start: 1_i64.into(),
                end: 2_i64.into(),
                cursor: None,
                limit: None,
                account: Some(726714),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        client
            .lighter
            .l3_orderbook
            .get_with_params(
                "BTC",
                L3OrderBookParams {
                    depth: Some(251),
                    ..Default::default()
                }
            )
            .await
            .unwrap_err(),
        Error::InvalidParam(_)
    ));
}

#[tokio::test]
async fn data_quality_encodes_symbols_and_sends_filters() {
    let server = MockServer::start().await;
    let coverage = |symbol: &str| json!({"exchange": "x", "symbol": symbol, "data_types": {}});
    for (route, symbol) in [
        ("/v1/data-quality/coverage/hip3/km%3AUS500", "km:US500"),
        ("/v1/data-quality/coverage/spot/HYPE-USDC", "HYPE-USDC"),
        ("/v1/data-quality/coverage/hip4/%230", "#0"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ok(coverage(symbol), meta(1)))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/data-quality/coverage/hyperliquid/BTC"))
        .and(query_param("from", "1788000000000"))
        .and(query_param("to", "1788086400000"))
        .respond_with(ok(coverage("BTC"), meta(1)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/data-quality/incidents"))
        .and(query_param("status", "resolved"))
        .and(query_param("exchange", "lighter"))
        .and(query_param("since", "1788000000000"))
        .and(query_param("limit", "2"))
        .and(query_param("offset", "4"))
        .respond_with(ok(
            json!({"incidents": [], "pagination": {"total": 5, "limit": 2, "offset": 4}}),
            meta(0),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let hip3 = client
        .data_quality
        .symbol_coverage("hip3", "km:US500")
        .await
        .unwrap();
    assert_eq!(hip3.symbol, "km:US500");
    client
        .data_quality
        .symbol_coverage("spot", "HYPE-USDC")
        .await
        .unwrap();
    client
        .data_quality
        .symbol_coverage("hip4", "#0")
        .await
        .unwrap();
    client
        .data_quality
        .symbol_coverage_with(
            "hyperliquid",
            "BTC",
            SymbolCoverageParams {
                from: Some(1788000000000_i64.into()),
                to: Some(1788086400000_i64.into()),
            },
        )
        .await
        .unwrap();
    let incidents = client
        .data_quality
        .list_incidents_with(ListIncidentsParams {
            status: Some("resolved".to_string()),
            exchange: Some("lighter".to_string()),
            since: Some(1788000000000_i64.into()),
            limit: Some(2),
            offset: Some(4),
        })
        .await
        .unwrap();
    assert_eq!(incidents.pagination.unwrap().total, 5);
}

#[tokio::test]
async fn spot_and_hip4_freshness_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/spot/freshness/HYPE-USDC"))
        .respond_with(ok(
            json!({
                "coin": "HYPE-USDC", "symbol": "HYPE-USDC", "exchange": "spot",
                "measured_at": "2026-09-29T03:15:52.789409117Z",
                "l4_checkpoints": {"lag_ms": 83444, "last_updated": "2026-09-29T03:14:29.345Z"},
                "l4_diffs": {"lag_ms": 2168, "last_updated": "2026-09-29T03:15:50.621Z"},
                "orderbook": {"lag_ms": 3619, "last_updated": "2026-09-29T03:15:49.170Z"},
                "orders": {"lag_ms": 1638, "last_updated": "2026-09-29T03:15:51.151Z"},
                "trades": {"lag_ms": 2641, "last_updated": "2026-09-29T03:15:50.148Z"},
                "twap": {"lag_ms": 88779, "last_updated": "2026-09-29T03:14:24.010Z"}
            }),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;
    // HIP-4 has no funding; its entries can be empty objects.
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/hip4/freshness/0"))
        .respond_with(ok(
            json!({
                "coin": "#0", "symbol": "#0", "exchange": "hip4",
                "measured_at": "2026-09-29T03:15:52.894702314Z",
                "orderbook": {}, "trades": {}, "open_interest": {}
            }),
            meta(1),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let spot = client
        .hyperliquid
        .spot
        .freshness("HYPE-USDC")
        .await
        .unwrap();
    assert_eq!(spot.data_types.len(), 6);
    assert_eq!(spot.data_types["twap"].lag_ms, Some(88779));
    let hip4 = client.hyperliquid.hip4.get_freshness("0").await.unwrap();
    assert!(!hip4.data_types.contains_key("funding"));
    assert_eq!(hip4.data_types["trades"].last_updated, None);
}
