//! Trade intent, USDC flows, breadth above the n-day moving average and
//! positioning, against responses recorded from the API (version
//! 2026-10-01) in `tests/fixtures/derived/`.
//!
//! Each fixture holds the request that was sent (`path`, `query`) and the
//! response (`status`, `body`). The mocks answer a fixture's body only to
//! its exact request, and the tests check that the SDK sent exactly the
//! recorded query, nothing more.

use std::collections::BTreeMap;

use oxarchive::resources::breadth_ma::BreadthMaHistoryParams;
use oxarchive::resources::trade_intent::TradeIntentParams;
use oxarchive::resources::usdc_flows::{UsdcFlowVolumeParams, UsdcFlowsParams};
use oxarchive::types::OiFundingInterval;
use oxarchive::{
    BreadthMaLength, Capability, Error, ErrorCode, OxArchive, PositioningWindow,
    TradeIntentInterval, UsdcFlowKind,
};
use serde_json::Value;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const PLANS: [&str; 4] = ["build", "pro", "scale", "enterprise"];
const PRICING: &str = "https://0xarchive.io/pricing";

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

/// A recorded fixture: `{request: {path, query, ...}, status, body}`.
fn recorded(name: &str) -> Value {
    let file = format!(
        "{}/tests/fixtures/derived/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap()
}

fn reply(fixture: &Value) -> ResponseTemplate {
    let status = fixture["status"].as_u64().unwrap() as u16;
    ResponseTemplate::new(status).set_body_json(fixture["body"].clone())
}

/// A query value as it travels: strings as they are, numbers as written.
fn wire(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The recorded request's query.
fn recorded_query(name: &str) -> BTreeMap<String, String> {
    recorded(name)["request"]["query"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), wire(v)))
        .collect()
}

/// The query a request carried.
fn sent(request: &Request) -> BTreeMap<String, String> {
    request
        .url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Answer `name`'s body to exactly its recorded path and query, once.
async fn mount(server: &MockServer, name: &str) {
    let fixture = recorded(name);
    let query = fixture["request"]["query"].as_object().unwrap().clone();
    let mut mock =
        Mock::given(method("GET")).and(path(fixture["request"]["path"].as_str().unwrap()));
    for (k, v) in &query {
        mock = mock.and(query_param(k.as_str(), wire(v).as_str()));
    }
    if !query.contains_key("cursor") {
        mock = mock.and(query_param_is_missing("cursor"));
    }
    mock.respond_with(reply(&fixture))
        .expect(1)
        .mount(server)
        .await;
}

/// Answer `name`'s body to any GET on its recorded path.
async fn mount_on_path(server: &MockServer, name: &str) {
    let fixture = recorded(name);
    Mock::given(method("GET"))
        .and(path(fixture["request"]["path"].as_str().unwrap()))
        .respond_with(reply(&fixture))
        .mount(server)
        .await;
}

/// Assert the requests the server received carried exactly these fixtures'
/// queries, in order.
async fn assert_sent(server: &MockServer, names: &[&str]) {
    let requests = server.received_requests().await.unwrap();
    let got: Vec<_> = requests.iter().map(sent).collect();
    let want: Vec<_> = names.iter().map(|n| recorded_query(n)).collect();
    assert_eq!(got, want);
}

fn ms(t: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(t)
        .unwrap()
        .timestamp_millis()
}

// ---------------------------------------------------------------------------
// Trade intent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trade_intent_pages_follow_the_cursor_with_the_window_unchanged() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_btc_1h_page1").await;
    mount(&server, "trade_intent_btc_1h_page2").await;

    let params = TradeIntentParams {
        start: Some("2026-11-02T00:00:00Z".into()),
        end: Some("2026-11-02T05:00:00Z".into()),
        interval: Some(TradeIntentInterval::OneHour),
        limit: Some(3),
        ..Default::default()
    };
    let mut pages = client(&server)
        .hyperliquid
        .trade_intent
        .iterate("BTC", params)
        .unwrap();

    let first = pages.next_page().await.unwrap().unwrap();
    assert_eq!(first.data.len(), 3);
    assert!(first.has_more);
    assert_eq!(first.next_cursor.as_deref(), Some("1793588400000"));
    assert_eq!(first.meta.symbol.as_deref(), Some("BTC"));
    assert_eq!(first.meta.venue.as_deref(), Some("hyperliquid"));
    assert_eq!(
        first.meta.built_through.as_deref(),
        Some("2026-11-04T19:00:00.000Z")
    );

    let b = &first.data[0];
    assert_eq!(b.timestamp, "2026-11-02T00:00:00.000Z");
    assert_eq!(b.timestamp_ms, ms("2026-11-02T00:00:00Z"));
    assert_eq!(b.close, Some(78061.0));
    assert_eq!(b.price_source.as_deref(), Some("mark"));
    assert_eq!(b.open_long, 27144995.59);
    assert_eq!(b.flip_sl, 875275.68);
    assert_eq!(b.mm_usd, 66752421.84);
    assert_eq!(b.losers_flow, Some(-7838880.08));
    assert_eq!(b.winners_net, Some(199258865.1));
    assert_eq!((b.winners_count, b.losers_count), (Some(4), Some(10)));
    assert_eq!(b.quality, "ok");
    assert!(b.is_final && !b.preliminary);
    assert_eq!(
        (b.preliminary_from.as_deref(), b.preliminary_from_ms),
        (None, None)
    );
    assert_eq!(b.method, "who/3");

    let second = pages.next_page().await.unwrap().unwrap();
    assert_eq!(second.data.len(), 3);
    assert!(!second.has_more);
    assert_eq!(second.next_cursor, None);
    assert_eq!(second.data[0].timestamp, "2026-11-02T03:00:00.000Z");
    assert!(pages.next_page().await.unwrap().is_none());
    assert!(pages.next_page().await.unwrap().is_none());

    assert_sent(
        &server,
        &["trade_intent_btc_1h_page1", "trade_intent_btc_1h_page2"],
    )
    .await;
}

