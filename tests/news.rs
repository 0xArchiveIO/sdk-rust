//! News stories on Hyperliquid, HIP-3 and Lighter.
//!
//! The bodies in `tests/fixtures/news/` have the shape the backend builds
//! (migration 338's `news_story_json` and the reaction job), with the
//! regression values of the news build (BTC 2026-11-04 02:57 UTC, xyz:CL
//! 18:07 UTC). They are not recordings: the routes are dark until
//! `NEWS_ROUTES_ENABLED` is set, so replace them with recordings after
//! launch. The mocks answer a fixture's body only to its exact request.

use std::collections::BTreeMap;

use oxarchive::resources::news::NewsParams;
use oxarchive::{Error, ErrorCode, OxArchive};
use serde_json::Value;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CURSOR: &str = "1793760070100_c3d90be1a7746f20";

fn client(server: &MockServer) -> OxArchive {
    OxArchive::builder("test-key")
        .base_url(server.uri())
        .build()
        .unwrap()
}

fn recorded(name: &str) -> Value {
    let file = format!(
        "{}/tests/fixtures/news/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap()
}

fn reply(fixture: &Value) -> ResponseTemplate {
    let status = fixture["status"].as_u64().unwrap() as u16;
    ResponseTemplate::new(status).set_body_json(fixture["body"].clone())
}

fn wire(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn recorded_query(name: &str) -> BTreeMap<String, String> {
    recorded(name)["request"]["query"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), wire(v)))
        .collect()
}

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

async fn mount_anywhere(server: &MockServer, name: &str, on: &str) {
    Mock::given(method("GET"))
        .and(path(on))
        .respond_with(reply(&recorded(name)))
        .mount(server)
        .await;
}

