//! WebSocket client for supported live subscriptions and historical replay.
//!
//! For large historical downloads, use the S3 Parquet bulk export at
//! <https://www.0xarchive.io/data>. The server no longer supports bulk
//! streaming over WebSocket.
//!
//! Requires the `websocket` feature:
//! ```toml
//! oxarchive = { version = "1.11", features = ["websocket"] }
//! ```

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::error::{Error, Result};
use crate::types::LighterLiveData;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Options for the WebSocket connection.
pub struct WsOptions {
    pub api_key: String,
    pub ws_url: String,
    pub auto_reconnect: bool,
    pub reconnect_delay: Duration,
    pub max_reconnect_attempts: u32,
}

impl WsOptions {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ws_url: "wss://api.0xarchive.io/ws".to_string(),
            auto_reconnect: true,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_attempts: 10,
        }
    }

    pub fn ws_url(mut self, url: impl Into<String>) -> Self {
        self.ws_url = url.into();
        self
    }

    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }
}

/// The six Lighter channels. All of them support bounded historical replay.
pub const LIGHTER_REPLAY_CHANNELS: [&str; 6] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_candles",
    "lighter_open_interest",
    "lighter_funding",
    "lighter_l3_orderbook",
];

/// Lighter channels that also support live subscriptions.
///
/// Live Lighter data is served on `wss://api.0xarchive.io/ws`, the default
/// `ws_url` in [`WsOptions`]. Payloads decode with
/// [`ServerMsg::lighter_live_data`].
pub const LIGHTER_LIVE_CHANNELS: [&str; 4] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_open_interest",
    "lighter_funding",
];

/// Lighter channels that support replay but not live subscriptions.
pub const LIGHTER_REPLAY_ONLY_CHANNELS: [&str; 2] = ["lighter_candles", "lighter_l3_orderbook"];

/// Guidance returned when a live subscription is requested on a replay-only
/// Lighter channel.
pub const LIGHTER_SUBSCRIPTION_ERROR: &str =
    "lighter_candles and lighter_l3_orderbook support replay, not live subscriptions. Use REST for current data or a replay request for stored history.";

/// Return whether a channel is one of the six Lighter channels, all of which
/// support replay. Use [`is_lighter_live_channel`] to check live support.
pub fn is_lighter_replay_channel(channel: &str) -> bool {
    LIGHTER_REPLAY_CHANNELS.contains(&channel)
}

/// Return whether a Lighter channel supports live subscriptions.
pub fn is_lighter_live_channel(channel: &str) -> bool {
    LIGHTER_LIVE_CHANNELS.contains(&channel)
}

/// Return whether a Lighter channel supports replay but not live subscriptions.
pub fn is_lighter_replay_only_channel(channel: &str) -> bool {
    LIGHTER_REPLAY_ONLY_CHANNELS.contains(&channel)
}

/// Default `lighter_orderbook` interval: one book per second.
pub const LIGHTER_ORDERBOOK_DEFAULT_INTERVAL_MS: u32 = 1000;
/// Smallest `interval_ms` accepted on `lighter_orderbook`.
pub const LIGHTER_ORDERBOOK_MIN_INTERVAL_MS: u32 = 100;
/// Largest `interval_ms` accepted on `lighter_orderbook`.
pub const LIGHTER_ORDERBOOK_MAX_INTERVAL_MS: u32 = 5000;

fn validate_subscribe_interval(channel: &str, interval_ms: u32) -> Result<()> {
    if channel != "lighter_orderbook" {
        return Err(Error::InvalidParam(
            "interval_ms is only supported on lighter_orderbook.".to_string(),
        ));
    }
    if !(LIGHTER_ORDERBOOK_MIN_INTERVAL_MS..=LIGHTER_ORDERBOOK_MAX_INTERVAL_MS)
        .contains(&interval_ms)
    {
        return Err(Error::InvalidParam(format!(
            "interval_ms must be between {LIGHTER_ORDERBOOK_MIN_INTERVAL_MS} and {LIGHTER_ORDERBOOK_MAX_INTERVAL_MS} for lighter_orderbook (got {interval_ms}). Leave it out for one book a second."
        )));
    }
    Ok(())
}

/// Hyperliquid core L4 channels with checkpoint-anchored replay support.
pub const CORE_L4_REPLAY_CHANNELS: [&str; 2] = ["l4_diffs", "l4_orders"];

