//! Webhook management: the event catalog, plan limits, endpoints, deliveries,
//! subscriptions, the two previews and watched wallets.
//!
//! All 21 routes live under `/v1/webhooks`, authenticate with the same API key
//! as the rest of the SDK, and cost no credits. The estimate and the dry-run
//! are metered like the market data they return.
//!
//! Verifying the deliveries that arrive is a separate job; see
//! [`crate::webhook_signature`].
//!
//! # Plan limits
//!
//! | Plan | Endpoints | Subscriptions | Watched wallets | Deliveries a day |
//! |---|---|---|---|---|
//! | Free | Not available | Not available | Not available | Not available |
//! | Build | 1 | 8 | 2 | 5,000 |
//! | Pro | 4 | 40 | 15 | 50,000 |
//! | Scale | 12 | 200 | 50 | 500,000 |
//! | Enterprise | Custom | Custom | Custom | Custom |
//!
//! [`limits`](WebhooksResource::limits) returns the caps of your own plan
//! with what is in use against each. Free has no webhook delivery, but it
//! keeps both previews: [`estimate`](WebhooksResource::estimate) and
//! [`dry_run`](WebhooksResource::dry_run) answer on every plan, so a rule can
//! be designed and sized before there is anywhere to deliver it.
//!
//! When an account reaches its deliveries a day, the subscription that
//! crossed the line is paused and says so
//! ([`WebhookSubscription::is_paused`](crate::types::WebhookSubscription::is_paused)),
//! rather than events being dropped without a signal. Nothing is buffered
//! while a rule is paused. A pause at the daily limit lasts until the rule is
//! resumed with [`resume_subscription`](WebhooksResource::resume_subscription)
//! or [`resume_all_subscriptions`](WebhooksResource::resume_all_subscriptions),
//! and the resume hands back the missed window, which can be re-read from the
//! REST routes.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{
    WebhookDelivery, WebhookDeliveryQueued, WebhookDryRun, WebhookEndpoint, WebhookEndpointCreated,
    WebhookEndpointSecret, WebhookEstimate, WebhookEventType, WebhookLimits, WebhookRedelivery,
    WebhookResume, WebhookResumeAll, WebhookResumeGap, WebhookSubscription,
    WebhookSubscriptionConfig, WebhookWatchedAddress, WebhookWatchedAddressAdded,
    WebhookWatchedAddressList,
};

const PREFIX: &str = "/v1/webhooks";

// ---------------------------------------------------------------------------
// Request parameters
// ---------------------------------------------------------------------------

/// Parameters for [`WebhooksResource::create_endpoint`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CreateEndpointParams {
    /// HTTPS destination that will receive deliveries. Destinations that
    /// resolve to a private or internal address are refused, at creation and
    /// again on every delivery.
    pub url: String,
    /// Your own label for the endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl CreateEndpointParams {
    /// An endpoint delivering to `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            description: None,
        }
    }

    /// Set your own label for the endpoint.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Parameters for [`WebhooksResource::create_subscription`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CreateSubscriptionParams {
    /// Endpoint that will receive the rule's deliveries.
    pub endpoint_id: String,
    /// Event type from [`WebhooksResource::event_types`]. Only types with
    /// `live: true` accept subscriptions.
    pub event_type: String,
    /// What the rule matches on. `None` takes every occurrence of the event
    /// type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<WebhookSubscriptionConfig>,
}

impl CreateSubscriptionParams {
    /// A rule delivering `event_type` to `endpoint_id`.
    pub fn new(endpoint_id: impl Into<String>, event_type: impl Into<String>) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            event_type: event_type.into(),
            filters: None,
        }
    }

    /// Set what the rule matches on.
    pub fn filters(mut self, filters: WebhookSubscriptionConfig) -> Self {
        self.filters = Some(filters);
        self
    }
}

/// Parameters for [`WebhooksResource::update_subscription`].
///
/// Only what is set is sent, and a field left out is left alone. A config
/// that is sent replaces the stored one and is validated as at create.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateSubscriptionParams {
    /// A complete replacement config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<WebhookSubscriptionConfig>,
    /// Switch the rule on or off without touching its config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl UpdateSubscriptionParams {
    /// Replace the stored config.
    pub fn filters(mut self, filters: WebhookSubscriptionConfig) -> Self {
        self.filters = Some(filters);
        self
    }

    /// Switch the rule on or off.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }
}

