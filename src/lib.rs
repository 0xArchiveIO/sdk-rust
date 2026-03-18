//! # oxarchive
//!
//! Rust client library for the [0xArchive](https://0xarchive.io) API.
//!
//! Query historical and real-time crypto market data — orderbooks, trades,
//! candles, funding rates, open interest, and liquidations across Hyperliquid,
//! Lighter.xyz, and HIP-3.
//!
//! ## Quick start
//!
//! ```no_run
//! use oxarchive::OxArchive;
//!
//! #[tokio::main]
//! async fn main() -> oxarchive::Result<()> {
//!     let client = OxArchive::new("your-api-key")?;
//!
//!     // Get BTC orderbook from Hyperliquid
//!     let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
//!     println!("BTC mid price: {:?}", ob.mid_price);
//!
//!     // List Lighter.xyz instruments
//!     let instruments = client.lighter.instruments.list().await?;
//!     println!("Lighter has {} instruments", instruments.len());
//!
//!     // Get current funding rate
//!     let funding = client.hyperliquid.funding.current("ETH").await?;
//!     println!("ETH funding: {}", funding.funding_rate);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Pagination
//!
//! Historical endpoints return [`CursorResponse`] with an optional
//! `next_cursor`. Pass it back as the `cursor` parameter to fetch the
//! next page:
//!
//! ```no_run
//! # use oxarchive::{OxArchive, types::CursorResponse};
//! # use oxarchive::resources::trades::GetTradesParams;
//! # async fn example() -> oxarchive::Result<()> {
//! # let client = OxArchive::new("key")?;
//! let mut all_trades = vec![];
//! let mut cursor = None;
//!
//! loop {
//!     let result = client.hyperliquid.trades.list("BTC", GetTradesParams {
//!         start: 1704067200000_i64.into(),
//!         end: 1704153600000_i64.into(),
//!         cursor,
//!         limit: Some(1000),
//!         side: None,
//!     }).await?;
//!
//!     all_trades.extend(result.data);
//!     cursor = result.next_cursor;
//!     if cursor.is_none() {
//!         break;
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## WebSocket (optional)
//!
//! Enable the `websocket` feature for real-time streaming, historical replay,
//! and bulk data download:
//!
//! ```toml
//! oxarchive = { version = "1.2", features = ["websocket"] }
//! ```

pub mod client;
pub mod error;
pub mod exchanges;
pub mod http;
pub mod orderbook_reconstructor;
pub mod resources;
pub mod types;
pub mod ws;

// Re-export the main entry points at the crate root.
pub use client::{ClientBuilder, OxArchive};
pub use error::{Error, Result};
pub use orderbook_reconstructor::{
    reconstruct_final, reconstruct_orderbook, OrderBookReconstructor,
};
pub use types::CursorResponse;

#[cfg(feature = "websocket")]
pub use ws::{ClientMsg, OxArchiveWs, ServerMsg, WsOptions};