/// L4 channel families that are live-only and must not inherit core replay.
pub const LIVE_ONLY_L4_CHANNELS: [&str; 6] = [
    "hip3_l4_diffs",
    "hip3_l4_orders",
    "hip4_l4_diffs",
    "hip4_l4_orders",
    "spot_l4_diffs",
    "spot_l4_orders",
];

/// Return whether a channel supports the core Hyperliquid L4 replay contract.
pub fn is_core_l4_replay_channel(channel: &str) -> bool {
    CORE_L4_REPLAY_CHANNELS.contains(&channel)
}

/// Return whether an L4 channel is explicitly live-only.
pub fn is_live_only_l4_channel(channel: &str) -> bool {
    LIVE_ONLY_L4_CHANNELS.contains(&channel)
}

const LIVE_ONLY_L4_REPLAY_ERROR: &str =
    "HIP-3, HIP-4, and Spot L4 channels are live-only; replay is supported only for Hyperliquid core l4_diffs and l4_orders.";

fn validate_replay_channel(channel: &str) -> Result<()> {
    if is_live_only_l4_channel(channel) {
        return Err(Error::InvalidParam(LIVE_ONLY_L4_REPLAY_ERROR.to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// A message sent from the client to the server.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ClientMsg {
    #[serde(rename = "subscribe")]
    Subscribe { channel: String, symbol: Option<String> },
    /// A `subscribe` request that sets `interval_ms`. Only `lighter_orderbook`
    /// accepts it, with a value from 100 to 5000.
    #[serde(rename = "subscribe")]
    SubscribeWithInterval {
        channel: String,
        symbol: String,
        interval_ms: u32,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { channel: String, symbol: Option<String> },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "replay")]
    Replay {
        channel: String,
        symbol: String,
        start: i64,
        end: Option<i64>,
        speed: Option<f64>,
    },
    #[serde(rename = "replay")]
    ReplayMulti {
        channels: Vec<String>,
        symbol: String,
        start: i64,
        end: Option<i64>,
        speed: Option<f64>,
    },
    #[serde(rename = "replay.pause")]
    ReplayPause,
    #[serde(rename = "replay.resume")]
    ReplayResume,
    #[serde(rename = "replay.seek")]
    ReplaySeek { timestamp: i64 },
    #[serde(rename = "replay.stop")]
    ReplayStop,
    /// Bulk stream request. The server discontinued bulk streaming and
    /// answers this message with an error; it is kept so existing code
    /// still compiles.
    #[serde(rename = "stream")]
    Stream {
        channel: String,
        symbol: String,
        start: i64,
        end: i64,
        batch_size: Option<usize>,
    },
    /// Bulk stream stop request. The server discontinued bulk streaming and
    /// answers this message with an error; it is kept so existing code
    /// still compiles.
    #[serde(rename = "stream.stop")]
    StreamStop,
}

/// A message received from the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Subscribed {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
    },
    Unsubscribed {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
    },
    Pong,
    /// An error or notice from the server.
    ///
    /// Not every error ends a live Lighter subscription. If the connection
    /// falls behind `lighter_trades`, `lighter_open_interest` or
    /// `lighter_funding`, a notice such as
    /// `Dropped ~N live lighter_trades messages for BTC: ...`
    /// reports messages that were not delivered, and the subscription
    /// continues. If the lag persists, a notice such as
    /// `Stopped the lighter_trades stream for BTC: ...` means the server ended
    /// that subscription; subscribe again to resume. `lighter_orderbook` sends
    /// the newest book at most once per interval and never sends an older
    /// book in place of a newer one.
    Error {
        message: String,
    },
    /// A live data message.
    ///
    /// For the live Lighter channels, [`ServerMsg::lighter_live_data`] decodes
    /// `data` into [`LighterLiveData`].
    Data {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        data: serde_json::Value,
    },
    /// Initial order-level L4 state for a live subscription or core replay.
    ///
    /// For core Hyperliquid `l4_diffs` and `l4_orders` replay, this checkpoint
    /// frame is emitted first and is followed by ordered [`ServerMsg::L4Batch`]
    /// frames. `data` contains the full `bids`/`asks` book plus checkpoint
    /// metadata; large symbols can be tens of MB of JSON. HIP-3, HIP-4, and
    /// Spot L4 channels remain live-only and never use this replay sequence.
    L4Snapshot {
        channel: String,
        coin: String,
        symbol: String,
        /// Block number of the last applied diff in this snapshot.
        last_block_number: u64,
        timestamp: i64,
        data: serde_json::Value,
    },
    /// Ordered L4 event batch for a live stream or core replay.
    ///
    /// In core Hyperliquid replay, apply every item in each batch in array
    /// order after [`ServerMsg::L4Snapshot`]. Diff and order-lifecycle items
    /// have channel-specific fields, so the payload remains JSON while the
    /// envelope and event ordering are typed by this enum.
    L4Batch {
        channel: String,
        coin: String,
        symbol: String,
        data: Vec<serde_json::Value>,
    },
    HistoricalData {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        timestamp: i64,
        data: serde_json::Value,
    },
    ReplaySnapshot {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        timestamp: i64,
        data: serde_json::Value,
    },
    HistoricalBatch {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        data: Vec<serde_json::Value>,
    },
    ReplayStarted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
    },
    ReplayPaused {
        current_timestamp: Option<i64>,
    },
    ReplayResumed {
        current_timestamp: Option<i64>,
    },
    ReplayCompleted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        snapshots_sent: Option<i64>,
    },
    ReplayStopped,
    /// No longer sent: the server discontinued bulk streaming. Kept so
    /// existing matches still compile.
    StreamStarted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
    },
    /// No longer sent: the server discontinued bulk streaming. Kept so
    /// existing matches still compile.
    StreamProgress {
        snapshots_sent: Option<i64>,
    },
    /// No longer sent: the server discontinued bulk streaming. Kept so
    /// existing matches still compile.
    StreamCompleted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        snapshots_sent: Option<i64>,
    },
    /// No longer sent: the server discontinued bulk streaming. Kept so
    /// existing matches still compile.
    StreamStopped {
        snapshots_sent: Option<i64>,
    },
    GapDetected {
        channel: Option<String>,
        coin: Option<String>,
        symbol: Option<String>,
        gap_start: Option<i64>,
        gap_end: Option<i64>,
        duration_minutes: Option<f64>,
    },
    /// Terminal signal for a HIP-4 coin: the outcome settled to `0` or `1`.
    ///
    /// Emitted at most once per `(outcome_id, side)`. On receipt, the server
    /// proactively unsubscribes the client from every `hip4_*` subscription
    /// for `coin`. Other subscriptions (Hyperliquid perps, HIP-3, etc.)
    /// remain active. Treat this as the terminal frame for the coin.
    OutcomeSettled {
        coin: String,
        outcome_id: u64,
        side: u8,
        settlement_value: Option<f64>,
        settlement_at: Option<String>,
    },
    /// Any message type this SDK version does not know. Carried instead of
    /// being silently dropped so callers can log or ignore explicitly.
    #[serde(other)]
    Unknown,
}

