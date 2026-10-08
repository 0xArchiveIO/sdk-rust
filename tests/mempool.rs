#![cfg(feature = "websocket")]
//! The `mempool` WebSocket channel: pending Hyperliquid transactions.

use oxarchive::error::Error;
use oxarchive::ws::{
    ClientMsg, OxArchiveWs, ServerMsg, WsOptions, DEFAULT_WS_URL, MEMPOOL_CHANNEL, STREAM_WS_URL,
};
use oxarchive::{ErrorCode, LighterLiveData, MempoolItem, MempoolSignature};
use serde_json::json;

/// A frame shaped as the server sends it: an order on BTC and a transfer that
/// references no market, on the unfiltered stream.
const UNFILTERED_FRAME: &str = r#"{"type":"data","channel":"mempool","coin":null,"symbol":null,"data":[
{"received_at":"2026-10-08T01:57:23.548737209Z","received_at_ms":1791424643548,"symbols":["BTC"],"action":{"type":"order","orders":[{"a":0,"b":true,"p":"83276","s":"0.40011","r":false,"t":{"limit":{"tif":"Alo"}},"c":"0x7849acc2c6c2f6f0fe4bc80ef13d1504"}],"grouping":"na"},"nonce":1791424643400,"vault_address":null,"expires_after_ms":null,"signature":{"r":"0x5afc","s":"0x57e2","v":28}},
{"received_at":"2026-10-08T01:57:23.548737209Z","received_at_ms":1791424643548,"symbols":[],"action":{"type":"usdSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x0000000000000000000000000000000000000001","amount":"10","time":1791424643400},"nonce":1791424643400,"vault_address":null,"expires_after_ms":1791424703400,"signature":{"r":"0x01","s":"0x02","v":27}}
]}"#;

#[test]
fn mempool_is_served_at_the_stream_endpoint_only() {
    assert_eq!(MEMPOOL_CHANNEL, "mempool");
    assert_eq!(STREAM_WS_URL, "wss://stream.0xarchive.io/ws");
    assert_eq!(DEFAULT_WS_URL, "wss://api.0xarchive.io/ws");
    assert_eq!(WsOptions::new("k").ws_url, DEFAULT_WS_URL);
    assert_eq!(
        WsOptions::new("k").ws_url(STREAM_WS_URL).ws_url,
        STREAM_WS_URL
    );
}

#[test]
fn subscribe_without_a_symbol_is_the_unfiltered_stream() {
    let unfiltered = ClientMsg::Subscribe {
        channel: MEMPOOL_CHANNEL.to_string(),
        symbol: None,
    };
    assert_eq!(
        serde_json::to_value(&unfiltered).unwrap(),
        json!({"op": "subscribe", "channel": "mempool", "symbol": null})
    );

    for symbol in ["BTC", "xyz:TSLA", "HYPE-USDC", "#49720"] {
        let filtered = ClientMsg::Subscribe {
            channel: MEMPOOL_CHANNEL.to_string(),
            symbol: Some(symbol.to_string()),
        };
        assert_eq!(
            serde_json::to_value(&filtered).unwrap(),
            json!({"op": "subscribe", "channel": "mempool", "symbol": symbol})
        );
    }

    let unsubscribe = ClientMsg::Unsubscribe {
        channel: MEMPOOL_CHANNEL.to_string(),
        symbol: None,
    };
    assert_eq!(
        serde_json::to_value(&unsubscribe).unwrap(),
        json!({"op": "unsubscribe", "channel": "mempool", "symbol": null})
    );
}

#[tokio::test]
async fn mempool_subscriptions_are_left_to_the_server() {
    // Endpoint and plan are the server's to decide: nothing is refused here.
    let ws = OxArchiveWs::new(WsOptions::new("test-key").ws_url(STREAM_WS_URL));
    ws.subscribe(MEMPOOL_CHANNEL, None).await.unwrap();
    ws.subscribe(MEMPOOL_CHANNEL, Some("BTC")).await.unwrap();
    ws.unsubscribe(MEMPOOL_CHANNEL, Some("BTC")).await.unwrap();
    ws.unsubscribe(MEMPOOL_CHANNEL, None).await.unwrap();
}

#[test]
fn unfiltered_frame_decodes_to_items() {
    let msg: ServerMsg = serde_json::from_str(UNFILTERED_FRAME).unwrap();
    match &msg {
        ServerMsg::Data {
            channel,
            coin,
            symbol,
            ..
        } => {
            assert_eq!(channel, "mempool");
            assert_eq!((coin, symbol), (&None, &None));
        }
        other => panic!("expected a data message, got {other:?}"),
    }
    assert!(msg.lighter_live_data().is_none());

    let items = msg.mempool_items().unwrap().unwrap();
    assert_eq!(items.len(), 2);

    let order = &items[0];
    assert_eq!(
        order.received_at.as_deref(),
        Some("2026-10-08T01:57:23.548737209Z")
    );
    assert_eq!(order.received_at_ms, Some(1791424643548));
    assert_eq!(order.symbols, ["BTC"]);
    assert_eq!(order.action["type"], "order");
    assert_eq!(order.action["orders"][0]["a"], 0);
    assert_eq!(order.action["orders"][0]["p"], "83276");
    assert_eq!(order.nonce, Some(1791424643400));
    assert_eq!(order.vault_address, None);
    assert_eq!(order.expires_after_ms, None);
    assert_eq!(
        order.signature,
        Some(MempoolSignature {
            r: "0x5afc".to_string(),
            s: "0x57e2".to_string(),
            v: 28,
        })
    );

    let transfer = &items[1];
    assert!(transfer.symbols.is_empty());
    assert_eq!(transfer.action["type"], "usdSend");
    assert_eq!(transfer.expires_after_ms, Some(1791424703400));
}