fn params(limit: i64, min_weight: u8, symbols: &[&str]) -> NewsParams {
    NewsParams {
        symbols: symbols.iter().map(|s| s.to_string()).collect(),
        min_weight: Some(min_weight),
        limit: Some(limit),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_page_parses_stories_with_their_reactions() {
    let server = MockServer::start().await;
    mount(&server, "news_hl_page1").await;

    let page = client(&server)
        .hyperliquid
        .news
        .list(params(2, 2, &["BTC", "BNB"]))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(sent(&requests[0]), recorded_query("news_hl_page1"));
    assert_eq!(page.next_cursor.as_deref(), Some(CURSOR));
    assert!(page.has_more);
    let liq = &page.data[0];
    assert_eq!(liq.id, "st_8a41c07e5d2b9f13");
    assert_eq!((liq.origin.as_str(), liq.topic.as_str(), liq.weight), ("venue", "liquidation", 3));
    assert_eq!(liq.first_seen_at, "2026-11-04T02:57:00.000Z");
    assert!(liq.url.is_none() && liq.source_line.is_none());
    assert_eq!(liq.markets.len(), 1);
    assert_eq!((liq.markets[0].venue.as_str(), liq.markets[0].symbol.as_str()), ("hyperliquid", "BTC"));

    let reaction = liq.reaction.as_ref().unwrap();
    assert_eq!(reaction.status, "final");
    assert_eq!(reaction.from, liq.first_seen_at);
    let five = reaction.windows.five_minutes.as_ref().unwrap();
    assert_eq!(five.end, "2026-11-04T03:02:00.000Z");
    let price = five.price.as_ref().unwrap();
    assert_eq!((price.at, price.end, price.move_pct), (Some(76554.0), Some(75983.0), Some(-0.75)));
    let liqs = five.liquidations_usd.as_ref().unwrap();
    assert_eq!((liqs.long, liqs.short), (Some(16_650_000.0), Some(0.0)));
    assert_eq!((five.oi_change_pct, five.volume_vs_prior), (Some(-0.76), Some(4.4)));
    assert_eq!((five.move_z, five.move_level.as_deref()), (Some(-7.0), Some("large")));
    assert_eq!(five.bid_depth_25bp_usd.as_ref().unwrap().at, Some(4_200_000.0));
    assert_eq!(reaction.others[0].market.symbol, "ETH");
    assert_eq!(reaction.others[0].move_pct_5m, Some(-1.12));
    assert!(reaction.known_at.is_some());
    let one = reaction.windows.one_minute.as_ref().unwrap();
    assert!(one.move_level.is_none() && one.bid_depth_25bp_usd.is_none());

    let tree = &page.data[1];
    assert_eq!(tree.source, "Tree News");
    assert_eq!(tree.source_line.as_deref(), Some("via Tree News"));
    assert_eq!(
        tree.headline,
        "Binance says it has replenished 95% of stolen crypto assets"
    );
    let symbols: Vec<_> = tree.markets.iter().map(|m| m.symbol.as_str()).collect();
    assert_eq!(symbols, ["BNB", "BTC"]);
    assert_eq!(tree.reports.len(), 2);
    assert!(tree.reports[1].published_at.is_none());
}

#[tokio::test]
async fn a_measuring_reaction_has_only_the_windows_that_are_due() {
    let server = MockServer::start().await;
    mount(&server, "news_hl_page1").await;
    let page = client(&server)
        .hyperliquid
        .news
        .list(params(2, 2, &["BTC,BNB"]))
        .await
        .unwrap();

    let reaction = page.data[1].reaction.as_ref().unwrap();
    assert_eq!(reaction.status, "measuring");
    assert!(reaction.windows.five_minutes.is_none());
    let one = reaction.windows.one_minute.as_ref().unwrap();
    assert_eq!(one.oi_change_pct, None, "unmeasured stays None, never 0");
    assert_eq!(one.liquidations_usd.as_ref().unwrap().total, Some(0.0));
}

#[tokio::test]
async fn a_story_without_a_market_has_no_reaction_and_the_last_page_no_cursor() {
    let server = MockServer::start().await;
    mount(&server, "news_hl_page2").await;
    let mut p = params(2, 2, &["BTC", "BNB"]);
    p.cursor = Some(CURSOR.to_string());

    let page = client(&server).hyperliquid.news.list(p).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(sent(&requests[0]), recorded_query("news_hl_page2"));
    assert!(page.data[0].markets.is_empty() && page.data[0].reaction.is_none());
    assert!(page.next_cursor.is_none() && !page.has_more);
}

#[tokio::test]
async fn as_of_is_sent_in_milliseconds() {
    let server = MockServer::start().await;
    mount(&server, "news_hl_as_of").await;
    let page = client(&server)
        .hyperliquid
        .news
        .list(NewsParams {
            symbols: vec!["BTC".into()],
            as_of: Some("2026-11-04T02:58:30Z".into()),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(sent(&requests[0]), recorded_query("news_hl_as_of"));
    let reaction = page.data[0].reaction.as_ref().unwrap();
    assert_eq!(reaction.status, "measuring");
    assert!(reaction.windows.five_minutes.is_none());
}

#[tokio::test]
async fn hip3_uses_its_own_route_and_keeps_the_symbol_as_given() {
    let server = MockServer::start().await;
    mount(&server, "news_hip3_cl").await;
    let page = client(&server)
        .hyperliquid
        .hip3
        .news
        .list(NewsParams {
            symbols: vec!["xyz:CL".into()],
            topic: Some("macro".into()),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v1/hyperliquid/hip3/news");
    assert_eq!(sent(&requests[0]), recorded_query("news_hip3_cl"));
    let story = &page.data[0];
    assert_eq!(story.markets[1].symbol, "xyz:BRENTOIL");
    let five = story.reaction.as_ref().unwrap().windows.five_minutes.as_ref().unwrap();
    let price = five.price.as_ref().unwrap();
    assert_eq!((price.at, price.end, price.move_pct), (Some(88.173), Some(88.9), Some(0.82)));
    assert_eq!((five.oi_change_pct, five.volume_vs_prior, five.move_z), (Some(1.83), Some(8.3), Some(3.6)));
    assert_eq!(five.move_level.as_deref(), Some("large"));
}

#[tokio::test]
async fn lighter_reports_a_liquidation_total_only() {
    let server = MockServer::start().await;
    mount(&server, "news_lighter_total_only").await;
    let page = client(&server)
        .lighter
        .news
        .list(NewsParams {
            symbols: vec!["BTC".into()],
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v1/lighter/news");
    let five = page.data[0].reaction.as_ref().unwrap().windows.five_minutes.as_ref().unwrap();
    let liqs = five.liquidations_usd.as_ref().unwrap();
    assert_eq!((liqs.long, liqs.short, liqs.total), (None, None, Some(1_210_000.0)));
    assert!(five.move_z.is_none() && five.move_level.is_none());
}

#[tokio::test]
async fn the_pager_follows_the_cursor_with_the_window_fixed_once() {
    let server = MockServer::start().await;
    let first = recorded("news_hl_page1");
    let last = recorded("news_hl_page2");
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/news"))
        .and(query_param_is_missing("cursor"))
        .respond_with(reply(&first))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/news"))
        .and(query_param("cursor", CURSOR))
        .respond_with(reply(&last))
        .expect(1)
        .mount(&server)
        .await;

    let before = chrono::Utc::now().timestamp_millis();
    let rows = client(&server)
        .hyperliquid
        .news
        .iterate(params(2, 2, &["BTC", "BNB"]))
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    let after = chrono::Utc::now().timestamp_millis();

    let ids: Vec<_> = rows.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["st_8a41c07e5d2b9f13", "st_c3d90be1a7746f20", "st_19ab0c5d7e3f8a42"]);
    let requests = server.received_requests().await.unwrap();
    let queries: Vec<_> = requests.iter().map(sent).collect();
    let end: i64 = queries[0]["end"].parse().unwrap();
    assert!((before..=after).contains(&end));
    assert_eq!(queries[1]["end"], end.to_string());
    assert!(!queries[0].contains_key("start"), "start stays optional");
    assert_eq!(queries[1]["cursor"], CURSOR);
}

#[tokio::test]
async fn a_repeated_cursor_fails_instead_of_looping() {
    let server = MockServer::start().await;
    mount_anywhere(&server, "news_hl_page1", "/v1/hyperliquid/news").await;
    let mut pages = client(&server)
        .hyperliquid
        .news
        .iterate(params(2, 2, &["BTC", "BNB"]))
        .unwrap();

    assert!(pages.next_page().await.unwrap().is_some());
    assert!(pages.next_page().await.unwrap().is_some());
    let err = pages.next_page().await.unwrap_err();
    assert!(err.to_string().contains("stopping rather than requesting"));
}

#[tokio::test]
async fn a_free_plan_refusal_is_typed_on_every_route() {
    let server = MockServer::start().await;
    let c = client(&server);
    let body = recorded("news_free_forbidden");
    for on in ["/v1/hyperliquid/news", "/v1/hyperliquid/hip3/news", "/v1/lighter/news"] {
        mount_anywhere(&server, "news_free_forbidden", on).await;
    }
    let errors: Vec<Error> = vec![
        c.hyperliquid.news.list(NewsParams::default()).await.unwrap_err(),
        c.hyperliquid.hip3.news.list(NewsParams::default()).await.unwrap_err(),
        c.lighter.news.list(NewsParams::default()).await.unwrap_err(),
    ];
    for err in errors {
        assert_eq!(err.status(), Some(403));
        assert_eq!(err.error_code(), Some(&ErrorCode::Forbidden));
        assert_eq!(err.request_id(), body["body"]["request_id"].as_str());
        let plan = err.plan_required().expect("a plan refusal");
        assert_eq!(plan.datatype.as_deref(), Some("news"));
        assert_eq!(plan.required_plans, ["build", "pro", "scale", "enterprise"]);
        assert_eq!(plan.upgrade_url.as_deref(), Some("https://0xarchive.io/pricing"));
    }
}

#[tokio::test]
async fn a_bad_request_keeps_its_code_and_is_not_a_plan_refusal() {
    let server = MockServer::start().await;
    mount_anywhere(&server, "news_invalid_parameter", "/v1/hyperliquid/news").await;
    let err = client(&server)
        .hyperliquid
        .news
        .list(NewsParams {
            min_weight: Some(3),
            ..Default::default()
        })
        .await
        .unwrap_err();

    assert_eq!(err.status(), Some(400));
    assert_eq!(err.error_code(), Some(&ErrorCode::InvalidParameter));
    assert!(err.plan_required().is_none());
}

#[tokio::test]
async fn symbols_are_trimmed_split_and_joined() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/hyperliquid/news"))
        .respond_with(reply(&recorded("news_hl_page2")))
        .mount(&server)
        .await;
    let news = &client(&server).hyperliquid.news;
    news.list(NewsParams {
        symbols: vec![" BTC".into(), "ETH,SOL ".into()],
        ..Default::default()
    })
    .await
    .unwrap();
    news.list(NewsParams {
        symbols: vec![],
        ..Default::default()
    })
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(sent(&requests[0])["symbols"], "BTC,ETH,SOL");
    assert!(!sent(&requests[1]).contains_key("symbols"));
}

#[tokio::test]
async fn bad_arguments_are_refused_before_sending() {
    let client = OxArchive::builder("test-key")
        .base_url("http://127.0.0.1:9")
        .build()
        .unwrap();
    let many: Vec<String> = (0..21).map(|i| format!("S{i}")).collect();
    let bad = [
        NewsParams { limit: Some(0), ..Default::default() },
        NewsParams { limit: Some(501), ..Default::default() },
        NewsParams { min_weight: Some(0), ..Default::default() },
        NewsParams { min_weight: Some(4), ..Default::default() },
        NewsParams { symbols: many, ..Default::default() },
    ];
    for p in bad {
        let err = client.hyperliquid.news.list(p.clone()).await.unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "{p:?}");
        assert!(matches!(client.lighter.news.iterate(p), Err(Error::InvalidParam(_))));
    }
}

#[test]
fn unknown_fields_are_ignored_and_known_ones_default() {
    let story: oxarchive::NewsStory = serde_json::from_value(serde_json::json!({
        "id": "st_1", "headline": "h", "source": "s", "origin": "outlet", "topic": "newtopic",
        "weight": 1, "first_seen_at": "2026-11-04T00:00:00.000Z", "future_field": 7
    }))
    .unwrap();
    assert!(story.markets.is_empty() && story.reaction.is_none() && story.url.is_none());
}