impl ServerMsg {
    /// Decode a live Lighter `data` message into a typed payload.
    ///
    /// Returns `None` for every other message, including Lighter replay
    /// messages (`historical_data`, `replay_snapshot`), whose rows keep their
    /// existing replay shapes. Returns `Some(Err(..))` when a live Lighter
    /// payload does not match the expected shape.
    pub fn lighter_live_data(&self) -> Option<Result<LighterLiveData>> {
        match self {
            ServerMsg::Data { channel, data, .. } => LighterLiveData::decode(channel, data),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket client
// ---------------------------------------------------------------------------

type WsSink =
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>;

/// A WebSocket client for the 0xArchive streaming API.
///
/// Supports two modes on a single connection:
/// - **Real-time**: subscribe to live market data
/// - **Replay**: replay historical data with timing preserved
///
/// Bulk streaming ([`stream`](Self::stream)) has been discontinued by the
/// server. For large historical downloads, use the S3 Parquet bulk export at
/// <https://www.0xarchive.io/data>.
pub struct OxArchiveWs {
    options: WsOptions,
    sink: Arc<Mutex<Option<WsSink>>>,
    /// Receive server messages from this channel.
    pub rx: Option<mpsc::UnboundedReceiver<ServerMsg>>,
}

impl OxArchiveWs {
    pub fn new(options: WsOptions) -> Self {
        Self {
            options,
            sink: Arc::new(Mutex::new(None)),
            rx: None,
        }
    }

    /// Connect to the WebSocket server.
    ///
    /// Returns a receiver for server messages. The connection is maintained
    /// in a background task that handles pings and reconnection.
    pub async fn connect(&mut self) -> Result<()> {
        let url = format!("{}?apiKey={}", self.options.ws_url, self.options.api_key);
        let (ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))?;

        let (write, mut read) = ws_stream.split();
        *self.sink.lock().await = Some(write);

        let (tx, rx) = mpsc::unbounded_channel();
        self.rx = Some(rx);

        let sink = self.sink.clone();

        // Background task: read messages, handle pings, forward to channel
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(server_msg) = serde_json::from_str::<ServerMsg>(&text) {
                            let _ = tx.send(server_msg);
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        if let Some(ref mut writer) = *sink.lock().await {
                            let _ = writer.send(Message::Pong(data)).await;
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Send a message to the server.
    pub async fn send(&self, msg: ClientMsg) -> Result<()> {
        let text = serde_json::to_string(&msg).map_err(|e| Error::WebSocket(e.to_string()))?;
        if let Some(ref mut writer) = *self.sink.lock().await {
            writer
                .send(Message::Text(text))
                .await
                .map_err(|e| Error::WebSocket(e.to_string()))?;
        }
        Ok(())
    }

    /// Subscribe to a supported live channel.
    ///
    /// Live Lighter subscriptions are available for `lighter_orderbook`,
    /// `lighter_trades`, `lighter_open_interest` and `lighter_funding`, using
    /// the same symbols as `client.lighter.instruments.list()`. Symbols are
    /// case-insensitive and echoed uppercase. `lighter_orderbook` sends one
    /// full book per second; use [`subscribe_with_interval`](Self::subscribe_with_interval)
    /// to change that. Decode Lighter payloads with
    /// [`ServerMsg::lighter_live_data`].
    ///
    /// `lighter_candles` and `lighter_l3_orderbook` support replay, not live
    /// subscriptions, and are rejected before a request is sent. Use REST for
    /// their current data or a bounded replay request for stored history.
    pub async fn subscribe(&self, channel: &str, symbol: Option<&str>) -> Result<()> {
        if is_lighter_replay_only_channel(channel) {
            return Err(Error::InvalidParam(LIGHTER_SUBSCRIPTION_ERROR.to_string()));
        }

        self.send(ClientMsg::Subscribe {
            channel: channel.to_string(),
            symbol: symbol.map(|s| s.to_string()),
        })
        .await
    }

    /// Subscribe to `lighter_orderbook` with a custom book interval.
    ///
    /// The server sends the newest book at most once per `interval_ms`, which
    /// must be between 100 and 5000 inclusive. Each book sent is one message.
    /// Without an interval, [`subscribe`](Self::subscribe) delivers one book
    /// per second.
    ///
    /// `interval_ms` is accepted only on `lighter_orderbook`. Other channels
    /// and out-of-range values are rejected before a request is sent.
    ///
    /// ```no_run
    /// # use oxarchive::ws::{OxArchiveWs, WsOptions};
    /// # async fn example() -> oxarchive::Result<()> {
    /// let mut ws = OxArchiveWs::new(WsOptions::new("your-api-key"));
    /// ws.connect().await?;
    /// // Up to four books a second instead of one.
    /// ws.subscribe_with_interval("lighter_orderbook", "BTC", 250).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn subscribe_with_interval(
        &self,
        channel: &str,
        symbol: &str,
        interval_ms: u32,
    ) -> Result<()> {
        validate_subscribe_interval(channel, interval_ms)?;
        self.send(ClientMsg::SubscribeWithInterval {
            channel: channel.to_string(),
            symbol: symbol.to_string(),
            interval_ms,
        })
        .await
    }

    /// Unsubscribe from a real-time channel.
    pub async fn unsubscribe(&self, channel: &str, symbol: Option<&str>) -> Result<()> {
        self.send(ClientMsg::Unsubscribe {
            channel: channel.to_string(),
            symbol: symbol.map(|s| s.to_string()),
        })
        .await
    }

    /// Start a bounded historical replay on a single channel.
    ///
    /// All six `lighter_*` channels support replay. Lighter replay rows keep
    /// their stored shapes, which differ from the live Lighter payloads
    /// decoded by [`ServerMsg::lighter_live_data`]. Hyperliquid
    /// core `l4_diffs` and `l4_orders` replay as `l4_snapshot` followed by
    /// ordered `l4_batch` frames, and ignore `speed`. HIP-3, HIP-4, and Spot L4
    /// channels are live-only and are rejected before a request is sent. A
    /// successful replay terminates with a `replay_completed` server message.
    pub async fn replay(
        &self,
        channel: &str,
        symbol: &str,
        start: i64,
        end: Option<i64>,
        speed: Option<f64>,
    ) -> Result<()> {
        validate_replay_channel(channel)?;
        self.send(ClientMsg::Replay {
            channel: channel.to_string(),
            symbol: symbol.to_string(),
            start,
            end,
            speed,
        })
        .await
    }

    /// Start a multi-channel synchronized standard replay.
    ///
    /// All channels are replayed together with data interleaved chronologically,
    /// including the six Lighter replay channels. Core L4 replay is single-
    /// channel and cannot be included here. HIP-3, HIP-4, and Spot L4 channels
    /// remain live-only. Initial `replay_snapshot` messages provide each
    /// standard channel's state at `start`; the server terminates the bounded
    /// replay with `replay_completed`.
    pub async fn replay_multi(
        &self,
        channels: &[&str],
        symbol: &str,
        start: i64,
        end: Option<i64>,
        speed: Option<f64>,
    ) -> Result<()> {
        for channel in channels {
            validate_replay_channel(channel)?;
            if is_core_l4_replay_channel(channel) {
                return Err(Error::InvalidParam(
                    "Hyperliquid core L4 replay is single-channel; use replay() for l4_diffs or l4_orders."
                        .to_string(),
                ));
            }
        }
        self.send(ClientMsg::ReplayMulti {
            channels: channels.iter().map(|c| c.to_string()).collect(),
            symbol: symbol.to_string(),
            start,
            end,
            speed,
        })
        .await
    }

    /// Pause an active replay.
    pub async fn replay_pause(&self) -> Result<()> {
        self.send(ClientMsg::ReplayPause).await
    }

    /// Resume a paused replay.
    pub async fn replay_resume(&self) -> Result<()> {
        self.send(ClientMsg::ReplayResume).await
    }

    /// Seek to a specific timestamp in a replay.
    pub async fn replay_seek(&self, timestamp: i64) -> Result<()> {
        self.send(ClientMsg::ReplaySeek { timestamp }).await
    }

    /// Stop an active replay.
    pub async fn replay_stop(&self) -> Result<()> {
        self.send(ClientMsg::ReplayStop).await
    }

    /// Start a bulk data stream. Deprecated: the server has discontinued bulk
    /// streaming.
    ///
    /// This method still sends the request, but the server now answers it
    /// with a [`ServerMsg::Error`] on the receiver instead of streaming data.
    /// The message says bulk streaming has been discontinued and points to the
    /// S3 Parquet bulk export. The returned `Result` only reports whether the
    /// request was sent.
    ///
    /// For large historical downloads, use the S3 Parquet bulk export at
    /// <https://www.0xarchive.io/data>. For bounded windows over WebSocket,
    /// use [`replay`](Self::replay).
    #[deprecated(
        since = "1.11.0",
        note = "the server has discontinued bulk streaming and answers this request with an error message; use the S3 Parquet bulk export at https://www.0xarchive.io/data for large downloads, or replay() for bounded windows"
    )]
    pub async fn stream(
        &self,
        channel: &str,
        symbol: &str,
        start: i64,
        end: i64,
        batch_size: Option<usize>,
    ) -> Result<()> {
        self.send(ClientMsg::Stream {
            channel: channel.to_string(),
            symbol: symbol.to_string(),
            start,
            end,
            batch_size,
        })
        .await
    }

    /// Stop an active bulk stream. Deprecated: the server has discontinued
    /// bulk streaming.
    ///
    /// No bulk stream can be active, so the server answers this request with
    /// a [`ServerMsg::Error`] on the receiver. To stop a replay, use
    /// [`replay_stop`](Self::replay_stop).
    #[deprecated(
        since = "1.11.0",
        note = "the server has discontinued bulk streaming and answers this request with an error message; use the S3 Parquet bulk export at https://www.0xarchive.io/data for large downloads"
    )]
    pub async fn stream_stop(&self) -> Result<()> {
        self.send(ClientMsg::StreamStop).await
    }

