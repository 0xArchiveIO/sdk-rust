#![cfg(feature = "websocket")]

use oxarchive::error::Error;
use oxarchive::ws::{
    is_core_l4_replay_channel, is_lighter_live_channel, is_lighter_replay_channel,
    is_lighter_replay_only_channel, is_live_only_l4_channel, is_rh_lighter_channel,
    is_rh_lighter_live_channel, is_rh_lighter_replay_only_channel, OxArchiveWs, ServerMsg,
    WsOptions, LIGHTER_LIVE_CHANNELS, LIGHTER_REPLAY_ONLY_CHANNELS, LIGHTER_SUBSCRIPTION_ERROR,
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

#[test]
fn only_hyperliquid_core_l4_channels_support_replay() {
    for channel in ["l4_diffs", "l4_orders"] {
        assert!(is_core_l4_replay_channel(channel));
        assert!(!is_live_only_l4_channel(channel));
    }
    for channel in [
        "hip3_l4_diffs",
        "hip3_l4_orders",
        "hip4_l4_diffs",
        "hip4_l4_orders",
        "spot_l4_diffs",
        "spot_l4_orders",
    ] {
        assert!(!is_core_l4_replay_channel(channel));
        assert!(is_live_only_l4_channel(channel));
    }
}

#[tokio::test]
async fn core_l4_replay_remains_allowed() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in ["l4_diffs", "l4_orders"] {
        ws.replay(channel, "BTC", 1, Some(2), None)
            .await
            .expect("core Hyperliquid L4 replay must remain allowed");
    }
}

#[tokio::test]
async fn non_core_l4_replay_is_rejected_before_send() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for channel in [
        "hip3_l4_diffs",
        "hip3_l4_orders",
        "hip4_l4_diffs",
        "hip4_l4_orders",
        "spot_l4_diffs",
        "spot_l4_orders",
    ] {
        let error = ws
            .replay(channel, "BTC", 1, Some(2), None)
            .await
            .expect_err("non-core L4 replay must be rejected");
        match error {
            Error::InvalidParam(message) => {
                assert!(
                    message.contains("live-only"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected invalid parameter error, got {other:?}"),
        }
    }
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
async fn full_depth_orderbook_channels_subscribe_and_replay() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));
    for (channel, symbol) in [
        ("orderbook_full", "BTC"),
        ("hip3_orderbook_full", "km:US500"),
    ] {
        ws.subscribe(channel, Some(symbol))
            .await
            .expect("full-depth live subscriptions must be allowed");
        ws.replay(channel, symbol, 1, Some(2), Some(10.0))
            .await
            .expect("full-depth replay must be allowed");
    }
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