#[test]
fn symbol_frame_carries_the_subscription_symbol() {
    let frame = UNFILTERED_FRAME.replace(
        r#""coin":null,"symbol":null"#,
        r#""coin":"BTC","symbol":"BTC""#,
    );
    let msg: ServerMsg = serde_json::from_str(&frame).unwrap();
    let ServerMsg::Data { coin, symbol, .. } = &msg else {
        panic!("expected a data message, got {msg:?}");
    };
    assert_eq!(coin.as_deref(), Some("BTC"));
    assert_eq!(symbol.as_deref(), Some("BTC"));
    assert_eq!(msg.mempool_items().unwrap().unwrap().len(), 2);
}

#[test]
fn other_messages_are_not_mempool_items() {
    let trades = r#"{"type":"data","channel":"trades","coin":"BTC","symbol":"BTC","data":[]}"#;
    let msg: ServerMsg = serde_json::from_str(trades).unwrap();
    assert!(msg.mempool_items().is_none());

    let pong: ServerMsg = serde_json::from_str(r#"{"type":"pong"}"#).unwrap();
    assert!(pong.mempool_items().is_none());

    assert!(MempoolItem::decode("trades", &json!([])).is_none());
    assert!(LighterLiveData::decode("mempool", &json!([])).is_none());
}

#[test]
fn items_tolerate_nulls_new_fields_and_new_action_types() {
    let data = json!([
        {"received_at": null, "received_at_ms": null, "symbols": [],
         "action": {"type": "someFutureAction", "x": 1}, "nonce": null,
         "vault_address": "0x00000000000000000000000000000000000000aa",
         "expires_after_ms": null, "signature": null, "a_future_field": true},
        {"action": {"type": "noop"}}
    ]);
    let items = MempoolItem::decode(MEMPOOL_CHANNEL, &data)
        .unwrap()
        .unwrap();
    assert_eq!(items[0].received_at, None);
    assert_eq!(items[0].nonce, None);
    assert_eq!(items[0].signature, None);
    assert_eq!(items[0].action["type"], "someFutureAction");
    assert_eq!(
        items[0].vault_address.as_deref(),
        Some("0x00000000000000000000000000000000000000aa")
    );
    assert!(items[1].symbols.is_empty());
    assert_eq!(items[1].action, json!({"type": "noop"}));
}

#[test]
fn a_payload_that_is_not_a_list_of_items_is_an_error() {
    let result = MempoolItem::decode(MEMPOOL_CHANNEL, &json!({"not": "a list"})).unwrap();
    assert!(matches!(result, Err(Error::Deserialize(_))));
}

#[test]
fn subscribe_acks_without_a_symbol_parse() {
    let ack: ServerMsg = serde_json::from_str(
        r#"{"type":"subscribed","channel":"mempool","coin":null,"symbol":null}"#,
    )
    .unwrap();
    match ack {
        ServerMsg::Subscribed {
            channel,
            coin,
            symbol,
        } => {
            assert_eq!(channel, "mempool");
            assert_eq!((coin, symbol), (None, None));
        }
        other => panic!("expected a subscribed message, got {other:?}"),
    }
}

#[test]
fn mempool_refusals_carry_their_error_code() {
    for (frame, expected) in [
        (
            r#"{"type":"error","message":"The mempool channel is included with the Pro, Scale and Enterprise plans. Upgrade at https://0xarchive.io/pricing.","error_code":"forbidden"}"#,
            ErrorCode::Forbidden,
        ),
        (
            r#"{"type":"error","message":"The 'mempool' channel is live only and served on wss://stream.0xarchive.io/ws. Subscribe to it there.","error_code":"endpoint_unsupported"}"#,
            ErrorCode::EndpointUnsupported,
        ),
        (
            r#"{"type":"error","message":"The unfiltered mempool stream is at capacity. Subscribe with a symbol, or try again later.","error_code":"rate_limited"}"#,
            ErrorCode::RateLimited,
        ),
        (
            r#"{"type":"error","message":"The mempool channel is temporarily unavailable. Please try again shortly.","error_code":"upstream_unavailable"}"#,
            ErrorCode::UpstreamUnavailable,
        ),
        (
            r#"{"type":"error","message":"Unknown symbol 'NOPE'.","error_code":"invalid_symbol"}"#,
            ErrorCode::InvalidSymbol,
        ),
    ] {
        let msg: ServerMsg = serde_json::from_str(frame).unwrap();
        assert_eq!(msg.error_code(), Some(&expected), "{frame}");
        assert!(msg.mempool_items().is_none());
    }
}