    /// Send an application-level ping.
    pub async fn ping(&self) -> Result<()> {
        self.send(ClientMsg::Ping).await
    }

    /// Disconnect from the server.
    pub async fn disconnect(&self) {
        if let Some(ref mut writer) = *self.sink.lock().await {
            let _ = writer.close().await;
        }
    }
}

#[cfg(test)]
mod lighter_live_tests {
    use super::{ClientMsg, ServerMsg};
    use crate::types::LighterLiveData;

    const ORDERBOOK_FRAME: &str = r#"{"type":"data","channel":"lighter_orderbook","coin":"BTC","symbol":"BTC","data":{"coin":"BTC","time":1790294171459,"levels":[[{"px":"84368.7","sz":"0.00020","n":1},{"px":"84368.6","sz":"0.00020","n":1},{"px":"84368.3","sz":"0.00010","n":1}],[{"px":"84368.8","sz":"0.05720","n":1},{"px":"84368.9","sz":"0.14223","n":1},{"px":"84369.1","sz":"0.01198","n":1}]]}}"#;

    const TRADES_FRAME: &str = r#"{"type":"data","channel":"lighter_trades","coin":"BTC","symbol":"BTC","data":[{"coin":"BTC","side":"A","px":"84367.9","sz":"0.00003","time":1790294182211,"hash":"0000001dc8774b28000001a0d5d94943000000000000000000000000000000000000000000000000","tid":31944180930,"oid":562953419896990,"crossed":false,"dir":null,"fee":null,"fee_token":null,"closed_pnl":null,"start_position":"109.79011","users":["281474976623827"]},{"coin":"BTC","side":"B","px":"84367.9","sz":"0.00003","time":1790294182211,"hash":"0000001dc8774b28000001a0d5d94943000000000000000000000000000000000000000000000000","tid":31944180930,"oid":844421425107071,"crossed":true,"dir":null,"fee":null,"fee_token":null,"closed_pnl":null,"start_position":"0.03940","users":["713845"]}]}"#;

