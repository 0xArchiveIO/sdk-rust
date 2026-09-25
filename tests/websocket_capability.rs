#![cfg(feature = "websocket")]

use oxarchive::error::Error;
use oxarchive::ws::{
    is_core_l4_replay_channel, is_lighter_live_channel, is_lighter_replay_channel,
    is_lighter_replay_only_channel, is_live_only_l4_channel, OxArchiveWs, WsOptions,
    LIGHTER_LIVE_CHANNELS, LIGHTER_REPLAY_ONLY_CHANNELS, LIGHTER_SUBSCRIPTION_ERROR,
};

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
