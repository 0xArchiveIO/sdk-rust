#![cfg(feature = "websocket")]

use oxarchive::error::Error;
use oxarchive::ws::{
    is_core_l4_replay_channel, is_lighter_replay_channel, is_live_only_l4_channel, OxArchiveWs,
    WsOptions,
};

const LIGHTER_CHANNELS: [&str; 6] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_candles",
    "lighter_open_interest",
    "lighter_funding",
    "lighter_l3_orderbook",
];

#[test]
fn lighter_channels_are_replay_only() {
    for channel in LIGHTER_CHANNELS {
        assert!(is_lighter_replay_channel(channel));
    }
    assert!(!is_lighter_replay_channel("orderbook"));
    assert!(!is_lighter_replay_channel("hip3_orderbook"));
}

#[tokio::test]
async fn lighter_live_subscriptions_fail_before_send() {
    let ws = OxArchiveWs::new(WsOptions::new("test-key"));

    for channel in LIGHTER_CHANNELS {
        let error = ws
            .subscribe(channel, Some("BTC"))
            .await
            .expect_err("Lighter live subscription must be rejected");
        match error {
            Error::InvalidParam(message) => assert_eq!(
                message,
                "Lighter WebSocket channels support replay, not live subscriptions. Use REST for current data or a replay request for stored history."
            ),
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