/// Parameters for [`WebhooksResource::dry_run`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DryRunParams {
    /// Event type to evaluate.
    pub event_type: String,
    /// The config a create would store. `None` sends an empty config, which
    /// matches every occurrence.
    pub config: Option<WebhookSubscriptionConfig>,
    /// Seconds of history to scan, ending now. The API accepts 60 to 86,400
    /// and defaults to 3,600.
    pub lookback_s: Option<i64>,
    /// Occurrences to return, newest first. The API accepts 1 to 200 and
    /// defaults to 100.
    pub limit: Option<i64>,
}

impl DryRunParams {
    /// A dry-run of `event_type`.
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            ..Default::default()
        }
    }

    /// Set the config to evaluate.
    pub fn config(mut self, config: WebhookSubscriptionConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the seconds of history to scan.
    pub fn lookback_s(mut self, seconds: i64) -> Self {
        self.lookback_s = Some(seconds);
        self
    }

    /// Set how many occurrences to return.
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    fn body(&self) -> Value {
        let mut body = preview_body(&self.event_type, self.config.as_ref());
        if let Some(l) = self.lookback_s {
            body.insert("lookback_s".into(), json!(l));
        }
        if let Some(l) = self.limit {
            body.insert("limit".into(), json!(l));
        }
        Value::Object(body)
    }
}

/// Parameters for [`WebhooksResource::estimate`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EstimateParams {
    /// Event type to evaluate.
    pub event_type: String,
    /// The config a create would store. `None` sends an empty config, which
    /// matches every occurrence.
    pub config: Option<WebhookSubscriptionConfig>,
    /// Days of history to evaluate, ending now. The API accepts 1 to 30 and
    /// defaults to 7.
    pub lookback_days: Option<i64>,
}

impl EstimateParams {
    /// An estimate of `event_type`.
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            ..Default::default()
        }
    }

    /// Set the config to evaluate.
    pub fn config(mut self, config: WebhookSubscriptionConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the days of history to evaluate.
    pub fn lookback_days(mut self, days: i64) -> Self {
        self.lookback_days = Some(days);
        self
    }

    fn body(&self) -> Value {
        let mut body = preview_body(&self.event_type, self.config.as_ref());
        if let Some(d) = self.lookback_days {
            body.insert("lookback_days".into(), json!(d));
        }
        Value::Object(body)
    }
}

/// The body both previews share: the event type and the config, which the
/// preview routes call `config` where create calls it `filters`.
fn preview_body(
    event_type: &str,
    config: Option<&WebhookSubscriptionConfig>,
) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("event_type".into(), json!(event_type));
    let config = match config {
        Some(c) => serde_json::to_value(c).unwrap_or_else(|_| json!({})),
        None => json!({}),
    };
    body.insert("config".into(), config);
    body
}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

/// Access to the webhook management routes (`/v1/webhooks`), as
/// `client.webhooks`.
#[derive(Debug, Clone)]
pub struct WebhooksResource {
    http: HttpClient,
}

impl WebhooksResource {
    pub(crate) fn new(http: HttpClient) -> Self {
        Self { http }
    }

    // -- Catalog and limits -------------------------------------------------

    /// List the event catalog (`GET /v1/webhooks/event-types`).
    ///
    /// One declaration per event type: its scope, covered venues, accepted
    /// filters, tunable parameters, the metrics conditions may be written
    /// against, the operator vocabulary and the smallest occurrence it
    /// reports. Subscriptions are validated against these declarations.
    pub async fn event_types(&self) -> Result<Vec<WebhookEventType>> {
        let body = self.get(&format!("{PREFIX}/event-types"), &[]).await?;
        take_data(body)
    }

    /// Get the plan's webhook caps and current usage
    /// (`GET /v1/webhooks/limits`).
    ///
    /// Endpoints, subscriptions, watched wallets, today's delivery budget and
    /// how many subscriptions are paused, read from the same place the caps
    /// are enforced from.
    pub async fn limits(&self) -> Result<WebhookLimits> {
        let body = self.get(&format!("{PREFIX}/limits"), &[]).await?;
        take_data(body)
    }

    // -- Endpoints ----------------------------------------------------------

    /// List delivery endpoints, oldest first (`GET /v1/webhooks/endpoints`).
    /// The signing secret is never included.
    pub async fn list_endpoints(&self) -> Result<Vec<WebhookEndpoint>> {
        let body = self.get(&format!("{PREFIX}/endpoints"), &[]).await?;
        take_data(body)
    }

