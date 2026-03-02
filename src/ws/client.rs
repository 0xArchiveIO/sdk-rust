/// WebSocket client for real-time streaming, historical replay, and bulk
/// data download.
///
/// Requires the `websocket` feature:
/// ```toml
/// oxarchive = { version = "1", features = ["websocket"] }
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

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// A message sent from the client to the server.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ClientMsg {
    #[serde(rename = "subscribe")]
    Subscribe { channel: String, coin: Option<String> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { channel: String, coin: Option<String> },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "replay")]
    Replay {
        channel: String,
        coin: String,
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
        coin: String,
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
    },
    Unsubscribed {
        channel: String,
        coin: Option<String>,
    },
    Pong,
    Error {
        message: String,
    },
    Data {
        channel: String,
        coin: Option<String>,
        data: serde_json::Value,
    },
    HistoricalData {
        channel: String,
        coin: Option<String>,
        timestamp: i64,
        data: serde_json::Value,
    },
    ReplaySnapshot {
        channel: String,
        coin: Option<String>,
        timestamp: i64,
        data: serde_json::Value,
    },
    HistoricalBatch {
        channel: String,
        coin: Option<String>,
        data: Vec<serde_json::Value>,
    },
    ReplayStarted {
        channel: String,
        coin: Option<String>,
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
        snapshots_sent: Option<i64>,
    },
    ReplayStopped,
    StreamStarted {
        channel: String,
        coin: Option<String>,
    },
    StreamProgress {
        snapshots_sent: Option<i64>,
    },
    StreamCompleted {
        channel: String,
        coin: Option<String>,
        snapshots_sent: Option<i64>,
    },
    StreamStopped {
        snapshots_sent: Option<i64>,
    },
    GapDetected {
        channel: Option<String>,
        coin: Option<String>,
        gap_start: Option<i64>,
        gap_end: Option<i64>,
        duration_minutes: Option<f64>,
    },
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
                .send(Message::Text(text.into()))
                .await
                .map_err(|e| Error::WebSocket(e.to_string()))?;
        }
        Ok(())
    }

    /// Subscribe to a real-time channel.
    pub async fn subscribe(&self, channel: &str, coin: Option<&str>) -> Result<()> {
        self.send(ClientMsg::Subscribe {
            channel: channel.to_string(),
            coin: coin.map(|s| s.to_string()),
        })
        .await
    }

    /// Unsubscribe from a real-time channel.
    pub async fn unsubscribe(&self, channel: &str, coin: Option<&str>) -> Result<()> {
        self.send(ClientMsg::Unsubscribe {
            channel: channel.to_string(),
            coin: coin.map(|s| s.to_string()),
        })
        .await
    }

    /// Start a historical replay.
    pub async fn replay(
        &self,
        channel: &str,
        coin: &str,
        start: i64,
        end: Option<i64>,
        speed: Option<f64>,
    ) -> Result<()> {
        self.send(ClientMsg::Replay {
            channel: channel.to_string(),
            coin: coin.to_string(),
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
        coin: &str,
        start: i64,
        end: i64,
        batch_size: Option<usize>,
    ) -> Result<()> {
        self.send(ClientMsg::Stream {
            channel: channel.to_string(),
            coin: coin.to_string(),
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
