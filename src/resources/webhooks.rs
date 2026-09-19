//! Webhook management: endpoints, subscriptions, watched wallets, deliveries,
//! and the two preview routes.
//!
//! All 18 routes live under `/v1/webhooks` and authenticate with the same API
//! key as the rest of the SDK.
//!
//! Verifying the deliveries that arrive is a separate job; see
//! [`crate::webhook_signature`].
//!
//! # Plan entitlements
//!
//! | Plan | Endpoints | Subscriptions | Watched wallets | Deliveries a day |
//! |---|---|---|---|---|
//! | Free | 0 | 0 | 0 | 0 |
//! | Build | 1 | 8 | 2 | 5,000 |
//! | Pro | 4 | 40 | 15 | 50,000 |
//! | Scale | 12 | 200 | 50 | 500,000 |
//! | Enterprise | negotiated | negotiated | negotiated | negotiated |
//!
//! Free has no webhook delivery at all, and a plan string this build does not
//! recognise is treated the same way. Free does keep both preview routes:
//! [`estimate`](WebhooksResource::estimate) and
//! [`dry_run`](WebhooksResource::dry_run) answer on every plan, so a rule can
//! be designed and sized before anything is paid for.
//!
//! When an account exceeds its deliveries a day, the offending subscription is
//! paused and reports that it is paused, rather than events being dropped
//! without a signal. Nothing is buffered while a subscription is paused: the
//! gap is recovered by querying the REST archive over that window.

use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{
    RotatedSecret, WatchedAddress, WatchedAddressList, WebhookDelivery, WebhookDryRun,
    WebhookEndpoint, WebhookEstimate, WebhookEventType, WebhookRedelivery, WebhookSubscription,
    WebhookTestFire,
};

const PREFIX: &str = "/v1/webhooks";

// ---------------------------------------------------------------------------
// Request parameters
// ---------------------------------------------------------------------------

/// Parameters for creating a delivery endpoint.
#[derive(Debug, Clone, Default)]
pub struct CreateEndpointParams {
    /// Where deliveries are posted. Must be reachable from the public
    /// internet: private and loopback destinations are refused at create time
    /// and again on every attempt.
    pub url: String,
    /// Your own label for the endpoint.
    pub description: Option<String>,
}

impl CreateEndpointParams {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            description: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Parameters for creating a subscription.
#[derive(Debug, Clone, Default)]
pub struct CreateSubscriptionParams {
    /// The endpoint that receives matching events.
    pub endpoint_id: String,
    /// An event type from [`WebhooksResource::event_types`]. Only types with
    /// `live: true` accept subscriptions.
    pub event_type: String,
    /// The rule: `venue`, `symbols`, `addresses`, `params` and `conditions`.
    /// Omit it to take every occurrence of the event type.
    ///
    /// Everything here is checked against the type's declaration, so a
    /// parameter or metric the type does not declare is refused rather than
    /// quietly ignored. Declared parameter defaults are filled in and the
    /// stored config is returned.
    pub filters: Option<Value>,
}

impl CreateSubscriptionParams {
    pub fn new(endpoint_id: impl Into<String>, event_type: impl Into<String>) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            event_type: event_type.into(),
            filters: None,
        }
    }

    pub fn filters(mut self, filters: Value) -> Self {
        self.filters = Some(filters);
        self
    }
}

/// Parameters for editing a subscription in place.
///
/// Both members are optional and only what is set is changed. Replacing the
/// config never requires delete and recreate.
#[derive(Debug, Clone, Default)]
pub struct UpdateSubscriptionParams {
    /// A complete replacement config, validated exactly as create validates.
    pub filters: Option<Value>,
    /// Switch the rule on or off without deleting it.
    pub enabled: Option<bool>,
}

impl UpdateSubscriptionParams {
    pub fn filters(mut self, filters: Value) -> Self {
        self.filters = Some(filters);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }
}