#[tokio::test]
async fn trade_intent_history_sends_the_cursor_it_is_given() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_btc_1h_page2").await;
    let page = client(&server)
        .hyperliquid
        .trade_intent
        .history(
            "BTC",
            TradeIntentParams {
                start: Some(ms("2026-11-02T00:00:00Z").into()),
                end: Some(ms("2026-11-02T05:00:00Z").into()),
                interval: Some(TradeIntentInterval::OneHour),
                cursor: Some("1793588400000".to_string()),
                limit: Some(3),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data.len(), 3);
    assert!(!page.has_more);
}

#[tokio::test]
async fn hip3_trade_intent_keeps_the_builder_prefix() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_hip3_1h").await;
    let page = client(&server)
        .hyperliquid
        .hip3
        .trade_intent
        .history(
            "xyz:XYZ100",
            TradeIntentParams {
                start: Some("2026-11-02T00:00:00Z".into()),
                end: Some("2026-11-02T02:00:00Z".into()),
                interval: Some(TradeIntentInterval::OneHour),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data.len(), 3);
    assert_eq!(page.meta.symbol.as_deref(), Some("xyz:XYZ100"));
    assert_eq!(page.meta.venue.as_deref(), Some("hip3"));
    assert_eq!(page.data[0].close, Some(28568.0));
    assert_eq!(page.data[0].winners_net, Some(-14454507.5));
}

#[tokio::test]
async fn second_buckets_carry_no_close_or_cohort_position() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_btc_1s").await;
    let q = recorded_query("trade_intent_btc_1s");
    let page = client(&server)
        .hyperliquid
        .trade_intent
        .history(
            "BTC",
            TradeIntentParams {
                start: Some(q["start"].parse::<i64>().unwrap().into()),
                end: Some(q["end"].parse::<i64>().unwrap().into()),
                interval: Some(TradeIntentInterval::OneSecond),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.meta.preliminary_row_count, Some(3));
    assert!(page.meta.built_through.is_some());
    let s = &page.data[0];
    assert_eq!((s.close, s.price_source.as_deref()), (None, None));
    assert_eq!(
        (s.winners_net, s.winners_gross, s.losers_net, s.losers_gross),
        (None, None, None, None)
    );
    assert_eq!((s.winners_count, s.losers_count), (None, None));
    assert_eq!((s.winners_flow, s.losers_flow), (Some(0.0), Some(0.02)));
    assert!(s.preliminary && !s.is_final);
    assert_eq!(s.preliminary_from_ms, Some(s.timestamp_ms));
    assert_eq!(s.preliminary_from.as_deref(), Some(s.timestamp.as_str()));
}