    const STATS_PAYLOAD: &str = r#"{"coin":"BTC","ctx":{"openInterest":"172706178.266310","funding":"0.000012","premium":"-0.000327","markPx":"84363.5","oraclePx":"84397.0","midPx":"84368.8","dayNtlVlm":"908611371.550746","dayBaseVlm":"10808.97087","prevDayPx":"84285.9","impactPxs":null}}"#;

    fn stats_frame(channel: &str) -> String {
        format!(
            r#"{{"type":"data","channel":"{channel}","coin":"BTC","symbol":"BTC","data":{STATS_PAYLOAD}}}"#
        )
    }

    #[test]
    fn orderbook_frame_decodes_bids_then_asks() {
        let msg: ServerMsg = serde_json::from_str(ORDERBOOK_FRAME).unwrap();
        let Some(Ok(LighterLiveData::OrderBook(book))) = msg.lighter_live_data() else {
            panic!("expected a decoded lighter_orderbook payload, got {msg:?}");
        };
        assert_eq!(book.coin, "BTC");
        assert_eq!(book.time, 1790294171459);
        assert_eq!(book.bids().len(), 3);
        assert_eq!(book.asks().len(), 3);
        assert_eq!(book.bids()[0].px, "84368.7");
        assert_eq!(book.bids()[0].sz, "0.00020");
        assert_eq!(book.bids()[0].n, 1);
        assert_eq!(book.asks()[0].px, "84368.8");
        assert_eq!(book.asks()[2].sz, "0.01198");
        // Best bid is below best ask.
        let best_bid: f64 = book.bids()[0].px.parse().unwrap();
        let best_ask: f64 = book.asks()[0].px.parse().unwrap();
        assert!(best_bid < best_ask);
    }

