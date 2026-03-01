/// The main entry point for the 0xArchive SDK.

use std::time::Duration;

use crate::error;
use crate::exchanges::{HyperliquidClient, LighterClient};
use crate::http::{HttpClient, HttpConfig};
use crate::resources::{DataQualityResource, Web3Resource};

/// Default base URL for the 0xArchive API.
pub const DEFAULT_BASE_URL: &str = "https://api.0xarchive.io";

/// Default request timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for building an [`OxArchive`] client.
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
}

impl ClientBuilder {
    /// Create a new builder with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Override the base URL (e.g. for local development).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the client.
    pub fn build(self) -> error::Result<OxArchive> {
        let http = HttpClient::new(HttpConfig {
            base_url: self.base_url,
            api_key: self.api_key,
            timeout: self.timeout,
        })?;

        Ok(OxArchive {
            hyperliquid: HyperliquidClient::new(http.clone()),
            lighter: LighterClient::new(http.clone()),
            data_quality: DataQualityResource::new(http.clone()),
            web3: Web3Resource::new(http),
        })
    }
}

/// The 0xArchive API client.
///
/// # Quick start
///
/// ```no_run
/// use oxarchive::OxArchive;
///
/// # async fn example() -> oxarchive::Result<()> {
/// let client = OxArchive::new("your-api-key")?;
///
/// // List Hyperliquid instruments
/// let instruments = client.hyperliquid.instruments.list().await?;
///
/// // Get BTC orderbook
/// let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
/// println!("BTC mid price: {:?}", ob.mid_price);
///
/// // Access Lighter.xyz
/// let lighter_instruments = client.lighter.instruments.list().await?;
///
/// // Access HIP-3 builder perps
/// let hip3_instruments = client.hyperliquid.hip3.instruments.list().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OxArchive {
    /// Hyperliquid exchange client (includes nested `.hip3` client).
    pub hyperliquid: HyperliquidClient,
    /// Lighter.xyz exchange client.
    pub lighter: LighterClient,
    /// Data quality monitoring.
    pub data_quality: DataQualityResource,
    /// Web3 wallet-based authentication.
    pub web3: Web3Resource,
}

impl OxArchive {
    /// Create a new client with default settings.
    ///
    /// For advanced configuration, use [`OxArchive::builder`].
    pub fn new(api_key: impl Into<String>) -> error::Result<Self> {
        ClientBuilder::new(api_key).build()
    }

    /// Create a builder for advanced configuration.
    ///
    /// ```no_run
    /// use oxarchive::OxArchive;
    /// use std::time::Duration;
    ///
    /// let client = OxArchive::builder("your-api-key")
    ///     .base_url("https://api.0xarchive.io")
    ///     .timeout(Duration::from_secs(60))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(api_key)
    }
}