#[tokio::test]
async fn live_minutes_are_preliminary_and_paged() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_btc_1m_live").await;
    let q = recorded_query("trade_intent_btc_1m_live");
    let page = client(&server)
        .hyperliquid
        .trade_intent
        .history(
            "BTC",
            TradeIntentParams {
                start: Some(q["start"].parse::<i64>().unwrap().into()),
                end: Some(q["end"].parse::<i64>().unwrap().into()),
                interval: Some(TradeIntentInterval::OneMinute),
                limit: Some(3),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(page.has_more);
    assert_eq!(page.meta.preliminary_row_count, Some(3));
    assert!(page
        .data
        .iter()
        .all(|b| b.preliminary && !b.is_final && b.close.is_some()));
}

#[tokio::test]
async fn build_plan_reads_trade_intent() {
    let server = MockServer::start().await;
    mount(&server, "trade_intent_build_ok").await;
    let page = client(&server)
        .hyperliquid
        .trade_intent
        .history(
            "ETH",
            TradeIntentParams {
                start: Some("2026-11-02T00:00:00Z".into()),
                end: Some("2026-11-02T01:00:00Z".into()),
                interval: Some(TradeIntentInterval::OneHour),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data.len(), 2);
    assert_eq!(page.data[0].close, Some(2270.4));
}

// ---------------------------------------------------------------------------
// USDC flows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usdc_flow_events_page_by_cursor_and_join_on_id() {
    let server = MockServer::start().await;
    mount(&server, "usdc_flows_events_page1").await;
    mount(&server, "usdc_flows_events_page2").await;
    let flows = client(&server).hyperliquid.usdc_flows;
    let mut params = UsdcFlowsParams {
        start: Some("2026-11-02T00:00:00Z".into()),
        end: Some("2026-11-02T01:00:00Z".into()),
        limit: Some(3),
        ..Default::default()
    };
    let first = flows.history(params.clone()).await.unwrap();
    assert!(first.has_more);
    params.cursor = first.next_cursor.clone();
    let second = flows.history(params).await.unwrap();
    assert!(second.has_more);
    assert_eq!(
        second.next_cursor.as_deref(),
        Some("1793577601003_1205759962_1")
    );

    let ids: Vec<&str> = first
        .data
        .iter()
        .chain(&second.data)
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "1205759951:16",
            "1205759951:17",
            "1205759951:18",
            "1205759951:19",
            "1205759962:0",
            "1205759962:1"
        ]
    );
    let e = &first.data[0];
    assert_eq!(e.kind, "evm_in");
    assert_eq!(e.flow_kind(), Some(UsdcFlowKind::EvmIn));
    assert_eq!((e.amount_usd, e.fee_usd), (0.0001, 0.0));
    assert_eq!(
        e.user.as_deref(),
        Some("0x6b9e773128f453f5c2c60935ee2de2cbc5390a24")
    );
    assert!(e.tx_hash.as_deref().unwrap().starts_with("0x"));
    assert_eq!((e.block_number, e.event_index), (1205759951, 16));
    assert_eq!(e.timestamp_ms, 1793577600062);
    assert_eq!(e.timestamp, "2026-11-02T00:00:00.062Z");

    assert_sent(
        &server,
        &["usdc_flows_events_page1", "usdc_flows_events_page2"],
    )
    .await;
}