    /// Create a delivery endpoint (`POST /v1/webhooks/endpoints`).
    ///
    /// The response carries the signing secret, and this and
    /// [`rotate_secret`](Self::rotate_secret) are the only calls that ever
    /// return one. Store it before doing anything else. A plan without
    /// webhook delivery, or one at its endpoint cap, is refused with the
    /// reason.
    pub async fn create_endpoint(
        &self,
        params: CreateEndpointParams,
    ) -> Result<WebhookEndpointCreated> {
        let body = self
            .post(&format!("{PREFIX}/endpoints"), Some(&to_body(&params)?))
            .await?;
        let note = sibling_str(&body, "note");
        let mut created: WebhookEndpointCreated = take_data(body)?;
        if created.note.is_none() {
            created.note = note;
        }
        Ok(created)
    }

    /// Delete an endpoint and every subscription pointing at it
    /// (`DELETE /v1/webhooks/endpoints/{id}`).
    pub async fn delete_endpoint(&self, endpoint_id: &str) -> Result<()> {
        self.delete(&format!("{PREFIX}/endpoints/{}", esc(endpoint_id)))
            .await
            .map(drop)
    }

    /// Put an endpoint back into service after you switched it off, or after
    /// a long run of failed deliveries switched it off
    /// (`POST /v1/webhooks/endpoints/{id}/enable`).
    ///
    /// Deliveries resume on the next matching event; nothing that happened
    /// while it was off is replayed.
    pub async fn enable_endpoint(&self, endpoint_id: &str) -> Result<()> {
        self.post(
            &format!("{PREFIX}/endpoints/{}/enable", esc(endpoint_id)),
            None,
        )
        .await
        .map(drop)
    }

    /// Issue a new signing secret for an endpoint
    /// (`POST /v1/webhooks/endpoints/{id}/rotate`).
    ///
    /// The previous secret keeps verifying for 24 hours, and every delivery
    /// in that window carries a `v1` signature for each secret. Hold both in
    /// your [`WebhookVerifier`](crate::webhook_signature::WebhookVerifier)
    /// until the roll is finished, then drop the old one. Only one previous
    /// secret is kept: rotating again inside the window replaces it, and the
    /// original stops verifying.
    pub async fn rotate_secret(&self, endpoint_id: &str) -> Result<WebhookEndpointSecret> {
        let body = self
            .post(
                &format!("{PREFIX}/endpoints/{}/rotate", esc(endpoint_id)),
                None,
            )
            .await?;
        let note = sibling_str(&body, "note");
        let mut rotated: WebhookEndpointSecret = take_data(body)?;
        if rotated.note.is_none() {
            rotated.note = note;
        }
        Ok(rotated)
    }

    /// Queue a `webhook.test` event to an endpoint
    /// (`POST /v1/webhooks/endpoints/{id}/test`).
    ///
    /// It is a real signed delivery through the same path as any other
    /// event, so it checks your receiver and its verification end to end. It
    /// counts against today's delivery budget, and it is refused on a plan
    /// without webhook delivery or once that budget is spent (HTTP 409).
    pub async fn test_endpoint(&self, endpoint_id: &str) -> Result<WebhookDeliveryQueued> {
        let body = self
            .post(
                &format!("{PREFIX}/endpoints/{}/test", esc(endpoint_id)),
                None,
            )
            .await?;
        take_data(body)
    }

    // -- Deliveries ---------------------------------------------------------

