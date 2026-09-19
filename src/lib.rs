//! # oxarchive
//!
//! Rust client library for the [0xArchive](https://0xarchive.io) API.
//!
//! Query historical and real-time crypto market data: orderbooks, trades,
//! candles, funding rates, open interest, and liquidations across Hyperliquid
//! perps, Hyperliquid Spot, HIP-3 builder perps, HIP-4 outcome markets, and
//! Lighter.xyz.
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
//! ## Webhooks
//!
//! `client.webhooks` covers the whole management surface: the event-type
//! catalog, endpoints, subscriptions, watched wallets, the delivery log, and
//! the two preview routes. [`webhook_signature`] verifies the deliveries that
//! arrive.
//!
//! ```no_run
//! # use oxarchive::{CreateEndpointParams, EstimateParams, OxArchive};
//! # use serde_json::json;
//! # async fn example() -> oxarchive::Result<()> {
//! # let client = OxArchive::new("key")?;
//! // How often would this rule have fired over the last week?
//! let estimate = client.webhooks.estimate(
//!     EstimateParams::new("market.liquidation")
//!         .filters(json!({"venue": "hyperliquid", "min_notional_usd": 250_000}))
//!         .lookback_days(7),
//! ).await?;
//! println!("{} in {} days", estimate.total, estimate.days);
//!
//! // Then point it somewhere. The secret is shown exactly once.
//! let endpoint = client.webhooks.create_endpoint(
//!     CreateEndpointParams::new("https://example.com/hooks/0xarchive"),
//! ).await?;
//! let secret = endpoint.secret.expect("create returns the secret once");
//! # Ok(())
//! # }
//! ```
//!
//! Webhook delivery is a paid feature: Free plans get no endpoints,
//! subscriptions, watched wallets or deliveries. The estimate and dry-run
//! previews answer on every plan. See [`resources::webhooks`] for the grid.
//!
//! ## WebSocket (optional)
//!
//! Enable the `websocket` feature for real-time streaming, historical replay,
//! and bulk data download:
//!
//! ```toml
//! oxarchive = { version = "1.9", features = ["websocket"] }
//! ```

pub mod client;
pub mod error;
pub mod exchanges;
pub mod http;
pub mod l4_reconstructor;
pub mod orderbook_reconstructor;
pub mod resources;
pub mod types;
pub mod webhook_signature;
pub mod ws;

// Re-export the main entry points at the crate root.
pub use client::{ClientBuilder, OxArchive};
pub use error::{Error, Result};
pub use exchanges::Hip4;
pub use l4_reconstructor::{L4OrderBookReconstructor, L4Order, L4Diff, L2Level};
pub use orderbook_reconstructor::{
    reconstruct_final, reconstruct_orderbook, OrderBookReconstructor,
};
pub use types::{
    CursorResponse, Hip4AggregatedOi, Hip4OpenInterestRecord, Hip4Outcome, Hip4OutcomeAggregate,
    Hip4SideSpec, L4OrderBookSnapshot, L4OrderEntry, L4DiffEntry,
    L2OrderBookSnapshot, L2PriceLevel, L2DiffEntry, OrderHistoryEntry,
    LiquidationLevelBucket, LiquidationLevels, LiquidationLevelsHistoryItem,
    TriggerLevelBucket, TriggerLevels, TriggerLevelsHistoryItem,
};
pub use resources::liquidations::{LevelsHistoryParams, LiquidationLevelsParams};
pub use resources::orders::TriggerLevelsParams;
pub use resources::webhooks::{
    CreateEndpointParams, CreateSubscriptionParams, DryRunParams, EstimateParams,
    UpdateSubscriptionParams, WebhooksResource,
};
pub use types::{
    RotatedSecret, WatchedAddress, WatchedAddressList, WebhookCostFloor, WebhookDayCount,
    WebhookDelivery, WebhookDistribution, WebhookDryRun, WebhookEndpoint, WebhookEstimate,
    WebhookEstimateBasis, WebhookEventType, WebhookLadderRung, WebhookMetricSpec,
    WebhookOccurrence, WebhookParamSpec, WebhookRedelivery, WebhookSubscription, WebhookTestFire,
    WebhookWindow,
};
pub use webhook_signature::{
    parse_signature_header, ParsedSignature, SignatureError, WebhookVerifier,
    DEFAULT_TOLERANCE_SECS, EVENT_ID_HEADER, EVENT_TYPE_HEADER, SIGNATURE_HEADER,
};

#[cfg(feature = "websocket")]
pub use ws::{ClientMsg, OxArchiveWs, ServerMsg, WsOptions};