/// Parameters for the dry-run preview.
#[derive(Debug, Clone, Default)]
pub struct DryRunParams {
    pub event_type: String,
    /// The same config create would store. Sent on the wire as `config`.
    pub filters: Option<Value>,
    /// Seconds of history to scan, ending now. 60 to 86,400. Defaults to one
    /// hour.
    pub lookback_s: Option<i64>,
    /// Occurrences to return, newest first. 1 to 200. Defaults to 100.
    pub limit: Option<i64>,
}

impl DryRunParams {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            ..Default::default()
        }
    }

    pub fn filters(mut self, filters: Value) -> Self {
        self.filters = Some(filters);
        self
    }

    pub fn lookback_s(mut self, seconds: i64) -> Self {
        self.lookback_s = Some(seconds);
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Parameters for the estimate preview.
#[derive(Debug, Clone, Default)]
pub struct EstimateParams {
    pub event_type: String,
    /// The same config create would store. Sent on the wire as `config`.
    pub filters: Option<Value>,
    /// Days of history to evaluate, ending now. 1 to 30. Defaults to 7.
    pub lookback_days: Option<i64>,
}

impl EstimateParams {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            ..Default::default()
        }
    }

    pub fn filters(mut self, filters: Value) -> Self {
        self.filters = Some(filters);
        self
    }

    pub fn lookback_days(mut self, days: i64) -> Self {
        self.lookback_days = Some(days);
        self
    }
}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

/// Access to the webhook management endpoints (`/v1/webhooks`).
#[derive(Debug, Clone)]
pub struct WebhooksResource {
    http: HttpClient,
}