    #[test]
    fn orderbook_sides_are_empty_when_levels_are_missing() {
        let book: crate::types::LighterLiveOrderBook =
            serde_json::from_str(r#"{"coin":"BTC","time":1,"levels":[]}"#).unwrap();
        assert!(book.bids().is_empty());
        assert!(book.asks().is_empty());
    }

    #[test]
    fn trades_frame_decodes_two_fills_per_trade() {
        let msg: ServerMsg = serde_json::from_str(TRADES_FRAME).unwrap();
        let Some(Ok(LighterLiveData::Trades(fills))) = msg.lighter_live_data() else {
            panic!("expected a decoded lighter_trades payload, got {msg:?}");
        };
        assert_eq!(fills.len(), 2);
        let (maker, taker) = (&fills[0], &fills[1]);
        assert_eq!(maker.tid, taker.tid);
        assert_eq!(maker.tid, 31944180930);
        assert_eq!(maker.side, "A");
        assert!(!maker.crossed);
        assert_eq!(maker.oid, Some(562953419896990));
        assert_eq!(maker.start_position.as_deref(), Some("109.79011"));
        assert_eq!(maker.users, vec!["281474976623827".to_string()]);
        assert_eq!(taker.side, "B");
        assert!(taker.crossed);
        assert_eq!(taker.oid, Some(844421425107071));
        assert_eq!(taker.users, vec!["713845".to_string()]);
        assert_eq!(taker.px, "84367.9");
        assert_eq!(taker.sz, "0.00003");
        assert_eq!(taker.time, 1790294182211);
        assert_eq!(
            taker.hash.as_deref(),
            Some(
                "0000001dc8774b28000001a0d5d94943000000000000000000000000000000000000000000000000"
            )
        );
        for fill in &fills {
            assert_eq!(fill.dir, None);
            assert_eq!(fill.fee, None);
            assert_eq!(fill.fee_token, None);
            assert_eq!(fill.closed_pnl, None);
        }
        // One trade, not two.
        let trades: std::collections::HashSet<i64> = fills.iter().map(|f| f.tid).collect();
        assert_eq!(trades.len(), 1);
    }

    #[test]
    fn open_interest_and_funding_frames_carry_the_same_stats() {
        let oi: ServerMsg = serde_json::from_str(&stats_frame("lighter_open_interest")).unwrap();
        let funding: ServerMsg = serde_json::from_str(&stats_frame("lighter_funding")).unwrap();
        let Some(Ok(LighterLiveData::OpenInterest(oi))) = oi.lighter_live_data() else {
            panic!("expected a decoded lighter_open_interest payload");
        };
        let Some(Ok(LighterLiveData::Funding(funding))) = funding.lighter_live_data() else {
            panic!("expected a decoded lighter_funding payload");
        };
        assert_eq!(oi, funding);
        assert_eq!(oi.coin, "BTC");
        let ctx = &oi.ctx;
        assert_eq!(ctx.open_interest.as_deref(), Some("172706178.266310"));
        assert_eq!(ctx.funding.as_deref(), Some("0.000012"));
        assert_eq!(ctx.premium.as_deref(), Some("-0.000327"));
        assert_eq!(ctx.mark_px.as_deref(), Some("84363.5"));
        assert_eq!(ctx.oracle_px.as_deref(), Some("84397.0"));
        assert_eq!(ctx.mid_px.as_deref(), Some("84368.8"));
        assert_eq!(ctx.day_ntl_vlm.as_deref(), Some("908611371.550746"));
        assert_eq!(ctx.day_base_vlm.as_deref(), Some("10808.97087"));
        assert_eq!(ctx.prev_day_px.as_deref(), Some("84285.9"));
        assert_eq!(ctx.impact_pxs, None);
    }

    #[test]
    fn stats_round_trip_keeps_camel_case_wire_keys() {
        let stats: crate::types::LighterLiveMarketStats =
            serde_json::from_str(STATS_PAYLOAD).unwrap();
        let back = serde_json::to_value(&stats).unwrap();
        let original: serde_json::Value = serde_json::from_str(STATS_PAYLOAD).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn only_live_lighter_data_messages_decode() {
        // Replay rows keep their stored shapes and are not decoded as live data.
        let replay = r#"{"type":"historical_data","channel":"lighter_orderbook","coin":"BTC","symbol":"BTC","timestamp":1790294171459,"data":{"bids":[],"asks":[]}}"#;
        let msg: ServerMsg = serde_json::from_str(replay).unwrap();
        assert!(msg.lighter_live_data().is_none());

        // Hyperliquid live data is not a Lighter payload.
        let hl = r#"{"type":"data","channel":"orderbook","coin":"BTC","symbol":"BTC","data":{"coin":"BTC","time":1,"levels":[[],[]]}}"#;
        let msg: ServerMsg = serde_json::from_str(hl).unwrap();
        assert!(msg.lighter_live_data().is_none());

        // Replay-only Lighter channels have no live payload.
        let candles =
            r#"{"type":"data","channel":"lighter_candles","coin":"BTC","symbol":"BTC","data":{}}"#;
        let msg: ServerMsg = serde_json::from_str(candles).unwrap();
        assert!(msg.lighter_live_data().is_none());

        // A malformed live payload reports an error instead of panicking.
        let bad = r#"{"type":"data","channel":"lighter_trades","coin":"BTC","symbol":"BTC","data":{"not":"an array"}}"#;
        let msg: ServerMsg = serde_json::from_str(bad).unwrap();
        assert!(matches!(msg.lighter_live_data(), Some(Err(_))));
    }

    #[test]
    fn subscribe_messages_serialize_to_the_wire_format() {
        let plain = ClientMsg::Subscribe {
            channel: "lighter_orderbook".to_string(),
            symbol: Some("BTC".to_string()),
        };
        assert_eq!(
            serde_json::to_value(&plain).unwrap(),
            serde_json::json!({"op": "subscribe", "channel": "lighter_orderbook", "symbol": "BTC"})
        );

        let with_interval = ClientMsg::SubscribeWithInterval {
            channel: "lighter_orderbook".to_string(),
            symbol: "BTC".to_string(),
            interval_ms: 250,
        };
        assert_eq!(
            serde_json::to_value(&with_interval).unwrap(),
            serde_json::json!({
                "op": "subscribe",
                "channel": "lighter_orderbook",
                "symbol": "BTC",
                "interval_ms": 250
            })
        );

        let unsubscribe = ClientMsg::Unsubscribe {
            channel: "lighter_trades".to_string(),
            symbol: Some("BTC".to_string()),
        };
        assert_eq!(
            serde_json::to_value(&unsubscribe).unwrap(),
            serde_json::json!({"op": "unsubscribe", "channel": "lighter_trades", "symbol": "BTC"})
        );
    }

    #[test]
    fn subscribed_and_unsubscribed_acks_parse() {
        let ack =
            r#"{"type":"subscribed","channel":"lighter_orderbook","coin":"BTC","symbol":"BTC"}"#;
        match serde_json::from_str::<ServerMsg>(ack).unwrap() {
            ServerMsg::Subscribed {
                channel,
                coin,
                symbol,
            } => {
                assert_eq!(channel, "lighter_orderbook");
                assert_eq!(coin.as_deref(), Some("BTC"));
                assert_eq!(symbol.as_deref(), Some("BTC"));
            }
            other => panic!("wrong variant: {other:?}"),
        }
        let unack =
            r#"{"type":"unsubscribed","channel":"lighter_trades","coin":"BTC","symbol":"BTC"}"#;
        assert!(matches!(
            serde_json::from_str::<ServerMsg>(unack).unwrap(),
            ServerMsg::Unsubscribed { .. }
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::ServerMsg;

    #[test]
    fn l4_snapshot_deserializes() {
        let json = r#"{"type":"l4_snapshot","channel":"l4_diffs","coin":"BTC","symbol":"BTC","last_block_number":1087344118,"timestamp":1785540000000,"data":{"bids":[],"asks":[]}}"#;
        match serde_json::from_str::<ServerMsg>(json).expect("l4_snapshot must parse") {
            ServerMsg::L4Snapshot { coin, last_block_number, .. } => {
                assert_eq!(coin, "BTC");
                assert_eq!(last_block_number, 1087344118);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn l4_batch_deserializes() {
        let json = r#"{"type":"l4_batch","channel":"l4_diffs","coin":"BTC","symbol":"BTC","data":[{"oid":1},{"oid":2}]}"#;
        match serde_json::from_str::<ServerMsg>(json).expect("l4_batch must parse") {
            ServerMsg::L4Batch { data, .. } => assert_eq!(data.len(), 2),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn unknown_type_maps_to_unknown_not_error() {
        let json = r#"{"type":"some_future_message","payload":123}"#;
        let msg = serde_json::from_str::<ServerMsg>(json).expect("unknown types must not error");
        assert!(matches!(msg, ServerMsg::Unknown));
    }

    #[test]
    fn l4_diff_entry_carries_seq_and_insert_before() {
        let json = r#"{"coin":"BTC","timestamp":"2026-07-26T22:31:23.618Z","block_number":1087344118,"seq":116,"oid":503076737852,"side":"B","price":58671.0,"diff_type":"new","new_size":0.00342,"user_address":"0xd4bb","insert_before":503076737000}"#;
        let d: crate::types::L4DiffEntry = serde_json::from_str(json).unwrap();
        assert_eq!(d.seq, 116);
        assert_eq!(d.insert_before, Some(503076737000));
        // seq/insert_before absent (pre-native-seq rows, tail placements)
        let json2 = r#"{"coin":"BTC","timestamp":"t","block_number":1,"oid":2,"side":"A","price":1.0,"diff_type":"remove","new_size":null,"user_address":"0x"}"#;
        let d2: crate::types::L4DiffEntry = serde_json::from_str(json2).unwrap();
        assert_eq!(d2.seq, 0);
        assert_eq!(d2.insert_before, None);
    }
}
