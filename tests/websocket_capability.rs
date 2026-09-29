#![cfg(feature = "websocket")]

use oxarchive::error::Error;
use oxarchive::ws::{
    is_core_l4_replay_channel, is_full_depth_channel, is_l4_channel, is_lighter_live_channel,
    is_lighter_replay_channel, is_lighter_replay_only_channel, is_rh_lighter_channel,
    is_rh_lighter_live_channel, is_rh_lighter_replay_only_channel, is_single_channel_replay,
    OxArchiveWs, ServerMsg, WsOptions, FULL_DEPTH_CHANNELS, L4_REPLAY_CHANNELS,
    LIGHTER_LIVE_CHANNELS, LIGHTER_REPLAY_ONLY_CHANNELS, LIGHTER_SUBSCRIPTION_ERROR,
    RH_LIGHTER_LIVE_CHANNELS, RH_LIGHTER_REPLAY_CHANNELS, RH_LIGHTER_REPLAY_ONLY_CHANNELS,
    RH_LIGHTER_SUBSCRIPTION_ERROR,
};
use oxarchive::LighterLiveData;

const LIGHTER_CHANNELS: [&str; 6] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_candles",
    "lighter_open_interest",
    "lighter_funding",
    "lighter_l3_orderbook",
];

const LIGHTER_LIVE: [&str; 4] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_open_interest",
    "lighter_funding",
];

const LIGHTER_REPLAY_ONLY: [&str; 2] = ["lighter_candles", "lighter_l3_orderbook"];

#[test]
fn every_lighter_channel_supports_replay() {
    for channel in LIGHTER_CHANNELS {
        assert!(is_lighter_replay_channel(channel));
    }
    assert!(!is_lighter_replay_channel("orderbook"));
    assert!(!is_lighter_replay_channel("hip3_orderbook"));
}

#[test]
fn four_lighter_channels_are_live_and_two_are_replay_only() {
    assert_eq!(LIGHTER_LIVE_CHANNELS, LIGHTER_LIVE);
    assert_eq!(LIGHTER_REPLAY_ONLY_CHANNELS, LIGHTER_REPLAY_ONLY);
    for channel in LIGHTER_LIVE {
        assert!(is_lighter_live_channel(channel));
        assert!(!is_lighter_replay_only_channel(channel));
    }
    for channel in LIGHTER_REPLAY_ONLY {
        assert!(!is_lighter_live_channel(channel));
        assert!(is_lighter_replay_only_channel(channel));
    }
    for channel in ["orderbook", "trades", "hip3_orderbook", "spot_orderbook"] {
        assert!(!is_lighter_live_channel(channel));
        assert!(!is_lighter_replay_only_channel(channel));
    }
}

#[tokio::test]
async fn lighter_live_subscriptions_are_allowed() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in LIGHTER_LIVE {
        ws.subscribe(channel, Some("BTC"))
            .await
            .expect("Lighter live subscription must be allowed");
    }
}