#[tokio::test]
async fn usdc_flow_event_filters_are_sent() {
    let server = MockServer::start().await;
    mount(&server, "usdc_flows_events_filtered").await;
    let page = client(&server)
        .hyperliquid
        .usdc_flows
        .history(UsdcFlowsParams {
            start: Some("2026-11-02T00:00:00Z".into()),
            end: Some("2026-11-03T00:00:00Z".into()),
            limit: Some(5),
            min_usd: Some(1_000_000.0),
            kind: Some(UsdcFlowKind::Withdraw),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.data.len(), 5);
    assert!(page
        .data
        .iter()
        .all(|e| e.flow_kind() == Some(UsdcFlowKind::Withdraw) && e.amount_usd >= 1_000_000.0));
    assert_eq!(page.data[0].fee_usd, 1.0);
    assert_sent(&server, &["usdc_flows_events_filtered"]).await;
}

#[tokio::test]
async fn the_pager_walks_through_empty_pages_until_the_cursor_stops() {
    let server = MockServer::start().await;
    let chain = [
        "usdc_flows_events_sparse_page1",
        "usdc_flows_events_sparse_page2",
        "usdc_flows_events_sparse_page3",
        "usdc_flows_events_sparse_page4",
    ];
    for name in chain {
        mount(&server, name).await;
    }
    let params = UsdcFlowsParams {
        start: Some("2026-07-01T00:00:00Z".into()),
        end: Some("2026-10-30T00:00:00Z".into()),
        limit: Some(10),
        min_usd: Some(100_000_000_000.0),
        ..Default::default()
    };

    let mut pages = client(&server)
        .hyperliquid
        .usdc_flows
        .iterate(params.clone())
        .unwrap();
    let mut seen = Vec::new();
    while let Some(page) = pages.next_page().await.unwrap() {
        assert!(page.data.is_empty());
        seen.push((page.has_more, page.next_cursor));
    }
    assert_eq!(seen.len(), 4);
    assert!(seen[..3]
        .iter()
        .all(|(more, cursor)| *more && cursor.is_some()));
    assert_eq!(seen[3], (false, None));
    assert_sent(&server, &chain).await;

    // collect_all walks the same chain and returns no rows.
    server.reset().await;
    for name in chain {
        mount(&server, name).await;
    }
    let rows = client(&server)
        .hyperliquid
        .usdc_flows
        .iterate(params)
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert!(rows.is_empty());
    assert_sent(&server, &chain).await;
}

#[tokio::test]
async fn usdc_flow_volume_buckets_decode_and_page() {
    let server = MockServer::start().await;
    mount(&server, "usdc_flows_volume_page1").await;
    mount(&server, "usdc_flows_volume_page2").await;
    let volume = client(&server).hyperliquid.usdc_flows;
    let mut params = UsdcFlowVolumeParams {
        start: Some("2026-11-02T00:00:00Z".into()),
        end: Some("2026-11-02T06:00:00Z".into()),
        interval: Some(OiFundingInterval::OneHour),
        limit: Some(3),
        ..Default::default()
    };
    let first = volume.volume(params.clone()).await.unwrap();
    assert_eq!(first.next_cursor.as_deref(), Some("1793584800000"));
    params.cursor = first.next_cursor.clone();
    let second = volume.volume(params).await.unwrap();
    assert_eq!(second.data[0].timestamp, "2026-11-02T03:00:00.000Z");

    let b = &first.data[0];
    assert_eq!(b.timestamp_ms, ms("2026-11-02T00:00:00Z"));
    assert_eq!((b.deposit_usd, b.deposit_count), (78574.57, 30));
    assert_eq!((b.withdraw_usd, b.withdraw_count), (2427797.87, 120));
    assert_eq!((b.evm_in_usd, b.evm_in_count), (11432121.13, 5069));
    assert_eq!((b.evm_out_usd, b.evm_out_count), (2351409.82, 118));
    let net = b.deposit_usd + b.evm_in_usd - b.withdraw_usd - b.evm_out_usd;
    assert!((net - b.net_usd).abs() < 0.01, "{net} vs {}", b.net_usd);

    assert_sent(
        &server,
        &["usdc_flows_volume_page1", "usdc_flows_volume_page2"],
    )
    .await;
}

#[tokio::test]
async fn iterate_fixes_end_once_and_sends_it_on_every_page() {
    let server = MockServer::start().await;
    let first = recorded("usdc_flows_events_sparse_page1");
    let last = recorded("usdc_flows_events_sparse_page4");
    let cursor = first["body"]["meta"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/usdc-flows/volume"))
        .and(query_param_is_missing("cursor"))
        .respond_with(reply(&first))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/usdc-flows/volume"))
        .and(query_param("cursor", cursor.as_str()))
        .respond_with(reply(&last))
        .expect(1)
        .mount(&server)
        .await;

    let before = chrono::Utc::now().timestamp_millis();
    let rows = client(&server)
        .hyperliquid
        .usdc_flows
        .iterate_volume(UsdcFlowVolumeParams {
            start: Some("2026-11-02T00:00:00Z".into()),
            interval: Some(OiFundingInterval::OneDay),
            ..Default::default()
        })
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    let after = chrono::Utc::now().timestamp_millis();
    assert!(rows.is_empty());

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let queries: Vec<_> = requests.iter().map(sent).collect();
    let end: i64 = queries[0]["end"].parse().unwrap();
    assert!((before..=after).contains(&end));
    for q in &queries {
        assert_eq!(q["end"], end.to_string());
        assert_eq!(q["start"], ms("2026-11-02T00:00:00Z").to_string());
        assert_eq!(q["interval"], "1d");
    }
    assert_eq!(queries[1]["cursor"], cursor);
}

#[tokio::test]
async fn a_repeated_cursor_fails_instead_of_looping() {
    let server = MockServer::start().await;
    let fixture = recorded("usdc_flows_events_sparse_page2");
    let cursor = fixture["body"]["meta"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();
    // The server answers the cursor it was sent.
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/usdc-flows"))
        .respond_with(reply(&fixture))
        .mount(&server)
        .await;

    let mut pages = client(&server)
        .hyperliquid
        .usdc_flows
        .iterate(UsdcFlowsParams {
            start: Some("2026-07-01T00:00:00Z".into()),
            end: Some("2026-10-30T00:00:00Z".into()),
            cursor: Some(cursor.clone()),
            ..Default::default()
        })
        .unwrap();
    let page = pages.next_page().await.unwrap().unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some(cursor.as_str()));
    let err = pages.next_page().await.unwrap_err();
    assert!(
        matches!(err, Error::Deserialize(ref m) if m.contains(&cursor)),
        "{err}"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn iterate_needs_a_start() {
    let c = offline_client();
    let no_start = |e: Error| matches!(e, Error::InvalidParam(ref m) if m.contains("start"));
    assert!(no_start(
        c.hyperliquid
            .trade_intent
            .iterate("BTC", TradeIntentParams::default())
            .unwrap_err()
    ));
    assert!(no_start(
        c.hyperliquid
            .usdc_flows
            .iterate(UsdcFlowsParams::default())
            .unwrap_err()
    ));
    assert!(no_start(
        c.hyperliquid
            .usdc_flows
            .iterate_volume(UsdcFlowVolumeParams::default())
            .unwrap_err()
    ));
    assert!(no_start(
        c.hyperliquid
            .hip3
            .breadth_ma
            .iterate(BreadthMaLength::Five, BreadthMaHistoryParams::default())
            .unwrap_err()
    ));
}

// ---------------------------------------------------------------------------
// Breadth above the n-day moving average
// ---------------------------------------------------------------------------

#[tokio::test]
async fn breadth_ma_current_reads_one_reading() {
    let server = MockServer::start().await;
    mount(&server, "breadth_ma_hl_current_n200").await;
    mount(&server, "breadth_ma_hip3_current_n5").await;
    let c = client(&server);

    let core = c
        .hyperliquid
        .breadth_ma
        .current(BreadthMaLength::TwoHundred)
        .await
        .unwrap();
    assert_eq!(core.n, 200);
    assert_eq!(core.method, "ma-breadth/1");
    assert_eq!(core.value_pct, Some(31.7919));
    assert_eq!(core.coverage_ratio, 0.7793);
    assert_eq!(core.counts.candidates, 222);
    assert_eq!(core.counts.eligible, 173);
    assert_eq!(
        (core.counts.above, core.counts.at, core.counts.below),
        (55, 0, 118)
    );
    assert_eq!(
        (
            core.counts.excluded_stale_price,
            core.counts.excluded_short_history
        ),
        (43, 6)
    );
    assert_eq!(
        core.counts.candidates,
        core.counts.eligible
            + core.counts.excluded_stale_price
            + core.counts.excluded_short_history
    );
    assert_eq!(core.distance.median, Some(-0.081344));
    assert_eq!(core.calculated_at_ms - core.timestamp_ms, 60_000);
    assert!(!core.is_final);

    let hip3 = c
        .hyperliquid
        .hip3
        .breadth_ma
        .current(BreadthMaLength::Five)
        .await
        .unwrap();
    assert_eq!(hip3.n, 5);
    assert_eq!(hip3.distance.p25, Some(-0.060409));

    assert_sent(
        &server,
        &["breadth_ma_hl_current_n200", "breadth_ma_hip3_current_n5"],
    )
    .await;
}

#[tokio::test]
async fn breadth_ma_history_pages_with_n_on_every_page() {
    let server = MockServer::start().await;
    mount(&server, "breadth_ma_hl_n50_1h_page1").await;
    mount(&server, "breadth_ma_hl_n50_1h_page2").await;
    let readings = client(&server)
        .hyperliquid
        .breadth_ma
        .iterate(
            BreadthMaLength::Fifty,
            BreadthMaHistoryParams {
                start: Some("2026-11-02T00:00:00Z".into()),
                end: Some("2026-11-02T04:00:00Z".into()),
                interval: Some(OiFundingInterval::OneHour),
                limit: Some(3),
                ..Default::default()
            },
        )
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(readings.len(), 5);
    assert!(readings.iter().all(|r| r.n == 50 && r.is_final));
    for pair in readings.windows(2) {
        assert_eq!(pair[1].timestamp_ms - pair[0].timestamp_ms, 3_600_000);
    }
    assert_eq!(readings[0].calculated_at, "2026-11-02T01:00:00.000Z");
    assert_eq!(readings[0].value_pct, Some(5.0562));
    assert_sent(
        &server,
        &["breadth_ma_hl_n50_1h_page1", "breadth_ma_hl_n50_1h_page2"],
    )
    .await;
}

#[tokio::test]
async fn hip3_breadth_ma_history_and_coverage_notice() {
    let server = MockServer::start().await;
    mount(&server, "breadth_ma_hip3_n20_1d").await;
    mount(&server, "breadth_ma_before_coverage").await;
    let c = client(&server);

    let page = c
        .hyperliquid
        .hip3
        .breadth_ma
        .history(
            BreadthMaLength::Twenty,
            BreadthMaHistoryParams {
                start: Some("2026-10-31T00:00:00Z".into()),
                end: Some("2026-11-02T00:00:00Z".into()),
                interval: Some(OiFundingInterval::OneDay),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.data.len(), 3);
    assert!(!page.has_more);
    assert_eq!(page.data[0].value_pct, Some(18.543));
    assert_eq!(page.data[0].counts.excluded_short_history, 12);

    let empty = c
        .hyperliquid
        .breadth_ma
        .history(
            BreadthMaLength::TwoHundred,
            BreadthMaHistoryParams {
                start: Some("2025-05-01T00:00:00Z".into()),
                end: Some("2025-05-02T00:00:00Z".into()),
                interval: Some(OiFundingInterval::OneDay),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(empty.data.is_empty() && !empty.has_more);
    assert_eq!(
        empty.meta.coverage_from.as_deref(),
        Some("2025-10-08T00:00:00Z")
    );
    assert!(empty.meta.notice.as_deref().unwrap().contains("200-day"));

    assert_sent(
        &server,
        &["breadth_ma_hip3_n20_1d", "breadth_ma_before_coverage"],
    )
    .await;
}

#[test]
fn breadth_ma_lengths_are_the_four_the_api_accepts() {
    let days: Vec<u32> = BreadthMaLength::ALL.iter().map(|n| n.days()).collect();
    assert_eq!(days, [5, 20, 50, 200]);
    assert_eq!(BreadthMaLength::from_days(50), Some(BreadthMaLength::Fifty));
    assert_eq!(BreadthMaLength::from_days(7), None);
}

// ---------------------------------------------------------------------------
// Positioning
// ---------------------------------------------------------------------------

#[tokio::test]
async fn positioning_reads_the_cross_section_and_its_close() {
    let server = MockServer::start().await;
    mount(&server, "positioning_hl_4h").await;
    mount(&server, "positioning_hip3_1h").await;
    let c = client(&server);

    let core = c
        .hyperliquid
        .positioning
        .current(PositioningWindow::FourHours)
        .await
        .unwrap();
    // The fixture keeps the first 3 of the 179 records; meta is as recorded.
    assert_eq!(core.meta.count, 179);
    assert_eq!(core.data.len(), 3);
    assert!(!core.has_more && core.next_cursor.is_none());
    assert_eq!(core.meta.as_of.as_deref(), Some("2026-11-04T20:00:00.000Z"));
    let symbols: Vec<&str> = core.data.iter().map(|r| r.symbol.as_str()).collect();
    let mut sorted = symbols.clone();
    sorted.sort();
    assert_eq!(symbols, sorted);
    let r = &core.data[0];
    assert_eq!(r.symbol, "0G");
    assert_eq!(r.window, "4h");
    assert_eq!(r.window_end, "2026-11-04T20:00:00.000Z");
    assert_eq!(r.window_end_ms - r.window_start_ms, 4 * 3_600_000);
    assert_eq!((r.oi_start, r.oi_end), (Some(6731236.0), Some(6720526.0)));
    assert_eq!((r.mark_start, r.mark_end), (Some(0.161752), Some(0.15742)));
    assert_eq!(r.oi_buckets, 48);
    assert_eq!((r.buy_usd, r.sell_usd), (30896.46, 32642.46));
    assert_eq!(
        (
            r.liq_long_usd,
            r.liq_short_usd,
            r.liq_long_count,
            r.liq_short_count
        ),
        (0.0, 0.0, 0, 0)
    );
    assert_eq!(r.typical_price, Some(0.01190657284537222));
    assert_eq!(r.typical_windows, 1952);

    let hip3 = c
        .hyperliquid
        .hip3
        .positioning
        .current(PositioningWindow::OneHour)
        .await
        .unwrap();
    assert_eq!(
        hip3.meta.as_of.as_deref(),
        Some(hip3.data[0].window_end.as_str())
    );
    assert!(hip3.data.iter().all(|r| r.window == "1h"));

    assert_sent(&server, &["positioning_hl_4h", "positioning_hip3_1h"]).await;
}

#[test]
fn positioning_windows_are_named_as_the_api_names_them() {
    let names: Vec<&str> = [
        PositioningWindow::OneHour,
        PositioningWindow::FourHours,
        PositioningWindow::OneDay,
    ]
    .iter()
    .map(|w| w.as_str())
    .collect();
    assert_eq!(names, ["1h", "4h", "24h"]);
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_free_plan_refusal_is_typed() {
    let server = MockServer::start().await;
    for name in [
        "usdc_flows_free_forbidden",
        "trade_intent_free_forbidden",
        "breadth_ma_free_forbidden",
        "positioning_free_forbidden",
    ] {
        mount_on_path(&server, name).await;
    }
    let c = client(&server);
    let refusals: Vec<(Error, &str, &str)> = vec![
        (
            c.hyperliquid
                .usdc_flows
                .history(UsdcFlowsParams::default())
                .await
                .unwrap_err(),
            "usdc_flows",
            "usdc_flows_free_forbidden",
        ),
        (
            c.hyperliquid
                .trade_intent
                .history("BTC", TradeIntentParams::default())
                .await
                .unwrap_err(),
            "trade_intent",
            "trade_intent_free_forbidden",
        ),
        (
            c.hyperliquid
                .breadth_ma
                .current(BreadthMaLength::Five)
                .await
                .unwrap_err(),
            "breadth_ma",
            "breadth_ma_free_forbidden",
        ),
        (
            c.hyperliquid
                .positioning
                .current(PositioningWindow::OneHour)
                .await
                .unwrap_err(),
            "positioning",
            "positioning_free_forbidden",
        ),
    ];
    for (err, datatype, fixture) in refusals {
        let body = &recorded(fixture)["body"];
        assert_eq!(err.status(), Some(403));
        assert_eq!(err.error_code(), Some(&ErrorCode::Forbidden));
        assert_eq!(err.request_id(), body["request_id"].as_str());
        let plan = err.plan_required().expect(datatype);
        assert_eq!(plan.datatype.as_deref(), Some(datatype));
        assert_eq!(plan.required_plans, PLANS);
        assert_eq!(plan.upgrade_url.as_deref(), Some(PRICING));
        match &err {
            Error::Api { message, plan, .. } => {
                assert_eq!(Some(message.as_str()), body["error"].as_str());
                assert!(plan.is_some());
            }
            other => panic!("expected an API error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn refusals_keep_their_contract_fields() {
    let server = MockServer::start().await;
    // Each recorded refusal is answered on the route the SDK calls.
    let cases: [(&str, &str); 6] = [
        ("breadth_ma_missing_n", "/v1/hyperliquid/breadth/above-ma"),
        (
            "breadth_ma_current_refuses_window",
            "/v1/hyperliquid/hip3/breadth/above-ma/current",
        ),
        (
            "trade_intent_invalid_interval",
            "/v1/hyperliquid/trade-intent/BTC",
        ),
        (
            "usdc_flows_volume_refuses_min_usd",
            "/v1/hyperliquid/usdc-flows/volume",
        ),
        (
            "positioning_missing_window",
            "/v1/hyperliquid/hip3/positioning",
        ),
        ("positioning_cold_503", "/v1/hyperliquid/positioning"),
    ];
    for (name, route) in cases {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(reply(&recorded(name)))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/usdc-flows"))
        .respond_with(reply(&recorded("usdc_flows_hip3_unsupported")))
        .mount(&server)
        .await;
    let c = client(&server);
    let hl = &c.hyperliquid;

    let e = hl
        .breadth_ma
        .history(BreadthMaLength::Five, BreadthMaHistoryParams::default())
        .await
        .unwrap_err();
    assert_eq!(e.error_code(), Some(&ErrorCode::InvalidParameter));
    assert_eq!(e.param(), Some("n"));
    assert_eq!(e.valid_values().unwrap(), ["5", "20", "50", "200"]);
    assert!(e.plan_required().is_none());

    let e = hl
        .hip3
        .breadth_ma
        .current(BreadthMaLength::Five)
        .await
        .unwrap_err();
    assert_eq!((e.status(), e.param()), (Some(400), Some("interval")));

    let e = hl
        .trade_intent
        .history("BTC", TradeIntentParams::default())
        .await
        .unwrap_err();
    assert_eq!(e.error_code(), Some(&ErrorCode::InvalidInterval));
    assert_eq!(e.valid_values().unwrap().len(), 9);

    let e = hl
        .usdc_flows
        .volume(UsdcFlowVolumeParams::default())
        .await
        .unwrap_err();
    assert_eq!(
        (e.error_code(), e.param()),
        (Some(&ErrorCode::InvalidParameter), Some("min_usd"))
    );

    let e = hl
        .hip3
        .positioning
        .current(PositioningWindow::OneHour)
        .await
        .unwrap_err();
    assert_eq!(e.param(), Some("window"));
    assert_eq!(e.valid_values().unwrap(), ["1h", "4h", "24h"]);

    let e = hl
        .positioning
        .current(PositioningWindow::FourHours)
        .await
        .unwrap_err();
    assert_eq!(e.status(), Some(503));
    assert_eq!(e.error_code(), Some(&ErrorCode::UpstreamUnavailable));
    assert!(e.to_string().contains("still being prepared"));

    let e = hl
        .usdc_flows
        .history(UsdcFlowsParams::default())
        .await
        .unwrap_err();
    assert_eq!(
        (e.status(), e.error_code()),
        (Some(404), Some(&ErrorCode::UnsupportedForVenue))
    );
    assert!(e.plan_required().is_none());
}

#[tokio::test]
async fn bad_arguments_are_refused_before_sending() {
    let c = offline_client();
    let hl = &c.hyperliquid;
    let invalid = |e: Error| matches!(e, Error::InvalidParam(_));

    for limit in [0, 2_001] {
        let params = TradeIntentParams {
            limit: Some(limit),
            ..Default::default()
        };
        assert!(invalid(
            hl.trade_intent.history("BTC", params).await.unwrap_err()
        ));
    }
    for limit in [0, 1_001] {
        let events = UsdcFlowsParams {
            limit: Some(limit),
            ..Default::default()
        };
        assert!(invalid(hl.usdc_flows.history(events).await.unwrap_err()));
        let volume = UsdcFlowVolumeParams {
            limit: Some(limit),
            ..Default::default()
        };
        assert!(invalid(hl.usdc_flows.volume(volume).await.unwrap_err()));
        let breadth = BreadthMaHistoryParams {
            limit: Some(limit),
            ..Default::default()
        };
        assert!(invalid(
            hl.hip3
                .breadth_ma
                .history(BreadthMaLength::Twenty, breadth)
                .await
                .unwrap_err()
        ));
    }
    for min_usd in [-1.0, 1.5e12, f64::NAN, f64::INFINITY] {
        let params = UsdcFlowsParams {
            min_usd: Some(min_usd),
            ..Default::default()
        };
        assert!(invalid(hl.usdc_flows.history(params).await.unwrap_err()));
    }
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn capability_rows_name_the_plans_that_include_them() {
    let server = MockServer::start().await;
    mount_on_path(&server, "capabilities_derived_rows").await;
    let rows = client(&server).capabilities().await.unwrap();
    assert_eq!(rows.len(), 7);
    for row in &rows {
        assert_eq!(row.plans.as_deref().unwrap(), PLANS, "{}", row.datatype);
        assert_eq!(row.ws_endpoint, None);
    }
    let find = |venue: &str, datatype: &str| row(&rows, venue, datatype);
    let flows = find("hyperliquid", "usdc_flows");
    assert_eq!(
        flows.available_from.as_deref(),
        Some("2025-09-27T09:33:00.000Z")
    );
    assert_eq!(flows.rest_routes.len(), 2);
    assert_eq!(
        find("hip3", "trade_intent").available_from.as_deref(),
        Some("2025-10-13T12:00:00.000Z")
    );
    assert_eq!(find("hyperliquid", "trade_intent").intervals.len(), 9);
    assert_eq!(
        find("hyperliquid", "breadth_ma").available_from.as_deref(),
        Some("2025-03-27T00:00:00.000Z")
    );
    assert_eq!(
        find("hip3", "breadth_ma").available_from.as_deref(),
        Some("2025-12-27T00:00:00.000Z")
    );
    let positioning = find("hyperliquid", "positioning");
    assert_eq!(
        (
            positioning.available_from.as_deref(),
            positioning.page_limit
        ),
        (None, None)
    );
}

fn row<'a>(rows: &'a [Capability], venue: &str, datatype: &str) -> &'a Capability {
    rows.iter()
        .find(|r| r.venue == venue && r.datatype == datatype)
        .unwrap()
}