impl WebhooksResource {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self { http }
    }

    // -- Catalog ------------------------------------------------------------

    /// List the event-type catalog.
    ///
    /// This is the source of truth for what can be subscribed to and how: the
    /// parameters each type declares, the metrics conditions can be written
    /// against, and the operator vocabulary for each metric type. Subscribe
    /// time validation reads the same declarations, so a config that satisfies
    /// the catalog is a config that will be accepted.
    pub async fn event_types(&self) -> Result<Vec<WebhookEventType>> {
        let body = self.get(&format!("{PREFIX}/event-types"), &[]).await?;
        take_data(body)
    }

    // -- Endpoints ----------------------------------------------------------

    /// List delivery endpoints. The signing secret is never included here.
    pub async fn list_endpoints(&self) -> Result<Vec<WebhookEndpoint>> {
        let body = self.get(&format!("{PREFIX}/endpoints"), &[]).await?;
        take_data(body)
    }

    /// Create a delivery endpoint.
    ///
    /// The response carries `secret`, the only time it is ever returned.
    /// Store it before doing anything else: no later call will show it again,
    /// and without it you cannot verify a single delivery.
    pub async fn create_endpoint(
        &self,
        params: CreateEndpointParams,
    ) -> Result<WebhookEndpoint> {
        let mut body = Map::new();
        body.insert("url".into(), json!(params.url));
        if let Some(d) = params.description {
            body.insert("description".into(), json!(d));
        }
        let resp = self
            .post(&format!("{PREFIX}/endpoints"), &Value::Object(body))
            .await?;
        take_data(resp)
    }

    /// Delete an endpoint and everything attached to it.
    pub async fn delete_endpoint(&self, endpoint_id: &str) -> Result<()> {
        self.delete(&format!("{PREFIX}/endpoints/{}", esc(endpoint_id)))
            .await
            .map(drop)
    }

    /// Rotate an endpoint's signing secret.
    ///
    /// The new secret is returned once. The previous secret keeps verifying
    /// for 24 hours, during which every delivery carries two `v1` signatures,
    /// one per secret. Hold both in your
    /// [`WebhookVerifier`](crate::webhook_signature::WebhookVerifier) until
    /// the roll is finished, then drop the old one.
    ///
    /// Only one previous secret is ever carried: rotating twice inside the
    /// window overwrites it, and the original stops verifying immediately.
    pub async fn rotate_secret(&self, endpoint_id: &str) -> Result<RotatedSecret> {
        let resp = self
            .post(
                &format!("{PREFIX}/endpoints/{}/rotate", esc(endpoint_id)),
                &json!({}),
            )
            .await?;
        let note = resp
            .get("note")
            .and_then(|n| n.as_str())
            .map(str::to_string);
        let mut rotated: RotatedSecret = take_data(resp)?;
        if rotated.note.is_none() {
            rotated.note = note;
        }
        Ok(rotated)
    }

    /// Re-enable an endpoint after it was disabled, by you or automatically.
    ///
    /// Ten consecutive failed deliveries spanning at least six hours auto
    /// disable an endpoint. This clears the failure counters as well.
    pub async fn enable_endpoint(&self, endpoint_id: &str) -> Result<()> {
        self.post(
            &format!("{PREFIX}/endpoints/{}/enable", esc(endpoint_id)),
            &json!({}),
        )
        .await
        .map(drop)
    }

    /// Queue a `webhook.test` delivery to an endpoint.
    ///
    /// It goes through the identical dispatch path as a real event, signed the
    /// same way, so it is a genuine end to end check of your receiver and its
    /// verification code rather than a simulation.
    pub async fn test_endpoint(&self, endpoint_id: &str) -> Result<WebhookTestFire> {
        let resp = self
            .post(
                &format!("{PREFIX}/endpoints/{}/test", esc(endpoint_id)),
                &json!({}),
            )
            .await?;
        take_data(resp)
    }

    /// List recent delivery attempts for an endpoint, newest first.
    ///
    /// `limit` is clamped server side to 1 to 200 and defaults to 50.
    pub async fn deliveries(
        &self,
        endpoint_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<WebhookDelivery>> {
        let query = match limit {
            Some(l) => vec![("limit", l.to_string())],
            None => vec![],
        };
        let body = self
            .get(
                &format!("{PREFIX}/endpoints/{}/deliveries", esc(endpoint_id)),
                &query,
            )
            .await?;
        take_data(body)
    }

    /// Send a past delivery again.
    ///
    /// The event id is deliberately unchanged, so a receiver that already
    /// processed the event deduplicates it on `0xa-event-id` rather than
    /// handling it twice.
    pub async fn redeliver(&self, delivery_id: &str) -> Result<WebhookRedelivery> {
        let resp = self
            .post(
                &format!("{PREFIX}/deliveries/{}/redeliver", esc(delivery_id)),
                &json!({}),
            )
            .await?;
        take_data(resp)
    }

    // -- Subscriptions ------------------------------------------------------

    /// List every subscription across all of your endpoints.
    pub async fn list_subscriptions(&self) -> Result<Vec<WebhookSubscription>> {
        let body = self.get(&format!("{PREFIX}/subscriptions"), &[]).await?;
        take_data(body)
    }

    /// Create a subscription.
    ///
    /// The stored, normalised config comes back in the response: operator
    /// spellings are canonicalised, addresses lowercased, and declared
    /// parameter defaults filled in. Read it back rather than assuming what
    /// was stored.
    pub async fn create_subscription(
        &self,
        params: CreateSubscriptionParams,
    ) -> Result<WebhookSubscription> {
        let mut body = Map::new();
        body.insert("endpoint_id".into(), json!(params.endpoint_id));
        body.insert("event_type".into(), json!(params.event_type));
        if let Some(f) = params.filters {
            body.insert("filters".into(), f);
        }
        let resp = self
            .post(&format!("{PREFIX}/subscriptions"), &Value::Object(body))
            .await?;
        take_data(resp)
    }

    /// Edit a subscription in place.
    pub async fn update_subscription(
        &self,
        subscription_id: &str,
        params: UpdateSubscriptionParams,
    ) -> Result<WebhookSubscription> {
        let mut body = Map::new();
        if let Some(f) = params.filters {
            body.insert("filters".into(), f);
        }
        if let Some(e) = params.enabled {
            body.insert("enabled".into(), json!(e));
        }
        let resp = self
            .patch(
                &format!("{PREFIX}/subscriptions/{}", esc(subscription_id)),
                &Value::Object(body),
            )
            .await?;
        take_data(resp)
    }

    /// Delete a subscription.
    pub async fn delete_subscription(&self, subscription_id: &str) -> Result<()> {
        self.delete(&format!(
            "{PREFIX}/subscriptions/{}",
            esc(subscription_id)
        ))
        .await
        .map(drop)
    }

    /// Preview which recent occurrences a subscription would have delivered.
    ///
    /// Validated and normalised exactly as create, evaluated against real
    /// history, and nothing is written. Available on every plan, Free
    /// included.
    ///
    /// Dry-run covers fewer event types than estimate does; the refusal names
    /// the ones it supports. Both previews share a per-minute budget.
    pub async fn dry_run(&self, params: DryRunParams) -> Result<WebhookDryRun> {
        let mut body = Map::new();
        body.insert("event_type".into(), json!(params.event_type));
        // `config` is this route's own spelling of the same object create
        // calls `filters`.
        body.insert("config".into(), params.filters.unwrap_or_else(|| json!({})));
        if let Some(l) = params.lookback_s {
            body.insert("lookback_s".into(), json!(l));
        }
        if let Some(l) = params.limit {
            body.insert("limit".into(), json!(l));
        }
        let resp = self
            .post(
                &format!("{PREFIX}/subscriptions/dry-run"),
                &Value::Object(body),
            )
            .await?;
        take_data(resp)
    }

    /// Estimate how often a subscription would fire, from history.
    ///
    /// Returns the total, a per-day series, the distribution of the primary
    /// metric, and a threshold ladder: the daily rate the same rule would have
    /// had at each threshold. The ladder is how you size a rule against a
    /// plan's deliveries a day before creating it.
    ///
    /// Available on every plan, Free included, and it is the reason a Free
    /// account can design a rule it cannot yet receive.
    pub async fn estimate(&self, params: EstimateParams) -> Result<WebhookEstimate> {
        let mut body = Map::new();
        body.insert("event_type".into(), json!(params.event_type));
        body.insert("config".into(), params.filters.unwrap_or_else(|| json!({})));
        if let Some(d) = params.lookback_days {
            body.insert("lookback_days".into(), json!(d));
        }
        let resp = self
            .post(
                &format!("{PREFIX}/subscriptions/estimate"),
                &Value::Object(body),
            )
            .await?;
        take_data(resp)
    }

    // -- Watched wallets ----------------------------------------------------

    /// List the wallets this account watches, with the plan's cap.
    ///
    /// Address-scoped event types only report on wallets on this list, and a
    /// subscription cannot name an address that is not on it.
    pub async fn list_addresses(&self) -> Result<WatchedAddressList> {
        let body = self.get(&format!("{PREFIX}/addresses"), &[]).await?;
        let limit = body.get("limit").and_then(Value::as_i64);
        Ok(WatchedAddressList {
            addresses: take_data(body)?,
            limit,
        })
    }

    /// Watch a wallet.
    ///
    /// Idempotent: re-adding an address you already watch updates its label
    /// and does not count against the cap again. The address is normalised to
    /// lowercase. Hyperliquid bridge system addresses are refused, because
    /// every Core to EVM move of that token passes through them.
    pub async fn add_address(
        &self,
        address: &str,
        label: Option<&str>,
    ) -> Result<WatchedAddress> {
        let body = json!({ "address": address, "label": label.unwrap_or_default() });
        let resp = self.post(&format!("{PREFIX}/addresses"), &body).await?;
        take_data(resp)
    }

    /// Stop watching a wallet.
    pub async fn delete_address(&self, address_id: &str) -> Result<()> {
        self.delete(&format!("{PREFIX}/addresses/{}", esc(address_id)))
            .await
            .map(drop)
    }

    // -- Transport ----------------------------------------------------------

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        self.http
            .request_envelope(reqwest::Method::GET, path, query, None)
            .await
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.http
            .request_envelope(reqwest::Method::POST, path, &[], Some(body))
            .await
    }

    async fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        self.http
            .request_envelope(reqwest::Method::PATCH, path, &[], Some(body))
            .await
    }

    async fn delete(&self, path: &str) -> Result<Value> {
        self.http
            .request_envelope(reqwest::Method::DELETE, path, &[], None)
            .await
    }
}