#[tokio::test]
async fn replay_only_lighter_subscriptions_fail_before_send() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));

    for channel in LIGHTER_REPLAY_ONLY {
        let error = ws
            .subscribe(channel, Some("BTC"))
            .await
            .expect_err("replay-only Lighter subscription must be rejected");
        match error {
            Error::InvalidParam(message) => {
                assert_eq!(message, LIGHTER_SUBSCRIPTION_ERROR);
                assert_eq!(
                    message,
                    "lighter_candles and lighter_l3_orderbook support replay, not live subscriptions. Use REST for current data or a replay request for stored history."
                );
            }
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn lighter_orderbook_interval_accepts_100_through_5000() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for interval_ms in [100, 250, 1000, 5000] {
        ws.subscribe_with_interval("lighter_orderbook", "BTC", interval_ms)
            .await
            .expect("in-range lighter_orderbook interval must be allowed");
    }
}

#[tokio::test]
async fn out_of_range_interval_is_rejected_before_send() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for interval_ms in [0, 50, 99, 5001] {
        let error = ws
            .subscribe_with_interval("lighter_orderbook", "BTC", interval_ms)
            .await
            .expect_err("out-of-range interval must be rejected");
        match error {
            Error::InvalidParam(message) => assert_eq!(
                message,
                format!(
                    "interval_ms must be between 100 and 5000 for lighter_orderbook (got {interval_ms}). Leave it out for one book a second."
                )
            ),
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn interval_is_rejected_on_other_channels() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in [
        "lighter_trades",
        "lighter_open_interest",
        "lighter_funding",
        "lighter_candles",
        "orderbook",
    ] {
        let error = ws
            .subscribe_with_interval(channel, "BTC", 250)
            .await
            .expect_err("interval_ms must be rejected off lighter_orderbook");
        match error {
            Error::InvalidParam(message) => {
                assert_eq!(
                    message,
                    "interval_ms is only supported on lighter_orderbook."
                )
            }
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn lighter_channels_remain_allowed_for_replay() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));

    for channel in LIGHTER_CHANNELS {
        ws.replay(channel, "BTC", 1, Some(2), Some(1.0))
            .await
            .expect("Lighter replay must remain allowed");
    }
}

#[tokio::test]
async fn hyperliquid_live_subscription_remains_allowed() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    ws.subscribe("orderbook", Some("BTC"))
        .await
        .expect("Hyperliquid live subscription must remain allowed");
}

const EVERY_L4_CHANNEL: [&str; 8] = [
    "l4_diffs",
    "l4_orders",
    "hip3_l4_diffs",
    "hip3_l4_orders",
    "spot_l4_diffs",
    "spot_l4_orders",
    "hip4_l4_diffs",
    "hip4_l4_orders",
];

#[test]
fn every_l4_channel_replays() {
    assert_eq!(L4_REPLAY_CHANNELS, EVERY_L4_CHANNEL);
    for channel in EVERY_L4_CHANNEL {
        assert!(is_l4_channel(channel));
        assert!(is_single_channel_replay(channel));
    }
    for channel in ["l4_diffs", "l4_orders"] {
        assert!(is_core_l4_replay_channel(channel));
    }
    assert!(!is_core_l4_replay_channel("hip3_l4_diffs"));
    assert!(!is_l4_channel("orderbook"));
    assert!(!is_single_channel_replay("trades"));
}

#[test]
#[allow(deprecated)]
fn the_deprecated_live_only_l4_helpers_report_no_live_only_channel() {
    use oxarchive::ws::{is_live_only_l4_channel, LIVE_ONLY_L4_CHANNELS};
    assert_eq!(LIVE_ONLY_L4_CHANNELS.len(), 6);
    for channel in EVERY_L4_CHANNEL {
        assert!(!is_live_only_l4_channel(channel));
    }
}

#[tokio::test]
async fn every_l4_channel_is_sent_for_replay() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for (channel, symbol) in [
        ("l4_diffs", "BTC"),
        ("l4_orders", "BTC"),
        ("hip3_l4_diffs", "xyz:XYZ100"),
        ("hip3_l4_orders", "xyz:XYZ100"),
        ("spot_l4_diffs", "HYPE-USDC"),
        ("spot_l4_orders", "HYPE-USDC"),
        ("hip4_l4_diffs", "#0"),
        ("hip4_l4_orders", "#0"),
    ] {
        ws.replay(channel, symbol, 1, Some(2), None)
            .await
            .unwrap_or_else(|e| panic!("{channel} replay must be sent, got {e:?}"));
    }
}

#[tokio::test]
async fn l4_replay_is_single_channel() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in EVERY_L4_CHANNEL {
        match ws
            .replay_multi(&["trades", channel], "BTC", 1, Some(2), None)
            .await
        {
            Err(Error::InvalidParam(message)) => {
                assert!(message.contains(channel), "unexpected message: {message}");
                assert!(
                    message.contains("single-channel"),
                    "unexpected message: {message}"
                );
            }
            other => {
                panic!("a multi-channel replay with {channel} must be rejected, got {other:?}")
            }
        }
    }
    ws.replay_multi(&["orderbook", "trades"], "BTC", 1, Some(2), None)
        .await
        .expect("a standard multi-channel replay must be sent");
}

const RH_LIGHTER_CHANNELS: [&str; 5] = [
    "rh_lighter_orderbook",
    "rh_lighter_trades",
    "rh_lighter_candles",
    "rh_lighter_open_interest",
    "rh_lighter_funding",
];

const RH_LIGHTER_LIVE: [&str; 4] = [
    "rh_lighter_orderbook",
    "rh_lighter_trades",
    "rh_lighter_open_interest",
    "rh_lighter_funding",
];

