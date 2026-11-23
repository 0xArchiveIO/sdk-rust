//! The momentum factor (`client.hyperliquid.factors.momentum`), against
//! responses recorded from the API (version 2026-10-01) in
//! `tests/fixtures/momentum/`.
//!
//! Each fixture holds the request that was sent (`path`, `query`) and the
//! response (`status`, `body`). The mocks answer a fixture's body only to
//! its exact request, and the tests check that the SDK sent exactly the
//! recorded query, nothing more.

use std::collections::BTreeMap;

use oxarchive::resources::momentum::MOMENTUM_MAX_LIMIT;
use oxarchive::types::Timestamp;
use oxarchive::{
    Error, ErrorCode, MomentumInterval, MomentumLookback, MomentumReturn, MomentumReturnsParams,
    MomentumScore, MomentumUniverse, OxArchive,
};
use serde_json::Value;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const PLANS: [&str; 4] = ["build", "pro", "scale", "enterprise"];
const PRICING: &str = "https://0xarchive.io/pricing";
const COVERAGE: &str = "2025-05-24T01:00:00Z";
const HOUR: i64 = 3_600_000;
const DAY: i64 = 86_400_000;
const SCORES: &str = "/v1/hyperliquid/factors/momentum";
const CURRENT: &str = "/v1/hyperliquid/factors/momentum/current";
const RETURNS: &str = "/v1/hyperliquid/factors/momentum/returns";
const PAGES: [&str; 2] = [
    "momentum_returns_7d_top25_1d_page1",
    "momentum_returns_7d_top25_1d_page2",
];
const HOLDINGS: &str = "momentum_returns_14d_top50_1h_holdings";

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
        "{}/tests/fixtures/momentum/{name}.json",
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

