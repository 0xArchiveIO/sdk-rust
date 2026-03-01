use crate::error::Result;
use crate::http::HttpClient;
use crate::types::{
    SiweChallenge, Web3KeysList, Web3RevokeResult, Web3SignupResult, Web3SubscribeResult,
};

/// Access to web3 wallet-based authentication endpoints.
#[derive(Debug, Clone)]
pub struct Web3Resource {
    http: HttpClient,
}

impl Web3Resource {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Request a SIWE (Sign-In with Ethereum) challenge message.
    pub async fn challenge(&self, address: &str) -> Result<SiweChallenge> {
        let body = serde_json::json!({ "address": address });
        self.http.post("/v1/auth/web3/challenge", &body).await
    }

    /// Create a free-tier account using a signed SIWE message.
    pub async fn signup(&self, message: &str, signature: &str) -> Result<Web3SignupResult> {
        let body = serde_json::json!({
            "message": message,
            "signature": signature,
        });
        self.http.post("/v1/web3/signup", &body).await
    }

    /// List API keys for the authenticated wallet.
    pub async fn list_keys(&self, message: &str, signature: &str) -> Result<Web3KeysList> {
        let body = serde_json::json!({
            "message": message,
            "signature": signature,
        });
        self.http.post("/v1/web3/keys", &body).await
    }

    /// Revoke an API key.
    pub async fn revoke_key(
        &self,
        message: &str,
        signature: &str,
        key_id: &str,
    ) -> Result<Web3RevokeResult> {
        let body = serde_json::json!({
            "message": message,
            "signature": signature,
            "keyId": key_id,
        });
        self.http.post("/v1/web3/keys/revoke", &body).await
    }

    /// Subscribe to a paid tier via x402 crypto payment.
    pub async fn subscribe(
        &self,
        tier: &str,
        payment_signature: &str,
    ) -> Result<Web3SubscribeResult> {
        let body = serde_json::json!({
            "tier": tier,
            "paymentSignature": payment_signature,
        });
        self.http.post("/v1/web3/subscribe", &body).await
    }
}