#[test]
fn rh_lighter_channels_are_their_own_family_live_except_candles() {
    assert_eq!(RH_LIGHTER_REPLAY_CHANNELS, RH_LIGHTER_CHANNELS);
    assert_eq!(RH_LIGHTER_LIVE_CHANNELS, RH_LIGHTER_LIVE);
    assert_eq!(RH_LIGHTER_REPLAY_ONLY_CHANNELS, ["rh_lighter_candles"]);
    for channel in RH_LIGHTER_CHANNELS {
        assert!(is_rh_lighter_channel(channel));
        // Robinhood Chain channels are not mainnet Lighter channels.
        assert!(!is_lighter_replay_channel(channel));
        assert!(!is_lighter_live_channel(channel));
    }
    for channel in RH_LIGHTER_LIVE {
        assert!(is_rh_lighter_live_channel(channel));
        assert!(!is_rh_lighter_replay_only_channel(channel));
    }
    assert!(is_rh_lighter_replay_only_channel("rh_lighter_candles"));
    assert!(!is_rh_lighter_live_channel("rh_lighter_candles"));
    assert!(!is_rh_lighter_channel("rh_lighter_l3_orderbook"));
    for channel in LIGHTER_CHANNELS {
        assert!(!is_rh_lighter_channel(channel));
    }
}

#[tokio::test]
async fn rh_lighter_live_subscriptions_are_allowed_and_candles_are_rejected() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in RH_LIGHTER_LIVE {
        ws.subscribe(channel, Some("BTC"))
            .await
            .expect("Robinhood Chain live subscription must be allowed");
    }
    match ws.subscribe("rh_lighter_candles", Some("BTC")).await {
        Err(Error::InvalidParam(message)) => {
            assert_eq!(message, RH_LIGHTER_SUBSCRIPTION_ERROR);
            assert_eq!(
                message,
                "rh_lighter_candles supports replay, not live subscriptions. Use REST for current data or a replay request for stored history."
            );
        }
        other => panic!("expected invalid parameter error, got {other:?}"),
    }
}

