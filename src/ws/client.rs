/// WebSocket client for supported live streaming, historical replay, and bulk
/// data download.
///
/// Requires the `websocket` feature:
/// ```toml
/// oxarchive = { version = "1.9", features = ["websocket"] }
/// ```

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::error::{Error, Result};

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

/// Lighter channels support historical replay but not live subscriptions.
pub const LIGHTER_REPLAY_CHANNELS: [&str; 6] = [
    "lighter_orderbook",
    "lighter_trades",
    "lighter_candles",
    "lighter_open_interest",
    "lighter_funding",
    "lighter_l3_orderbook",
];

/// Canonical guidance returned when a Lighter live subscription is requested.
pub const LIGHTER_SUBSCRIPTION_ERROR: &str =
    "Lighter WebSocket channels support replay, not live subscriptions. Use REST for current data or a replay request for stored history.";

/// Return whether a channel is available through replay but not live subscribe.
pub fn is_lighter_replay_channel(channel: &str) -> bool {
    LIGHTER_REPLAY_CHANNELS.contains(&channel)
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
    #[serde(rename = "stream")]
    Stream {
        channel: String,
        symbol: String,
        start: i64,
        end: i64,
        batch_size: Option<usize>,
    },
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
    Error {
        message: String,
    },
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
    StreamStarted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
    },
    StreamProgress {
        snapshots_sent: Option<i64>,
    },
    StreamCompleted {
        channel: String,
        coin: Option<String>,
        symbol: Option<String>,
        snapshots_sent: Option<i64>,
    },
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

// ---------------------------------------------------------------------------
// WebSocket client
// ---------------------------------------------------------------------------

type WsSink =
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>;

/// A WebSocket client for the 0xArchive streaming API.
///
/// Supports three modes on a single connection:
/// - **Real-time** — subscribe to live market data
/// - **Replay** — replay historical data with timing preserved
/// - **Stream** — bulk-download historical data as fast as possible
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
    /// Lighter channels support replay, not live subscriptions. Use REST for
    /// current data or a bounded replay request for stored history.
    pub async fn subscribe(&self, channel: &str, symbol: Option<&str>) -> Result<()> {
        if is_lighter_replay_channel(channel) {
            return Err(Error::InvalidParam(LIGHTER_SUBSCRIPTION_ERROR.to_string()));
        }

        self.send(ClientMsg::Subscribe {
            channel: channel.to_string(),
            symbol: symbol.map(|s| s.to_string()),
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
    /// The six `lighter_*` channels support replay but not live subscriptions;
    /// use the corresponding Lighter REST route for current data. Hyperliquid
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

    /// Start a bulk data stream.
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

    /// Stop an active bulk stream.
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