fn query_ms(name: &str, key: &str) -> i64 {
    recorded_query(name)[key].parse().unwrap()
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

/// Answer `name`'s body to any GET on `route`.
async fn mount_on(server: &MockServer, route: &str, name: &str) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(reply(&recorded(name)))
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

fn window(name: &str) -> MomentumReturnsParams {
    let q = recorded_query(name);
    assert_eq!(
        (q["universe"].as_str(), q["interval"].as_str()),
        ("top25", "1d")
    );
    MomentumReturnsParams {
        universe: Some(MomentumUniverse::Top25),
        interval: Some(MomentumInterval::OneDay),
        start: Some(Timestamp::Millis(query_ms(name, "start"))),
        end: Some(Timestamp::Millis(query_ms(name, "end"))),
        limit: Some(query_ms(name, "limit")),
        ..Default::default()
    }
}

fn check_scores(scores: &[MomentumScore], raw: &Value, lookback: &str) {
    let raw = raw.as_array().unwrap();
    assert!(!scores.is_empty());
    assert_eq!(scores.len(), raw.len());
    for (s, r) in scores.iter().zip(raw) {
        assert_eq!(s.symbol, r["symbol"].as_str().unwrap());
        assert_eq!(s.timestamp, r["timestamp"].as_str().unwrap());
        assert_eq!(s.timestamp_ms, r["timestamp_ms"].as_i64().unwrap());
        assert_eq!(s.lookback, lookback);
        assert_eq!(s.method, "momentum/1");
        assert_eq!(s.ret, r["ret"].as_f64());
        assert_eq!(s.vol, r["vol"].as_f64());
        assert_eq!(s.score, r["score"].as_f64().unwrap());
        assert_eq!(s.pct_rank, r["pct_rank"].as_f64().unwrap());
        assert_eq!(u64::from(s.quintile), r["quintile"].as_u64().unwrap());
        assert_eq!(s.in_top25, r["in_top25"].as_bool().unwrap());
        assert_eq!(s.in_top50, r["in_top50"].as_bool().unwrap());
        assert_eq!(s.is_final, r["is_final"].as_bool().unwrap());
        assert!(s.pct_rank > 0.0 && s.pct_rank <= 1.0);
        assert!((1..=5).contains(&s.quintile));
        // The top-25 universe is part of the top-50 one.
        assert!(!s.in_top25 || s.in_top50);
    }
    // One reading: every perp at the same hour close, strongest first.
    assert!(scores
        .iter()
        .all(|s| s.timestamp_ms == scores[0].timestamp_ms));
    assert_eq!(scores[0].timestamp_ms % HOUR, 0);
    assert!(scores.windows(2).all(|w| w[0].score >= w[1].score));
}

fn check_returns(buckets: &[MomentumReturn], raw: &Value) {
    let raw = raw.as_array().unwrap();
    assert_eq!(buckets.len(), raw.len());
    for (b, r) in buckets.iter().zip(raw) {
        assert_eq!(b.timestamp, r["timestamp"].as_str().unwrap());
        assert_eq!(b.timestamp_ms, r["timestamp_ms"].as_i64().unwrap());
        assert_eq!(b.calculated_at, r["calculated_at"].as_str().unwrap());
        assert_eq!(b.calculated_at_ms, r["calculated_at_ms"].as_i64().unwrap());
        assert_eq!(b.lookback, r["lookback"].as_str().unwrap());
        assert_eq!(b.universe, r["universe"].as_str().unwrap());
        assert_eq!(b.method, "momentum/1");
        assert_eq!(b.ret, r["ret"].as_f64().unwrap());
        assert_eq!(b.long_ret, r["long_ret"].as_f64().unwrap());
        assert_eq!(b.short_ret, r["short_ret"].as_f64().unwrap());
        assert_eq!(u64::from(b.hours), r["hours"].as_u64().unwrap());
        assert_eq!(u64::from(b.longs), r["longs"].as_u64().unwrap());
        assert_eq!(u64::from(b.shorts), r["shorts"].as_u64().unwrap());
        assert_eq!(b.is_final, r["is_final"].as_bool().unwrap());
        // The factor is the long leg less the short leg.
        assert!((b.ret - (b.long_ret - b.short_ret)).abs() < 1e-12);
        assert!(b.calculated_at_ms > b.timestamp_ms);
    }
}

// ---------------------------------------------------------------------------
// Scores
// ---------------------------------------------------------------------------

#[tokio::test]
async fn current_sends_the_lookback_and_reads_every_perp() {
    let server = MockServer::start().await;
    let name = "momentum_current_14d";
    mount(&server, name).await;

    let page = client(&server)
        .hyperliquid
        .factors
        .momentum
        .current(MomentumLookback::FourteenDays)
        .await
        .unwrap();

    let fixture = recorded(name);
    assert_eq!(fixture["request"]["path"], CURRENT);
    assert!(!page.has_more);
    assert!(page.next_cursor.is_none());
    assert_eq!(
        page.meta.request_id,
        fixture["body"]["meta"]["request_id"].as_str().unwrap()
    );
    // Recorded; the fixture keeps 6 of the records.
    assert_eq!(page.meta.count, 178);
    check_scores(&page.data, &fixture["body"]["data"], "14d");
    let first = &page.data[0];
    assert_eq!(first.symbol, "BLUR");
    assert_eq!(first.timestamp, "2026-11-23T16:00:00.000Z");
    assert_eq!(
        (first.pct_rank, first.quintile, first.is_final),
        (1.0, 5, false)
    );
    assert!((first.score - first.ret.unwrap() / first.vol.unwrap()).abs() < 1e-9);
    assert_eq!(page.data.last().unwrap().quintile, 1);
    assert_sent(&server, &[name]).await;
}

#[tokio::test]
async fn get_sends_at_in_milliseconds_and_reads_that_hour() {
    let server = MockServer::start().await;
    let name = "momentum_scores_7d_at";
    let fixture = recorded(name);
    let query = fixture["request"]["query"].as_object().unwrap().clone();
    let mut mock = Mock::given(method("GET")).and(path(SCORES));
    for (k, v) in &query {
        mock = mock.and(query_param(k.as_str(), wire(v).as_str()));
    }
    mock.respond_with(reply(&fixture))
        .expect(2)
        .mount(&server)
        .await;
    assert_eq!(query_ms(name, "at"), ms("2026-11-01T12:00:00Z"));
    let momentum = client(&server).hyperliquid.factors.momentum;

    let by_text = momentum
        .get(
            MomentumLookback::SevenDays,
            Some("2026-11-01T12:00:00Z".into()),
        )
        .await
        .unwrap();
    let by_datetime = momentum
        .get(
            MomentumLookback::SevenDays,
            Some(
                chrono::DateTime::parse_from_rfc3339("2026-11-01T12:00:00Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc)
                    .into(),
            ),
        )
        .await
        .unwrap();

    for page in [&by_text, &by_datetime] {
        check_scores(&page.data, &fixture["body"]["data"], "7d");
        // The reading is as of the close of the hour at or before `at`.
        assert_eq!(page.data[0].timestamp, "2026-11-01T12:00:00.000Z");
        assert!(page.data.iter().all(|s| s.is_final));
    }
    assert_eq!(by_text.data[0].symbol, "HYPE");
    assert!(by_text.data[0].in_top25);
    assert_sent(&server, &[name, name]).await;
}

#[tokio::test]
async fn get_without_at_sends_only_the_lookback() {
    let server = MockServer::start().await;
    mount_on(&server, SCORES, "momentum_current_14d").await;

    client(&server)
        .hyperliquid
        .factors
        .momentum
        .get(MomentumLookback::FourteenDays, None)
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        sent(&requests[0]),
        BTreeMap::from([("lookback".to_string(), "14d".to_string())])
    );
}

#[test]
fn null_returns_and_volatility_decode_as_none() {
    let mut row = recorded("momentum_current_14d")["body"]["data"][0].clone();
    row["ret"] = Value::Null;
    row["vol"] = Value::Null;
    let score: MomentumScore = serde_json::from_value(row).unwrap();
    assert_eq!((score.ret, score.vol), (None, None));
}

// ---------------------------------------------------------------------------
// Returns
// ---------------------------------------------------------------------------

#[tokio::test]
async fn returns_page_with_the_window_unchanged() {
    let server = MockServer::start().await;
    mount(&server, PAGES[0]).await;
    mount(&server, PAGES[1]).await;
    let momentum = client(&server).hyperliquid.factors.momentum;

    let first = momentum
        .returns(MomentumLookback::SevenDays, window(PAGES[0]))
        .await
        .unwrap();
    let second = momentum
        .returns(
            MomentumLookback::SevenDays,
            MomentumReturnsParams {
                cursor: first.next_cursor.clone(),
                ..window(PAGES[0])
            },
        )
        .await
        .unwrap();

    assert_eq!(recorded(PAGES[0])["request"]["path"], RETURNS);
    assert_eq!(query_ms(PAGES[0], "start"), ms("2026-10-01T00:00:00Z"));
    assert!(first.has_more && second.has_more);
    assert_eq!(first.next_cursor.as_deref(), Some("1791676800000"));
    assert_eq!(second.next_cursor.as_deref(), Some("1792540800000"));
    check_returns(&first.data, &recorded(PAGES[0])["body"]["data"]);
    check_returns(&second.data, &recorded(PAGES[1])["body"]["data"]);
    let joined: Vec<&MomentumReturn> = first.data.iter().chain(&second.data).collect();
    let opens: Vec<i64> = joined.iter().map(|b| b.timestamp_ms).collect();
    // Daily buckets, oldest first, none repeated across the page boundary;
    // the cursor is the next bucket's open.
    assert_eq!(opens[0], ms("2026-10-01T00:00:00Z"));
    assert!(opens.windows(2).all(|w| w[1] - w[0] == DAY));
    assert_eq!(
        Some(second.data[0].timestamp_ms.to_string()),
        first.next_cursor
    );
    for b in &joined {
        assert_eq!(
            (
                b.lookback.as_str(),
                b.universe.as_str(),
                b.hours,
                b.is_final
            ),
            ("7d", "top25", 24, true)
        );
        assert_eq!(b.calculated_at_ms, b.timestamp_ms + DAY);
        assert!(b.holdings.is_none());
    }
    let oct1 = &first.data[0];
    assert!((oct1.ret - 0.011022206600979339).abs() < 1e-15);
    assert_eq!((oct1.longs, oct1.shorts), (6, 5));
    assert_sent(&server, &PAGES).await;
}

#[tokio::test]
async fn iterate_returns_sends_the_cursor_with_the_same_window() {
    let server = MockServer::start().await;
    mount(&server, PAGES[0]).await;
    mount(&server, PAGES[1]).await;

    let mut pages = client(&server)
        .hyperliquid
        .factors
        .momentum
        .iterate_returns(MomentumLookback::SevenDays, window(PAGES[0]))
        .unwrap();
    // Two recorded pages of a longer walk.
    let mut rows = Vec::new();
    for _ in 0..2 {
        rows.extend(pages.next_page().await.unwrap().unwrap().data);
    }

    let want: Vec<i64> = PAGES
        .iter()
        .flat_map(|n| {
            recorded(n)["body"]["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["timestamp_ms"].as_i64().unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        rows.iter().map(|b| b.timestamp_ms).collect::<Vec<_>>(),
        want
    );
    assert_sent(&server, &PAGES).await;
}

#[tokio::test]
async fn iterate_returns_stops_on_the_page_without_a_cursor() {
    let server = MockServer::start().await;
    mount(&server, HOLDINGS).await;

    let rows = client(&server)
        .hyperliquid
        .factors
        .momentum
        .iterate_returns(
            MomentumLookback::FourteenDays,
            MomentumReturnsParams {
                universe: Some(MomentumUniverse::Top50),
                interval: Some(MomentumInterval::OneHour),
                holdings: Some(true),
                start: Some(Timestamp::Millis(query_ms(HOLDINGS, "start"))),
                end: Some(Timestamp::Millis(query_ms(HOLDINGS, "end"))),
                ..Default::default()
            },
        )
        .unwrap()
        .collect_all()
        .await
        .unwrap();

    assert_eq!(rows.len(), 4);
    assert_sent(&server, &[HOLDINGS]).await;
}

#[tokio::test]
async fn iterate_returns_fixes_end_once_and_sends_it_on_every_page() {
    let server = MockServer::start().await;
    let first = recorded(PAGES[0]);
    let cursor = first["body"]["meta"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();
    Mock::given(method("GET"))
        .and(path(RETURNS))
        .and(query_param_is_missing("cursor"))
        .respond_with(reply(&first))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(RETURNS))
        .and(query_param("cursor", cursor.as_str()))
        .respond_with(reply(&recorded(HOLDINGS)))
        .expect(1)
        .mount(&server)
        .await;

    let before = chrono::Utc::now().timestamp_millis();
    client(&server)
        .hyperliquid
        .factors
        .momentum
        .iterate_returns(
            MomentumLookback::SevenDays,
            MomentumReturnsParams {
                start: Some("2026-10-01T00:00:00Z".into()),
                interval: Some(MomentumInterval::OneDay),
                ..Default::default()
            },
        )
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    let after = chrono::Utc::now().timestamp_millis();

    let queries: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(sent)
        .collect();
    assert_eq!(queries.len(), 2);
    let end: i64 = queries[0]["end"].parse().unwrap();
    assert!((before..=after).contains(&end));
    for q in &queries {
        assert_eq!(q["end"], end.to_string());
        assert_eq!(q["start"], ms("2026-10-01T00:00:00Z").to_string());
        assert_eq!(q["lookback"], "7d");
        assert_eq!(q["interval"], "1d");
    }
    assert!(!queries[0].contains_key("cursor"));
    assert_eq!(queries[1]["cursor"], cursor);
}

#[tokio::test]
async fn a_repeated_cursor_fails_instead_of_looping() {
    let server = MockServer::start().await;
    let fixture = recorded(PAGES[1]);
    let cursor = fixture["body"]["meta"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();
    // The server answers the cursor it was sent.
    mount_on(&server, RETURNS, PAGES[1]).await;

    let mut pages = client(&server)
        .hyperliquid
        .factors
        .momentum
        .iterate_returns(
            MomentumLookback::SevenDays,
            MomentumReturnsParams {
                cursor: Some(cursor.clone()),
                ..window(PAGES[0])
            },
        )
        .unwrap();
    pages.next_page().await.unwrap().unwrap();
    let err = pages.next_page().await.unwrap_err();
    assert!(
        matches!(err, Error::Deserialize(ref m) if m.contains(&cursor)),
        "{err}"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn returns_with_holdings_the_top_50_universe_and_hourly_buckets() {
    let server = MockServer::start().await;
    mount(&server, HOLDINGS).await;
    assert_eq!(recorded_query(HOLDINGS)["holdings"], "true");

    let page = client(&server)
        .hyperliquid
        .factors
        .momentum
        .returns(
            MomentumLookback::FourteenDays,
            MomentumReturnsParams {
                universe: Some(MomentumUniverse::Top50),
                interval: Some(MomentumInterval::OneHour),
                holdings: Some(true),
                start: Some("2026-11-15T00:00:00Z".into()),
                end: Some("2026-11-15T03:00:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(!page.has_more && page.next_cursor.is_none());
    let raw = &recorded(HOLDINGS)["body"]["data"];
    check_returns(&page.data, raw);
    // `end` is a bucket open: the 03:00 bucket is the last one.
    let opens: Vec<&str> = page.data.iter().map(|b| b.timestamp.as_str()).collect();
    assert_eq!(
        opens,
        [
            "2026-11-15T00:00:00.000Z",
            "2026-11-15T01:00:00.000Z",
            "2026-11-15T02:00:00.000Z",
            "2026-11-15T03:00:00.000Z"
        ]
    );
    for (b, r) in page.data.iter().zip(raw.as_array().unwrap()) {
        assert_eq!(
            (b.universe.as_str(), b.lookback.as_str(), b.hours),
            ("top50", "14d", 1)
        );
        assert_eq!(b.calculated_at_ms, b.timestamp_ms + HOUR);
        let holdings = b.holdings.as_ref().unwrap();
        let want: Vec<(String, f64)> = r["holdings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| {
                (
                    h["symbol"].as_str().unwrap().to_string(),
                    h["weight"].as_f64().unwrap(),
                )
            })
            .collect();
        let got: Vec<(String, f64)> = holdings
            .iter()
            .map(|h| (h.symbol.clone(), h.weight))
            .collect();
        assert_eq!(got, want);
        let longs: Vec<f64> = holdings
            .iter()
            .map(|h| h.weight)
            .filter(|w| *w > 0.0)
            .collect();
        let shorts: Vec<f64> = holdings
            .iter()
            .map(|h| h.weight)
            .filter(|w| *w < 0.0)
            .collect();
        assert_eq!((longs.len(), shorts.len()), (11, 10));
        assert_eq!((b.longs, b.shorts), (11, 10));
        // Each leg sums to 1 in size, long leg first.
        assert!((longs.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((shorts.iter().sum::<f64>() + 1.0).abs() < 1e-9);
        assert!(holdings[..longs.len()].iter().all(|h| h.weight > 0.0));
    }
    assert_eq!(page.data[0].holdings.as_ref().unwrap()[0].symbol, "STRK");
    assert_sent(&server, &[HOLDINGS]).await;
}

#[tokio::test]
async fn a_window_before_coverage_is_empty_with_the_notice() {
    let server = MockServer::start().await;
    mount_on(&server, RETURNS, "momentum_returns_before_coverage").await;

    let page = client(&server)
        .hyperliquid
        .factors
        .momentum
        .returns(
            MomentumLookback::SevenDays,
            MomentumReturnsParams {
                start: Some("2025-05-01".into()),
                end: Some("2025-05-02".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(page.data.is_empty() && !page.has_more);
    assert_eq!(
        page.meta.coverage_from.as_deref().map(ms),
        Some(ms(COVERAGE))
    );
    assert!(page
        .meta
        .notice
        .as_deref()
        .unwrap()
        .contains("returns begin 2025-05-24 01:00 UTC"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        sent(&requests[0]),
        BTreeMap::from([
            ("lookback".to_string(), "7d".to_string()),
            ("start".to_string(), ms("2025-05-01T00:00:00Z").to_string()),
            ("end".to_string(), ms("2025-05-02T00:00:00Z").to_string()),
        ])
    );
}

#[tokio::test]
async fn holdings_false_is_sent_and_unset_parameters_are_left_to_the_api() {
    let server = MockServer::start().await;
    mount_on(&server, RETURNS, HOLDINGS).await;
    let momentum = client(&server).hyperliquid.factors.momentum;

    momentum
        .returns(
            MomentumLookback::FourteenDays,
            MomentumReturnsParams::default(),
        )
        .await
        .unwrap();
    momentum
        .returns(
            MomentumLookback::FourteenDays,
            MomentumReturnsParams {
                holdings: Some(false),
                start: Some("2026-11-01".into()),
                end: Some("2026-11-02T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let queries: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(sent)
        .collect();
    let q = |pairs: &[(&str, String)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    };
    assert_eq!(queries[0], q(&[("lookback", "14d".into())]));
    assert_eq!(
        queries[1],
        q(&[
            ("lookback", "14d".into()),
            ("holdings", "false".into()),
            ("start", ms("2026-11-01T00:00:00Z").to_string()),
            ("end", ms("2026-11-02T00:00:00Z").to_string()),
        ])
    );
}

#[test]
fn the_wire_values_are_the_ones_the_api_accepts() {
    assert_eq!(MomentumLookback::ALL.map(|l| l.as_str()), ["7d", "14d"]);
    assert_eq!(
        MomentumLookback::from_wire("14d"),
        Some(MomentumLookback::FourteenDays)
    );
    for removed in ["1d", "28d", "7D", ""] {
        assert_eq!(MomentumLookback::from_wire(removed), None);
    }
    assert_eq!(
        [MomentumUniverse::Top25, MomentumUniverse::Top50].map(|u| u.as_str()),
        ["top25", "top50"]
    );
    assert_eq!(
        [MomentumInterval::OneHour, MomentumInterval::OneDay].map(|i| i.as_str()),
        ["1h", "1d"]
    );
}

#[tokio::test]
async fn bad_arguments_are_refused_before_sending() {
    let c = offline_client();
    let momentum = &c.hyperliquid.factors.momentum;
    for limit in [0, MOMENTUM_MAX_LIMIT + 1] {
        let params = MomentumReturnsParams {
            limit: Some(limit),
            ..Default::default()
        };
        let err = momentum
            .returns(MomentumLookback::SevenDays, params.clone())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "{err}");
        let err = momentum
            .iterate_returns(
                MomentumLookback::SevenDays,
                MomentumReturnsParams {
                    start: Some("2026-10-01".into()),
                    ..params
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "{err}");
    }
    let err = momentum
        .iterate_returns(
            MomentumLookback::SevenDays,
            MomentumReturnsParams::default(),
        )
        .unwrap_err();
    assert!(
        matches!(err, Error::InvalidParam(ref m) if m.contains("start")),
        "{err}"
    );
}

// ---------------------------------------------------------------------------
// Errors and capabilities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_free_plan_refusal_is_typed() {
    let server = MockServer::start().await;
    mount_on(&server, CURRENT, "momentum_free_forbidden").await;
    mount_on(&server, RETURNS, "momentum_free_forbidden_returns").await;
    let momentum = client(&server).hyperliquid.factors.momentum;

    let refusals = [
        (
            momentum
                .current(MomentumLookback::SevenDays)
                .await
                .unwrap_err(),
            "momentum_free_forbidden",
        ),
        (
            momentum
                .returns(
                    MomentumLookback::SevenDays,
                    MomentumReturnsParams::default(),
                )
                .await
                .unwrap_err(),
            "momentum_free_forbidden_returns",
        ),
    ];
    for (err, name) in refusals {
        let body = &recorded(name)["body"];
        assert_eq!(err.status(), Some(403));
        assert_eq!(err.error_code(), Some(&ErrorCode::Forbidden));
        assert_eq!(err.request_id(), body["request_id"].as_str());
        let plan = err.plan_required().expect("a plan refusal");
        assert_eq!(plan.datatype.as_deref(), Some("momentum"));
        assert_eq!(plan.required_plans, PLANS);
        assert_eq!(plan.upgrade_url.as_deref(), Some(PRICING));
        match &err {
            Error::Api { message, .. } => {
                assert_eq!(Some(message.as_str()), body["error"].as_str())
            }
            other => panic!("expected an API error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn refusals_keep_their_contract_fields() {
    type Case = (
        &'static str,
        u16,
        ErrorCode,
        Option<&'static str>,
        Option<&'static [&'static str]>,
    );
    let cases: [Case; 5] = [
        (
            "momentum_missing_lookback",
            400,
            ErrorCode::InvalidParameter,
            Some("lookback"),
            Some(&["7d", "14d"]),
        ),
        (
            "momentum_lookback_28d",
            400,
            ErrorCode::InvalidParameter,
            Some("lookback"),
            Some(&["7d", "14d"]),
        ),
        (
            "momentum_current_refuses_at",
            400,
            ErrorCode::InvalidParameter,
            Some("at"),
            None,
        ),
        (
            "momentum_returns_invalid_interval",
            400,
            ErrorCode::InvalidInterval,
            Some("interval"),
            Some(&["1h", "1d"]),
        ),
        (
            "momentum_cold_503",
            503,
            ErrorCode::UpstreamUnavailable,
            None,
            None,
        ),
    ];
    for (name, status, code, param, values) in cases {
        let server = MockServer::start().await;
        mount_on(&server, RETURNS, name).await;
        let err = client(&server)
            .hyperliquid
            .factors
            .momentum
            .returns(
                MomentumLookback::SevenDays,
                MomentumReturnsParams::default(),
            )
            .await
            .unwrap_err();
        let body = &recorded(name)["body"];
        assert_eq!(err.status(), Some(status), "{name}");
        assert_eq!(err.error_code(), Some(&code), "{name}");
        assert_eq!(err.param(), param, "{name}");
        assert_eq!(
            err.valid_values().map(|v| v.to_vec()),
            values.map(|v| v.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            "{name}"
        );
        assert!(err.plan_required().is_none(), "{name}");
        assert_eq!(err.request_id(), body["request_id"].as_str(), "{name}");
        match &err {
            Error::Api { message, .. } => {
                assert_eq!(Some(message.as_str()), body["error"].as_str(), "{name}")
            }
            other => panic!("expected an API error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn a_cold_start_answers_a_retryable_503() {
    let server = MockServer::start().await;
    mount(&server, "momentum_cold_503").await;

    let err = client(&server)
        .hyperliquid
        .factors
        .momentum
        .current(MomentumLookback::SevenDays)
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(503));
    assert_eq!(err.error_code(), Some(&ErrorCode::UpstreamUnavailable));
    assert!(err.to_string().contains("still being prepared"), "{err}");
    assert!(err.plan_required().is_none());
}

#[tokio::test]
async fn the_capability_row_names_the_plans_the_routes_and_the_limits() {
    let server = MockServer::start().await;
    mount(&server, "capabilities_momentum_row").await;

    let rows = client(&server).capabilities().await.unwrap();
    let momentum: Vec<_> = rows.iter().filter(|r| r.datatype == "momentum").collect();
    assert_eq!(momentum.len(), 1);
    let row = momentum[0];
    assert_eq!(row.venue, "hyperliquid");
    assert_eq!(row.plans.as_deref().unwrap(), PLANS);
    let mut routes = row.rest_routes.clone();
    routes.sort();
    assert_eq!(routes, [SCORES, CURRENT, RETURNS]);
    let mut intervals = row.intervals.clone();
    intervals.sort();
    assert_eq!(intervals, ["1d", "1h"]);
    assert_eq!(row.page_limit, Some(MOMENTUM_MAX_LIMIT as u64));
    // As recorded: the row does not set available_from; the notes name the
    // start.
    assert_eq!(row.available_from, None);
    assert!(row
        .notes
        .as_deref()
        .unwrap()
        .contains("2025-05-24 01:00 UTC"));
}
