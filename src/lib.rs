//! # oxarchive
//!
//! Rust client library for the [0xArchive](https://0xarchive.io) API.
//!
//! Query historical and real-time crypto market data: orderbooks, trades,
//! candles, funding rates, open interest, liquidations, and account
//! positions, and manage signed webhook delivery. Two venues are covered:
//! Hyperliquid (core perps, Hyperliquid Spot, HIP-3 builder perps, and HIP-4
//! outcome markets) and Lighter.
//! Lighter has two deployments, mainnet (`client.lighter`) and Robinhood
//! Chain (`client.rh_lighter`).
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
//!     // List Lighter instruments (mainnet)
//!     let instruments = client.lighter.instruments.list().await?;
//!     println!("Lighter has {} instruments", instruments.len());
//!
//!     // Lighter on Robinhood Chain, quoted in USDG
//!     let rh = client.rh_lighter.instruments.list().await?;
//!     println!("Lighter on Robinhood Chain has {} instruments", rh.len());
//!
//!     // Get current funding rate
//!     let funding = client.hyperliquid.funding.current("ETH").await?;
//!     println!("ETH funding: {}", funding.funding_rate);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## API version
//!
//! Every request sends the `0xArchive-Version: 2026-10-01` header
//! ([`API_VERSION`]), and the WebSocket connects with `version=2026-10-01`.
//! The version selects the response shapes this crate's types describe: the
//! standard `{success, data, meta}` envelope, RFC 3339 times with `*_ms`
//! integer twins, and the stable error codes.
//!
//! ## Pagination
//!
//! Historical endpoints return [`CursorResponse`]: the page's `data`,
//! `next_cursor`, `has_more` and the full `meta` block (including the
//! canonical `meta.symbol` and `meta.venue` on per-symbol routes). Pass
//! `next_cursor` back as the `cursor` parameter, with the other parameters
//! unchanged, while `has_more` is `true`:
//!
//! ```no_run
//! # use oxarchive::{OxArchive, types::CursorResponse};
//! # use oxarchive::resources::trades::GetTradesParams;
//! # async fn example() -> oxarchive::Result<()> {
//! # let client = OxArchive::new("key")?;
//! // The last ten minutes
//! let end = chrono::Utc::now();
//! let start = end - chrono::Duration::minutes(10);
//! let mut all_trades = vec![];
//! let mut cursor = None;
//!
//! loop {
//!     let page = client.hyperliquid.trades.history("BTC", GetTradesParams {
//!         start: start.into(),
//!         end: end.into(),
//!         cursor,
//!         limit: Some(1000),
//!         side: None,
//!     }).await?;
//!
//!     all_trades.extend(page.data);
//!     if !page.has_more {
//!         break;
//!     }
//!     cursor = page.next_cursor;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Errors
//!
//! An API error is [`Error::Api`], which carries the HTTP status, the stable
//! [`ErrorCode`] (`invalid_symbol`, `rate_limited`, ...), the request id and,
//! for a bad parameter, its name and accepted values:
//!
//! ```no_run
//! # use oxarchive::{Error, ErrorCode, OxArchive};
//! # async fn example() -> oxarchive::Result<()> {
//! # let client = OxArchive::new("key")?;
//! match client.hyperliquid.orderbook.get("NOT-A-MARKET", None).await {
//!     Err(Error::Api { error_code: Some(ErrorCode::InvalidSymbol), message, .. }) => {
//!         eprintln!("{message}");
//!     }
//!     Err(e) => eprintln!("{e} ({:?})", e.error_code()),
//!     Ok(book) => println!("{:?}", book.mid_price),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Webhooks
//!
//! `client.webhooks` manages webhook delivery: the event catalog, plan
//! limits, endpoints, deliveries, subscriptions, the estimate and dry-run
//! previews, and watched wallets. [`webhook_signature`] verifies the
//! deliveries that arrive.
//!
//! ```no_run
//! # use oxarchive::{CreateEndpointParams, EstimateParams, OxArchive};
//! # use oxarchive::types::{WebhookSubscriptionCondition, WebhookSubscriptionConfig};
//! # async fn example() -> oxarchive::Result<()> {
//! # let client = OxArchive::new("key")?;
//! // How often would this rule have fired over the last week?
//! let config = WebhookSubscriptionConfig::default()
//!     .venue("hyperliquid")
//!     .condition(WebhookSubscriptionCondition::new(
//!         "notional_usd",
//!         "greater_than_or_equal",
//!         250_000,
//!     ));
//! let estimate = client
//!     .webhooks
//!     .estimate(EstimateParams::new("market.liquidation").config(config).lookback_days(7))
//!     .await?;
//! println!("{} in {} days", estimate.total, estimate.days);
//!
//! // Register a destination. The secret is returned once.
//! let endpoint = client
//!     .webhooks
//!     .create_endpoint(CreateEndpointParams::new("https://example.com/hooks/0xarchive"))
//!     .await?;
//! let secret = endpoint.secret;
//! # Ok(())
//! # }
//! ```
//!
//! Webhook delivery starts on the Build plan. The estimate and the dry-run
//! answer on every plan, Free included.
//!
//! ## WebSocket (optional)
//!
//! Enable the `websocket` feature for real-time streaming and historical
//! replay:
//!
//! ```toml
//! oxarchive = { version = "1.13", features = ["websocket"] }
//! ```
//!
//! Live subscriptions cover the supported Hyperliquid channels and four
//! channels on each Lighter deployment: `lighter_orderbook`,
//! `lighter_trades`, `lighter_open_interest` and `lighter_funding` on
//! mainnet, and the same four with the `rh_lighter_` prefix on Robinhood
//! Chain. Their payloads, live and replayed, decode into [`LighterLiveData`].
//! `lighter_candles`, `lighter_l3_orderbook` and `rh_lighter_candles` remain
//! replay-only.
//!
//! Every L4 channel (Hyperliquid core, HIP-3, Spot and HIP-4) and both
//! full-depth order book channels (`orderbook_full`, `hip3_orderbook_full`)
//! replay as bulk, single-channel streams. `client.capabilities()` lists
//! which channels stream live and which replay.
//!
//! The live-only `mempool` channel carries pending Hyperliquid transactions,
//! before they are in a block; its items decode into [`MempoolItem`]. It is
//! served only at `wss://stream.0xarchive.io/ws` (`ws::STREAM_WS_URL`) and
//! included with the Pro, Scale and Enterprise plans.
//!
//! Bulk streaming over WebSocket has been discontinued, so
//! `OxArchiveWs::stream` is deprecated. For large historical downloads, use
//! the S3 Parquet bulk export at <https://www.0xarchive.io/data>.

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
pub use client::{ClientBuilder, OxArchive, API_VERSION, API_VERSION_HEADER};
pub use error::{Error, ErrorCode, Result};
pub use exchanges::{Hip4, Hip4ListQuestionsParams, RhLighterClient};
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
    LighterLiveAssetCtx, LighterLiveData, LighterLiveLevel, LighterLiveMarketStats,
    LighterLiveOrderBook, LighterLiveTrade, MempoolItem, MempoolSignature,
    AccountSummary, CumulativeFunding, LighterL1Account, LighterL1Accounts, LighterLiquidation,
    LighterLiquidationVolume, MarketPosition, MarketPositionsSummary, MetaResponse, Position,
    PositionChange, PositionLeverage, PositionsFreshness, ResponseMeta, WalletPositions,
};
pub use resources::liquidations::{LevelsHistoryParams, LiquidationLevelsParams};
pub use resources::positions::{
    AccountHistoryParams, BulkPositionsParams, GetPositionsParams, MarketPositionsParams,
    MarketSummaryParams, PositionRangeParams,
};
pub use resources::orders::TriggerLevelsParams;
pub use resources::cvd::CvdParams;
pub use resources::wallets::WalletClassifyParams;
pub use resources::webhooks::{
    CreateEndpointParams, CreateSubscriptionParams, DryRunParams, EstimateParams,
    UpdateSubscriptionParams, WebhooksResource,
};
pub use resources::trades::RecentTradesParams;
pub use types::{Capability, TradeSide};
pub use types::{
    BreadthSnapshot, ClassifiedWallet, CvdBucket, Hip3OracleDiscoveryBounds,
    Hip3OracleExternalPrice, Hip4Question, SymbolEntry, WalletClassification, WalletMetrics,
    WebhookCostFloor, WebhookDelivery, WebhookDeliveryBudget, WebhookDeliveryQueued, WebhookDryRun, WebhookEndpoint,
    WebhookEndpointCreated, WebhookEndpointSecret, WebhookEstimate, WebhookEstimateBasis,
    WebhookEstimateDayCount, WebhookEstimateDistribution, WebhookEstimateRung, WebhookEventType,
    WebhookEventTypeMetric, WebhookEventTypeParam, WebhookLimitUsage, WebhookLimits,
    WebhookPausedSubscriptions, WebhookPreviewOccurrence, WebhookPreviewWindow, WebhookRedelivery,
    WebhookReplayWindow, WebhookResume, WebhookResumeAll, WebhookResumeGap, WebhookSubscription,
    WebhookSubscriptionCondition, WebhookSubscriptionConfig, WebhookVenueFilter,
    WebhookWatchedAddress, WebhookWatchedAddressAdded, WebhookWatchedAddressList,
};
pub use webhook_signature::{
    parse_signature_header, ParsedSignature, SignatureError, WebhookVerifier,
    DEFAULT_TOLERANCE_SECS, EVENT_ID_HEADER, EVENT_TYPE_HEADER, SIGNATURE_HEADER,
};

#[cfg(feature = "websocket")]
pub use ws::{ClientMsg, OxArchiveWs, ServerMsg, WsOptions};