/// Percent-encode one path segment.
fn esc(segment: &str) -> String {
    urlencoding::encode(segment).into_owned()
}

/// Pull `data` out of a management-route envelope and type it.
///
/// These routes always answer `{"success": true, "data": ...}`, so a missing
/// `data` is a real protocol surprise rather than something to paper over with
/// a default.
fn take_data<T: DeserializeOwned>(body: Value) -> Result<T> {
    let data = match body {
        Value::Object(mut map) => map.remove("data").ok_or_else(|| {
            Error::Deserialize("response envelope has no `data` member".to_string())
        })?,
        other => other,
    };
    serde_json::from_value(data).map_err(|e| Error::Deserialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WebhookEndpoint;

    #[test]
    fn take_data_unwraps_the_envelope() {
        let body = json!({"success": true, "data": [{"id": "e1", "url": "https://example.com/hooks", "description": "", "status": "active", "consecutive_failures": 0, "created_at": "2026-09-19T00:00:00Z"}]});
        let eps: Vec<WebhookEndpoint> = take_data(body).unwrap();
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].id, "e1");
        assert!(eps[0].secret.is_none(), "list never returns the secret");
    }

    #[test]
    fn take_data_reports_a_missing_data_member() {
        let err = take_data::<Vec<WebhookEndpoint>>(json!({"success": true})).unwrap_err();
        assert!(matches!(err, Error::Deserialize(ref m) if m.contains("data")), "{err}");
    }

    #[test]
    fn subscriptions_keep_unnamed_fields_rather_than_dropping_them() {
        // Pause state is not bound to typed fields yet, so it has to survive
        // in `extra` rather than vanish on deserialize.
        let body = json!({"data": {
            "id": "s1", "endpoint_id": "e1", "event_type": "account.fill",
            "filters": {"params": {"max_age_s": 3600}}, "enabled": true,
            "created_at": "2026-09-19T00:00:00Z",
            "status": "paused", "paused_at": "2026-09-19T04:00:00Z"
        }});
        let sub: WebhookSubscription = take_data(body).unwrap();
        assert_eq!(sub.event_type, "account.fill");
        assert_eq!(sub.extra.get("status").unwrap(), "paused");
        assert!(sub.extra.contains_key("paused_at"));
    }

    #[test]
    fn redelivery_accepts_both_the_short_and_the_detailed_answer() {
        let short: WebhookRedelivery = take_data(json!({"data": {"delivery_id": "d1"}})).unwrap();
        assert_eq!(short.delivery_id, "d1");
        assert!(short.event_id.is_none());

        let detailed: WebhookRedelivery = take_data(json!({"data": {
            "delivery_id": "d1", "event_id": "ev1", "event_type": "webhook.test",
            "state": "pending", "attempts": 0, "next_attempt_at": "2026-09-19T00:00:05Z"
        }}))
        .unwrap();
        assert_eq!(detailed.event_id.as_deref(), Some("ev1"));
        assert_eq!(detailed.attempts, Some(0));
    }

    #[test]
    fn params_builders_produce_the_bodies_the_routes_expect() {
        let p = CreateEndpointParams::new("https://example.com/hooks/0xarchive")
            .description("prod receiver");
        assert_eq!(p.url, "https://example.com/hooks/0xarchive");
        assert_eq!(p.description.as_deref(), Some("prod receiver"));

        let s = CreateSubscriptionParams::new("e1", "market.liquidation")
            .filters(json!({"venue": "hyperliquid"}));
        assert_eq!(s.filters.unwrap()["venue"], "hyperliquid");

        let d = DryRunParams::new("account.fill").lookback_s(3600).limit(50);
        assert_eq!(d.lookback_s, Some(3600));
        assert_eq!(d.limit, Some(50));

        let e = EstimateParams::new("market.liquidation").lookback_days(14);
        assert_eq!(e.lookback_days, Some(14));

        let u = UpdateSubscriptionParams::default().enabled(false);
        assert_eq!(u.enabled, Some(false));
        assert!(u.filters.is_none(), "an unset member must stay unsent");
    }

    #[test]
    fn path_segments_are_escaped() {
        assert_eq!(esc("00000000-0000-4000-8000-000000000000"), "00000000-0000-4000-8000-000000000000");
        assert_eq!(esc("a/b"), "a%2Fb");
    }
}
