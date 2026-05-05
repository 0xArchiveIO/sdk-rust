use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{Hip3Instrument, Instrument, LighterInstrument};

/// Hyperliquid instruments resource.
#[derive(Debug, Clone)]
pub struct InstrumentsResource {
    http: HttpClient,
    prefix: String,
}

impl InstrumentsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// List all instruments on this exchange.
    pub async fn list(&self) -> Result<Vec<Instrument>> {
        self.http
            .get(&format!("{}/instruments", self.prefix), &[])
            .await
    }

    /// Get a single instrument by coin symbol.
    pub async fn get(&self, coin: &str) -> Result<Instrument> {
        self.http
            .get(&format!("{}/instruments/{}", self.prefix, coin), &[])
            .await
    }
}

/// Lighter.xyz instruments resource (extended metadata).
#[derive(Debug, Clone)]
pub struct LighterInstrumentsResource {
    http: HttpClient,
    prefix: String,
}

impl LighterInstrumentsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// List all Lighter instruments.
    pub async fn list(&self) -> Result<Vec<LighterInstrument>> {
        self.http
            .get(&format!("{}/instruments", self.prefix), &[])
            .await
    }

    /// Get a single Lighter instrument by symbol.
    pub async fn get(&self, coin: &str) -> Result<LighterInstrument> {
        self.http
            .get(&format!("{}/instruments/{}", self.prefix, coin), &[])
            .await
    }
}

/// HIP-3 instruments resource (case-sensitive symbols like `km:US500`).
#[derive(Debug, Clone)]
pub struct Hip3InstrumentsResource {
    http: HttpClient,
    prefix: String,
}

impl Hip3InstrumentsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// List all HIP-3 instruments.
    pub async fn list(&self) -> Result<Vec<Hip3Instrument>> {
        self.http
            .get(&format!("{}/instruments", self.prefix), &[])
            .await
    }

    /// Get a single HIP-3 instrument by coin (case-sensitive).
    pub async fn get(&self, coin: &str) -> Result<Hip3Instrument> {
        self.http
            .get(&format!("{}/instruments/{}", self.prefix, coin), &[])
            .await
    }
}

/// HIP-4 instruments resource. Returns per-side outcome rows (`#0`, `#1`, ...).
///
/// The user-facing API takes the bare numeric form. The backend accepts the
/// bare form, but raw `#` is the URL fragment delimiter per RFC 3986, so the
/// SDK percent-encodes `#` to `%23` strictly for wire transport. Always pass
/// the bare form (`"#0"`) in your code.
#[derive(Debug, Clone)]
pub struct Hip4InstrumentsResource {
    http: HttpClient,
    prefix: String,
}

impl Hip4InstrumentsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// List every per-side HIP-4 outcome instrument.
    pub async fn list(&self) -> Result<Vec<crate::types::Hip4Outcome>> {
        self.http
            .get(&format!("{}/instruments", self.prefix), &[])
            .await
    }

    /// Get a single HIP-4 instrument by coin symbol (e.g. `"#0"`). Pass the
    /// bare form; the SDK percent-encodes `#` for the URL wire path only.
    pub async fn get(&self, symbol: &str) -> Result<crate::types::Hip4Outcome> {
        let encoded = urlencoding::encode(symbol);
        self.http
            .get(&format!("{}/instruments/{}", self.prefix, encoded), &[])
            .await
    }
}
