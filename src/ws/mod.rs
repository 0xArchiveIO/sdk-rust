//! WebSocket client for live subscriptions and historical replay:
//! `OxArchiveWs`, its options (`WsOptions`), the messages it sends
//! (`ClientMsg`) and receives (`ServerMsg`), and the channel lists.
//!
//! Requires the `websocket` feature:
//! ```toml
//! oxarchive = { version = "1.12", features = ["websocket"] }
//! ```
//!
//! For large historical downloads, use the S3 Parquet bulk export at
//! <https://www.0xarchive.io/data>.

#[cfg(feature = "websocket")]
mod client;

#[cfg(feature = "websocket")]
pub use client::*;