    /// List an endpoint's delivery log, newest first
    /// (`GET /v1/webhooks/endpoints/{id}/deliveries`).
    ///
    /// `limit` defaults to 50; values outside 1 to 200 are clamped by the
    /// API.
    pub async fn list_deliveries(
        &self,
        endpoint_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<WebhookDelivery>> {
        let mut query = vec![];
        if let Some(l) = limit {
            query.push(("limit", l.to_string()));
        }
        let body = self
            .get(
                &format!("{PREFIX}/endpoints/{}/deliveries", esc(endpoint_id)),
                &query,
            )
            .await?;
        take_data(body)
    }

    /// Queue a past delivery for another attempt
    /// (`POST /v1/webhooks/deliveries/{id}/redeliver`).
    ///
    /// The delivery and event identifiers are unchanged, so a receiver that
    /// already processed the event deduplicates it on `0xa-event-id`. It
    /// counts against today's delivery budget, and it is refused (HTTP 409)
    /// when the endpoint is switched off or the budget is spent.
    pub async fn redeliver(&self, delivery_id: &str) -> Result<WebhookRedelivery> {
        let body = self
            .post(
                &format!("{PREFIX}/deliveries/{}/redeliver", esc(delivery_id)),
                None,
            )
            .await?;
        let note = sibling_str(&body, "note");
        let mut redelivery: WebhookRedelivery = take_data(body)?;
        if redelivery.note.is_none() {
            redelivery.note = note;
        }
        Ok(redelivery)
    }

    // -- Subscriptions ------------------------------------------------------

    /// List every subscription across your endpoints, with its config and
    /// pause state (`GET /v1/webhooks/subscriptions`).
    pub async fn list_subscriptions(&self) -> Result<Vec<WebhookSubscription>> {
        let body = self.get(&format!("{PREFIX}/subscriptions"), &[]).await?;
        take_data(body)
    }

    /// Create a subscription (`POST /v1/webhooks/subscriptions`).
    ///
    /// The config is checked against the event type's catalog declaration
    /// before anything is stored, and the stored, normalized config comes
    /// back in the response. Addresses in the config must already be on your
    /// watched list.
    pub async fn create_subscription(
        &self,
        params: CreateSubscriptionParams,
    ) -> Result<WebhookSubscription> {
        let body = self
            .post(&format!("{PREFIX}/subscriptions"), Some(&to_body(&params)?))
            .await?;
        take_data(body)
    }

    /// Edit a subscription in place
    /// (`PATCH /v1/webhooks/subscriptions/{id}`).
    pub async fn update_subscription(
        &self,
        subscription_id: &str,
        params: UpdateSubscriptionParams,
    ) -> Result<WebhookSubscription> {
        let body = self
            .patch(
                &format!("{PREFIX}/subscriptions/{}", esc(subscription_id)),
                &to_body(&params)?,
            )
            .await?;
        take_data(body)
    }

    /// Delete a subscription (`DELETE /v1/webhooks/subscriptions/{id}`).
    /// Its endpoint and other subscriptions are untouched.
    pub async fn delete_subscription(&self, subscription_id: &str) -> Result<()> {
        self.delete(&format!("{PREFIX}/subscriptions/{}", esc(subscription_id)))
            .await
            .map(drop)
    }

    /// Put one paused subscription back into service
    /// (`POST /v1/webhooks/subscriptions/{id}/resume`).
    ///
    /// The result carries the window the pause missed (`gap`), which can be
    /// re-read from the REST routes. A subscription that is already serving
    /// is left as it is, with `gap` `None` and a `note`. The request is
    /// refused (HTTP 409) when resuming would be undone at once because
    /// today's budget is spent. Your own `enabled` switch is never touched.
    pub async fn resume_subscription(&self, subscription_id: &str) -> Result<WebhookResume> {
        let body = self
            .post(
                &format!("{PREFIX}/subscriptions/{}/resume", esc(subscription_id)),
                None,
            )
            .await?;
        let gap = sibling::<WebhookResumeGap>(&body, "gap")?;
        let note = sibling_str(&body, "note");
        Ok(WebhookResume {
            subscription: take_data(body)?,
            gap,
            note,
        })
    }

    /// Put every paused subscription on the account back into service
    /// (`POST /v1/webhooks/subscriptions/resume`).
    ///
    /// The daily delivery limit is counted per account, so one busy rule can
    /// pause all of them; this brings them back in one call. When nothing is
    /// paused nothing changes, `resumed_count` is 0 and `note` says so.
    pub async fn resume_all_subscriptions(&self) -> Result<WebhookResumeAll> {
        let body = self
            .post(&format!("{PREFIX}/subscriptions/resume"), None)
            .await?;
        let gap = sibling::<WebhookResumeGap>(&body, "gap")?;
        let note = sibling_str(&body, "note");
        let resumed_count = body.get("resumed_count").and_then(Value::as_i64);
        let subscriptions: Vec<WebhookSubscription> = take_data(body)?;
        Ok(WebhookResumeAll {
            resumed_count: resumed_count.unwrap_or(subscriptions.len() as i64),
            subscriptions,
            gap,
            note,
        })
    }

    // -- Previews -----------------------------------------------------------

    /// Preview which recent occurrences a would-be subscription would have
    /// delivered (`POST /v1/webhooks/subscriptions/dry-run`).
    ///
    /// The config is validated and normalized as at create, then evaluated
    /// against the last `lookback_s` seconds. Nothing is stored and nothing
    /// is sent. Available on every plan, Free included, for the event types
    /// the API lists in its refusal (currently `account.fill`,
    /// `account.transfer` and `market.liquidation`). Estimates and dry-runs
    /// share a budget of six a minute per account.
    pub async fn dry_run(&self, params: DryRunParams) -> Result<WebhookDryRun> {
        let body = self
            .post(
                &format!("{PREFIX}/subscriptions/dry-run"),
                Some(&params.body()),
            )
            .await?;
        take_data(body)
    }

    /// Estimate how often a would-be subscription would have fired
    /// (`POST /v1/webhooks/subscriptions/estimate`).
    ///
    /// Returns the total, a count per day, the median and busiest day, the
    /// distribution of the event's primary metric, a ladder of the daily
    /// rate at other thresholds and a sample of real matches, so a threshold
    /// can be chosen against history. Nothing is stored and nothing is sent.
    /// Available on every plan, Free included.
    pub async fn estimate(&self, params: EstimateParams) -> Result<WebhookEstimate> {
        let body = self
            .post(
                &format!("{PREFIX}/subscriptions/estimate"),
                Some(&params.body()),
            )
            .await?;
        take_data(body)
    }

    // -- Watched wallets ----------------------------------------------------

    /// List the watched wallets with the plan's cap
    /// (`GET /v1/webhooks/addresses`).
    ///
    /// Address scoped event types report only on wallets on this list.
    pub async fn list_addresses(&self) -> Result<WebhookWatchedAddressList> {
        let body = self.get(&format!("{PREFIX}/addresses"), &[]).await?;
        let limit = body.get("limit").and_then(Value::as_i64);
        Ok(WebhookWatchedAddressList {
            addresses: take_data(body)?,
            limit,
        })
    }

    /// Add a wallet to the watched list (`POST /v1/webhooks/addresses`).
    ///
    /// `address` is a `0x` prefixed, 40 hex character wallet and is stored
    /// lowercase; `label` is at most 64 characters. Adding a wallet that is
    /// already watched changes nothing and does not count against the cap
    /// again. Hyperliquid bridge system addresses are refused.
    pub async fn add_address(
        &self,
        address: &str,
        label: Option<&str>,
    ) -> Result<WebhookWatchedAddressAdded> {
        let mut request = Map::new();
        request.insert("address".into(), json!(address));
        if let Some(l) = label {
            request.insert("label".into(), json!(l));
        }
        let body = self
            .post(
                &format!("{PREFIX}/addresses"),
                Some(&Value::Object(request)),
            )
            .await?;
        let limit = body.get("limit").and_then(Value::as_i64);
        let address: WebhookWatchedAddress = take_data(body)?;
        Ok(WebhookWatchedAddressAdded { address, limit })
    }

    /// Remove a wallet from the watched list
    /// (`DELETE /v1/webhooks/addresses/{id}`).
    ///
    /// Subscriptions that named it keep their stored config, so remove the
    /// address from those rules too if they should no longer reference it.
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

    async fn post(&self, path: &str, body: Option<&Value>) -> Result<Value> {
        self.http
            .request_envelope(reqwest::Method::POST, path, &[], body)
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

/// Serialize a request body.
fn to_body<T: Serialize>(params: &T) -> Result<Value> {
    serde_json::to_value(params).map_err(|e| Error::InvalidParam(e.to_string()))
}

/// Pull `data` out of a management-route response and type it.
///
/// These routes answer `{"success": true, "data": ...}`, so a missing `data`
/// is reported rather than papered over with a default.
fn take_data<T: DeserializeOwned>(body: Value) -> Result<T> {
    let data = match body {
        Value::Object(mut map) => map
            .remove("data")
            .ok_or_else(|| Error::Deserialize("response has no `data` member".to_string()))?,
        other => other,
    };
    serde_json::from_value(data).map_err(|e| Error::Deserialize(e.to_string()))
}

/// A string member next to `data`, such as `note`.
fn sibling_str(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A typed member next to `data`, such as `gap`. `null` and absent are
/// `None`.
fn sibling<T: DeserializeOwned>(body: &Value, key: &str) -> Result<Option<T>> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| Error::Deserialize(format!("`{key}`: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{WebhookSubscriptionCondition, WebhookVenueFilter};

    #[test]
    fn take_data_unwraps_the_envelope() {
        let body = json!({"success": true, "data": [{
            "id": "e1", "url": "https://example.com/hooks", "description": "",
            "status": "active", "consecutive_failures": 0,
            "created_at": "2026-09-19T00:00:00Z"
        }]});
        let eps: Vec<WebhookEndpoint> = take_data(body).unwrap();
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].id, "e1");
    }

    #[test]
    fn take_data_reports_a_missing_data_member() {
        let err = take_data::<Vec<WebhookEndpoint>>(json!({"success": true})).unwrap_err();
        assert!(
            matches!(err, Error::Deserialize(ref m) if m.contains("data")),
            "{err}"
        );
    }

    #[test]
    fn sibling_treats_null_as_none() {
        let body = json!({"data": {}, "gap": null, "note": "x"});
        assert!(sibling::<WebhookResumeGap>(&body, "gap").unwrap().is_none());
        assert!(sibling::<WebhookResumeGap>(&body, "absent")
            .unwrap()
            .is_none());
        assert_eq!(sibling_str(&body, "note").as_deref(), Some("x"));
    }

    #[test]
    fn previews_always_send_a_config_object() {
        let bare = DryRunParams::new("market.liquidation").body();
        assert_eq!(
            bare,
            json!({"event_type": "market.liquidation", "config": {}})
        );

        let full = DryRunParams::new("market.liquidation")
            .config(WebhookSubscriptionConfig::default().venue("hyperliquid"))
            .lookback_s(3600)
            .limit(5)
            .body();
        assert_eq!(
            full,
            json!({
                "event_type": "market.liquidation",
                "config": {"venue": "hyperliquid"},
                "lookback_s": 3600,
                "limit": 5
            })
        );

        let est = EstimateParams::new("account.fill").lookback_days(14).body();
        assert_eq!(
            est,
            json!({"event_type": "account.fill", "config": {}, "lookback_days": 14})
        );
    }

    #[test]
    fn create_and_update_bodies_leave_out_what_is_unset() {
        let p = CreateEndpointParams::new("https://example.com/hooks");
        assert_eq!(
            to_body(&p).unwrap(),
            json!({"url": "https://example.com/hooks"})
        );

        let s = CreateSubscriptionParams::new("e1", "market.liquidation");
        assert_eq!(
            to_body(&s).unwrap(),
            json!({"endpoint_id": "e1", "event_type": "market.liquidation"})
        );

        let u = UpdateSubscriptionParams::default().enabled(false);
        assert_eq!(to_body(&u).unwrap(), json!({"enabled": false}));
    }

    #[test]
    fn a_config_round_trips_through_the_wire_shape() {
        let config = WebhookSubscriptionConfig::default()
            .venues(["hyperliquid", "hip3"])
            .symbols(["BTC"])
            .param("max_age_s", 3600)
            .condition(WebhookSubscriptionCondition::new(
                "notional_usd",
                "between",
                json!([100_000, 500_000]),
            ))
            .condition(WebhookSubscriptionCondition::without_value(
                "direction",
                "is_not_empty",
            ));
        let wire = serde_json::to_value(&config).unwrap();
        assert_eq!(
            wire,
            json!({
                "venue": ["hyperliquid", "hip3"],
                "symbols": ["BTC"],
                "params": {"max_age_s": 3600},
                "conditions": [
                    {"metric": "notional_usd", "op": "between", "value": [100000, 500000]},
                    {"metric": "direction", "op": "is_not_empty"}
                ]
            })
        );
        let back: WebhookSubscriptionConfig = serde_json::from_value(wire).unwrap();
        assert_eq!(back, config);

        // A declared parameter written at the top level survives in `extra`.
        let top: WebhookSubscriptionConfig =
            serde_json::from_value(json!({"venue": "hyperliquid", "window_s": 300})).unwrap();
        assert_eq!(
            top.venue,
            Some(WebhookVenueFilter::One("hyperliquid".into()))
        );
        assert_eq!(top.extra.get("window_s"), Some(&json!(300)));
    }

    #[test]
    fn path_segments_are_escaped() {
        assert_eq!(
            esc("00000000-0000-4000-8000-000000000000"),
            "00000000-0000-4000-8000-000000000000"
        );
        assert_eq!(esc("a/b"), "a%2Fb");
    }
}