#[tokio::test]
async fn rh_lighter_orderbook_takes_the_same_interval_range() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for interval_ms in [100, 500, 1000, 5000] {
        ws.subscribe_with_interval("rh_lighter_orderbook", "BTC", interval_ms)
            .await
            .expect("in-range rh_lighter_orderbook interval must be allowed");
    }
    for interval_ms in [99, 5001] {
        match ws
            .subscribe_with_interval("rh_lighter_orderbook", "BTC", interval_ms)
            .await
        {
            Err(Error::InvalidParam(message)) => assert_eq!(
                message,
                format!(
                    "interval_ms must be between 100 and 5000 for rh_lighter_orderbook (got {interval_ms}). Leave it out for one book a second."
                )
            ),
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
    for channel in [
        "rh_lighter_trades",
        "rh_lighter_open_interest",
        "rh_lighter_funding",
        "rh_lighter_candles",
    ] {
        match ws.subscribe_with_interval(channel, "BTC", 250).await {
            Err(Error::InvalidParam(message)) => {
                assert_eq!(
                    message,
                    "interval_ms is only supported on rh_lighter_orderbook."
                )
            }
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn rh_lighter_channels_replay() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in RH_LIGHTER_CHANNELS {
        ws.replay(channel, "BTC", 1, Some(2), Some(1.0))
            .await
            .expect("Robinhood Chain replay must be allowed");
    }
}

#[test]
fn rh_lighter_live_frames_decode_with_the_mainnet_shapes() {
    let book = r#"{"type":"data","channel":"rh_lighter_orderbook","coin":"BTC","symbol":"BTC","data":{"coin":"BTC","time":1790294171459,"levels":[[{"px":"108300.1","sz":"0.012","n":1}],[{"px":"108300.9","sz":"0.4","n":1}]]}}"#;
    let msg: ServerMsg = serde_json::from_str(book).unwrap();
    match msg.lighter_live_data() {
        Some(Ok(LighterLiveData::OrderBook(book))) => {
            assert_eq!(book.bids()[0].px, "108300.1");
            assert_eq!(book.asks()[0].sz, "0.4");
        }
        other => panic!("expected a decoded rh_lighter_orderbook payload, got {other:?}"),
    }

    let trades = r#"{"type":"data","channel":"rh_lighter_trades","coin":"AAPL-USDG","symbol":"AAPL-USDG","data":[{"coin":"AAPL-USDG","side":"A","px":"231.5","sz":"2","time":1790294182211,"hash":"00ab","tid":77,"oid":1,"crossed":false,"dir":null,"fee":null,"fee_token":null,"closed_pnl":null,"start_position":"5","users":["1001"]},{"coin":"AAPL-USDG","side":"B","px":"231.5","sz":"2","time":1790294182211,"hash":"00ab","tid":77,"oid":2,"crossed":true,"dir":null,"fee":null,"fee_token":null,"closed_pnl":null,"start_position":"0","users":["1002"]}]}"#;
    let msg: ServerMsg = serde_json::from_str(trades).unwrap();
    match msg.lighter_live_data() {
        Some(Ok(LighterLiveData::Trades(fills))) => {
            assert_eq!(fills.len(), 2);
            assert_eq!(fills[0].tid, fills[1].tid);
        }
        other => panic!("expected a decoded rh_lighter_trades payload, got {other:?}"),
    }

    let stats = r#"{"coin":"BTC","ctx":{"openInterest":"55.1","funding":"0.00001","premium":null,"markPx":"108300.5","oraclePx":null,"midPx":null,"dayNtlVlm":null,"dayBaseVlm":null,"prevDayPx":null,"impactPxs":null}}"#;
    for (channel, want_funding) in [
        ("rh_lighter_open_interest", false),
        ("rh_lighter_funding", true),
    ] {
        let frame = format!(
            r#"{{"type":"data","channel":"{channel}","coin":"BTC","symbol":"BTC","data":{stats}}}"#
        );
        let msg: ServerMsg = serde_json::from_str(&frame).unwrap();
        match (msg.lighter_live_data(), want_funding) {
            (Some(Ok(LighterLiveData::Funding(s))), true)
            | (Some(Ok(LighterLiveData::OpenInterest(s))), false) => {
                assert_eq!(s.ctx.open_interest.as_deref(), Some("55.1"));
            }
            (other, _) => panic!("unexpected decode for {channel}: {other:?}"),
        }
    }

    // Replay-only and unknown channels have no live payload.
    let candles =
        r#"{"type":"data","channel":"rh_lighter_candles","coin":"BTC","symbol":"BTC","data":{}}"#;
    let msg: ServerMsg = serde_json::from_str(candles).unwrap();
    assert!(msg.lighter_live_data().is_none());
}

#[tokio::test]
async fn full_depth_orderbook_channels_stream_live_and_replay_on_their_own() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    assert_eq!(
        FULL_DEPTH_CHANNELS,
        ["orderbook_full", "hip3_orderbook_full"]
    );
    for (channel, symbol) in [
        ("orderbook_full", "BTC"),
        ("hip3_orderbook_full", "km:US500"),
    ] {
        assert!(is_full_depth_channel(channel));
        assert!(is_single_channel_replay(channel));
        ws.subscribe(channel, Some(symbol))
            .await
            .expect("full-depth live subscriptions must be allowed");
        ws.replay(channel, symbol, 1, Some(2), None)
            .await
            .expect("full-depth replay must be sent");
        match ws
            .replay_multi(&["orderbook", channel], symbol, 1, Some(2), None)
            .await
        {
            Err(Error::InvalidParam(message)) => {
                assert!(message.contains(channel), "unexpected message: {message}");
            }
            other => panic!(
                "a multi-channel replay with a full-depth channel must be rejected, got {other:?}"
            ),
        }
    }
    assert!(!is_full_depth_channel("orderbook"));
    assert!(!is_full_depth_channel("hip3_orderbook"));
}

#[test]
fn full_depth_orderbook_frames_decode() {
    let snapshot = r#"{"type":"l4_snapshot","channel":"orderbook_full","coin":"BTC","symbol":"BTC","last_block_number":1164898902,"timestamp":1790649697779,"data":{"bids":[{"px":108300.0,"sz":1.5,"n":3}],"asks":[{"px":108301.0,"sz":0.2,"n":1}],"bid_count":1,"ask_count":1,"mid_price":108300.5}}"#;
    match serde_json::from_str::<ServerMsg>(snapshot).unwrap() {
        ServerMsg::L4Snapshot {
            channel,
            last_block_number,
            data,
            ..
        } => {
            assert_eq!(channel, "orderbook_full");
            assert_eq!(last_block_number, 1164898902);
            assert_eq!(data["bids"][0]["n"], 3);
        }
        other => panic!("expected an l4_snapshot frame, got {other:?}"),
    }

    let batch = r#"{"type":"l4_batch","channel":"hip3_orderbook_full","coin":"km:US500","symbol":"km:US500","data":[{"side":"B","px":749.6,"sz":0.0,"n":0,"bn":1164898903},{"side":"A","px":749.7,"sz":12.0,"n":2,"bn":1164898903}]}"#;
    match serde_json::from_str::<ServerMsg>(batch).unwrap() {
        ServerMsg::L4Batch {
            channel,
            coin,
            data,
            ..
        } => {
            assert_eq!(channel, "hip3_orderbook_full");
            assert_eq!(coin, "km:US500");
            assert_eq!(data.len(), 2);
            assert_eq!(data[0]["sz"], 0.0);
        }
        other => panic!("expected an l4_batch frame, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// API contract 2026-10: error codes, Lighter replay shapes, replay matrix
// ---------------------------------------------------------------------------

#[test]
fn error_messages_carry_the_error_code() {
    use oxarchive::ErrorCode;
    for (code, expected) in [
        ("slow_consumer", ErrorCode::SlowConsumer),
        ("endpoint_unsupported", ErrorCode::EndpointUnsupported),
        ("unsupported_for_venue", ErrorCode::UnsupportedForVenue),
        ("invalid_parameter", ErrorCode::InvalidParameter),
        ("conflict", ErrorCode::Conflict),
        (
            "a_future_code",
            ErrorCode::Other("a_future_code".to_string()),
        ),
    ] {
        let frame = format!(r#"{{"type":"error","message":"m","error_code":"{code}"}}"#);
        let msg: ServerMsg = serde_json::from_str(&frame).unwrap();
        assert_eq!(msg.error_code(), Some(&expected));
        match msg {
            ServerMsg::Error {
                message,
                error_code,
            } => {
                assert_eq!(message, "m");
                assert_eq!(error_code, Some(expected));
            }
            other => panic!("expected an error message, got {other:?}"),
        }
    }
    // A server that sends no code still parses.
    let bare: ServerMsg = serde_json::from_str(r#"{"type":"error","message":"m"}"#).unwrap();
    assert_eq!(bare.error_code(), None);
    let pong: ServerMsg = serde_json::from_str(r#"{"type":"pong"}"#).unwrap();
    assert_eq!(pong.error_code(), None);
}

#[test]
fn lighter_replay_rows_decode_with_the_live_shapes() {
    let trade = r#"{"type":"historical_data","channel":"lighter_trades","coin":"BTC","symbol":"BTC","timestamp":1790521266168,"data":[{"closed_pnl":"-0.061239","coin":"BTC","crossed":true,"dir":null,"fee":"0","fee_token":null,"hash":"5f66","oid":562953432224725,"px":"84518","side":"A","start_position":"0.01183","sz":"0.00411","tid":32228458048,"time":1790521266168,"users":["736647"]}]}"#;
    let msg: ServerMsg = serde_json::from_str(trade).unwrap();
    let Some(Ok(LighterLiveData::Trades(fills))) = msg.lighter_live_data() else {
        panic!("expected a replayed trade leg, got {msg:?}");
    };
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].tid, 32228458048);
    assert_eq!(fills[0].closed_pnl.as_deref(), Some("-0.061239"));
    assert_eq!(fills[0].fee.as_deref(), Some("0"));

    let snapshot = r#"{"type":"replay_snapshot","channel":"rh_lighter_open_interest","coin":"BTC","symbol":"BTC","timestamp":1790521260650,"data":{"coin":"BTC","ctx":{"dayBaseVlm":null,"dayNtlVlm":null,"funding":null,"impactPxs":null,"markPx":"84511.4","midPx":null,"openInterest":"172850198.76447","oraclePx":"84552.7","premium":null,"prevDayPx":null}}}"#;
    let msg: ServerMsg = serde_json::from_str(snapshot).unwrap();
    let Some(Ok(LighterLiveData::OpenInterest(stats))) = msg.lighter_live_data() else {
        panic!("expected replayed open interest, got {msg:?}");
    };
    assert_eq!(stats.ctx.open_interest.as_deref(), Some("172850198.76447"));
    assert_eq!(stats.ctx.funding, None);

    let book = r#"{"type":"historical_data","channel":"rh_lighter_orderbook","coin":"BTC","symbol":"BTC","timestamp":1790522085209,"data":{"coin":"BTC","levels":[[{"n":1,"px":"84465.1","sz":"1.3517"}],[]],"time":1790522085209}}"#;
    let msg: ServerMsg = serde_json::from_str(book).unwrap();
    assert!(matches!(
        msg.lighter_live_data(),
        Some(Ok(LighterLiveData::OrderBook(_)))
    ));

    // Candles have no live shape and are not decoded.
    let candles = r#"{"type":"historical_data","channel":"lighter_candles","coin":"BTC","symbol":"BTC","timestamp":1,"data":{"open":"1"}}"#;
    let msg: ServerMsg = serde_json::from_str(candles).unwrap();
    assert!(msg.lighter_live_data().is_none());
}

/// Channel rows of `GET /v1/capabilities` as served on 2026-09-29.
const CAPABILITY_CHANNELS: &str = r#"[
{"venue":"hyperliquid","datatype":"l2_orderbook","ws_channels":["orderbook"],"live":true,"replay":true,"available_from":"2023-04-15T00:00:00.000Z"},
{"venue":"hyperliquid","datatype":"l2_full_depth","ws_channels":["orderbook_full"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hyperliquid","datatype":"l4_diffs","ws_channels":["l4_diffs"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hyperliquid","datatype":"l4_orders","ws_channels":["l4_orders"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hyperliquid","datatype":"trades","ws_channels":["trades"],"live":true,"replay":true,"available_from":"2023-04-15T03:31:00.000Z"},
{"venue":"hyperliquid","datatype":"candles","ws_channels":["candles"],"live":false,"replay":true,"available_from":"2025-03-01T00:00:00.000Z"},
{"venue":"hyperliquid","datatype":"funding","ws_channels":["funding"],"live":true,"replay":true,"available_from":"2023-05-20T02:50:00.000Z"},
{"venue":"hyperliquid","datatype":"oi","ws_channels":["open_interest"],"live":true,"replay":true,"available_from":"2023-05-20T02:50:00.000Z"},
{"venue":"hyperliquid","datatype":"liquidations","ws_channels":["liquidations"],"live":true,"replay":true,"available_from":"2025-12-22T00:00:00.000Z"},
{"venue":"hyperliquid","datatype":"ticker","ws_channels":["ticker","all_tickers"],"live":true,"replay":false,"available_from":null},
{"venue":"hip3","datatype":"l2_orderbook","ws_channels":["hip3_orderbook"],"live":true,"replay":true,"available_from":"2026-02-16T16:57:00.000Z"},
{"venue":"hip3","datatype":"l2_full_depth","ws_channels":["hip3_orderbook_full"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hip3","datatype":"l4_diffs","ws_channels":["hip3_l4_diffs"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hip3","datatype":"l4_orders","ws_channels":["hip3_l4_orders"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"hip3","datatype":"trades","ws_channels":["hip3_trades"],"live":true,"replay":true,"available_from":"2025-10-13T12:24:00.000Z"},
{"venue":"hip3","datatype":"candles","ws_channels":["hip3_candles"],"live":false,"replay":true,"available_from":"2025-12-22T00:00:00.000Z"},
{"venue":"hip3","datatype":"funding","ws_channels":["hip3_funding"],"live":true,"replay":true,"available_from":"2026-02-16T17:03:00.000Z"},
{"venue":"hip3","datatype":"oi","ws_channels":["hip3_open_interest"],"live":true,"replay":true,"available_from":"2026-02-16T17:03:00.000Z"},
{"venue":"hip3","datatype":"liquidations","ws_channels":["hip3_liquidations"],"live":true,"replay":true,"available_from":"2025-12-22T00:00:00.000Z"},
{"venue":"hip4","datatype":"l2_orderbook","ws_channels":["hip4_orderbook"],"live":true,"replay":true,"available_from":"2026-05-02T16:51:00.000Z"},
{"venue":"hip4","datatype":"l4_diffs","ws_channels":["hip4_l4_diffs"],"live":true,"replay":true,"available_from":"2026-05-02T07:47:00.000Z"},
{"venue":"hip4","datatype":"l4_orders","ws_channels":["hip4_l4_orders"],"live":true,"replay":true,"available_from":"2026-05-02T07:47:00.000Z"},
{"venue":"hip4","datatype":"trades","ws_channels":["hip4_trades"],"live":true,"replay":true,"available_from":"2026-05-02T08:00:00.000Z"},
{"venue":"hip4","datatype":"oi","ws_channels":["hip4_open_interest"],"live":true,"replay":true,"available_from":"2026-05-02T16:51:00.000Z"},
{"venue":"spot","datatype":"l2_orderbook","ws_channels":["spot_orderbook"],"live":true,"replay":false,"available_from":"2026-05-05T19:56:00.000Z"},
{"venue":"spot","datatype":"l4_diffs","ws_channels":["spot_l4_diffs"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"spot","datatype":"l4_orders","ws_channels":["spot_l4_orders"],"live":true,"replay":true,"available_from":"2026-03-11T01:03:00.000Z"},
{"venue":"spot","datatype":"trades","ws_channels":["spot_trades"],"live":true,"replay":false,"available_from":"2025-03-22T10:50:22.000Z"},
{"venue":"spot","datatype":"twap","ws_channels":["spot_twap"],"live":true,"replay":false,"available_from":null},
{"venue":"lighter","datatype":"l2_orderbook","ws_channels":["lighter_orderbook"],"live":true,"replay":true,"available_from":"2026-01-29T02:13:00.000Z"},
{"venue":"lighter","datatype":"l3_orderbook","ws_channels":["lighter_l3_orderbook"],"live":false,"replay":true,"available_from":"2026-03-05T03:33:00.000Z"},
{"venue":"lighter","datatype":"trades","ws_channels":["lighter_trades"],"live":true,"replay":true,"available_from":"2025-01-17T08:43:00.000Z"},
{"venue":"lighter","datatype":"candles","ws_channels":["lighter_candles"],"live":false,"replay":true,"available_from":"2025-08-01T00:00:00.000Z"},
{"venue":"lighter","datatype":"funding","ws_channels":["lighter_funding"],"live":true,"replay":true,"available_from":"2025-08-25T15:28:00.000Z"},
{"venue":"lighter","datatype":"oi","ws_channels":["lighter_open_interest"],"live":true,"replay":true,"available_from":"2025-08-25T15:28:00.000Z"},
{"venue":"rh-lighter","datatype":"l2_orderbook","ws_channels":["rh_lighter_orderbook"],"live":true,"replay":true,"available_from":"2026-08-22T18:43:00.000Z"},
{"venue":"rh-lighter","datatype":"trades","ws_channels":["rh_lighter_trades"],"live":true,"replay":true,"available_from":"2026-06-26T20:10:26.000Z"},
{"venue":"rh-lighter","datatype":"candles","ws_channels":["rh_lighter_candles"],"live":false,"replay":true,"available_from":"2026-06-26T20:10:00.000Z"},
{"venue":"rh-lighter","datatype":"funding","ws_channels":["rh_lighter_funding"],"live":true,"replay":true,"available_from":"2026-08-22T18:43:00.000Z"},
{"venue":"rh-lighter","datatype":"oi","ws_channels":["rh_lighter_open_interest"],"live":true,"replay":true,"available_from":"2026-08-22T18:43:00.000Z"}
]"#;

#[derive(serde::Deserialize)]
struct ChannelRow {
    ws_channels: Vec<String>,
    live: bool,
    replay: bool,
}

#[tokio::test]
async fn client_side_refusals_follow_the_capabilities() {
    let rows: Vec<ChannelRow> = serde_json::from_str(CAPABILITY_CHANNELS).unwrap();
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    let mut replayable = 0;
    for row in &rows {
        for channel in &row.ws_channels {
            let replay = ws.replay(channel, "BTC", 1, Some(2), None).await;
            let multi = ws
                .replay_multi(&[channel.as_str()], "BTC", 1, Some(2), None)
                .await;
            let live = ws.subscribe(channel, Some("BTC")).await;
            if row.replay {
                replayable += 1;
                // Nothing the API replays is refused before sending; only
                // the single-channel rule applies to multi-channel replay.
                assert!(replay.is_ok(), "{channel} replays but was refused");
                assert_eq!(
                    multi.is_err(),
                    is_single_channel_replay(channel),
                    "{channel}"
                );
            }
            // A live subscription is refused only where the API has none.
            if live.is_err() {
                assert!(!row.live, "{channel} streams live but was refused");
            }
        }
    }
    assert!(replayable >= 30);
}
