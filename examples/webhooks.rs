//! Webhooks end to end: size a rule, point it somewhere, verify what arrives.
//!
//! The estimate at the top runs on any plan, Free included. Everything after
//! it needs a plan that includes webhook delivery (Build and up), so the
//! example prints the refusal and stops rather than failing if yours does not.
//!
//! Run with:
//!   export OXARCHIVE_API_KEY="your-api-key"
//!   cargo run --example webhooks

use oxarchive::webhook_signature::WebhookVerifier;
use oxarchive::{CreateEndpointParams, CreateSubscriptionParams, EstimateParams, Error, OxArchive};
use serde_json::json;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");
    let client = OxArchive::new(api_key)?;

    // 1. What can be subscribed to.
    let catalog = client.webhooks.event_types().await?;
    let live = catalog.iter().filter(|e| e.live).count();
    println!("catalog: {} event types, {live} live", catalog.len());

    // 2. How often would this rule have fired? Free plans get this too, which
    //    is the point: size the rule before paying for the deliveries.
    let rule = json!({
        "venue": "hyperliquid",
        "conditions": [
            {"metric": "notional_usd", "op": "greater_than_or_equal", "value": 250_000}
        ]
    });

    let estimate = client
        .webhooks
        .estimate(
            EstimateParams::new("market.liquidation")
                .filters(rule.clone())
                .lookback_days(7),
        )
        .await?;

    println!(
        "\nmarket.liquidation over {} days: {} events, median {:.1}/day, busiest {}/day",
        estimate.days, estimate.total, estimate.per_day_p50, estimate.per_day_max
    );
    if !estimate.ladder.is_empty() {
        println!("  the same rule at other thresholds:");
        for rung in &estimate.ladder {
            println!("    {:>14.0} -> {:.1}/day", rung.value, rung.per_day);
        }
    }

    // 3. Point it somewhere. Free stops here with a refusal that says why.
    let endpoint = match client
        .webhooks
        .create_endpoint(
            CreateEndpointParams::new("https://example.com/hooks/0xarchive")
                .description("oxarchive example"),
        )
        .await
    {
        Ok(ep) => ep,
        Err(Error::Api { message, code, .. }) => {
            println!("\ncould not create an endpoint (HTTP {code}): {message}");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // The secret is returned here and on rotate, and nowhere else, ever.
    let secret = endpoint
        .secret
        .clone()
        .expect("create returns the signing secret once");
    println!("\nendpoint {} created", endpoint.id);
    println!("  secret starts {}... store it now", &secret[..12.min(secret.len())]);

    let subscription = client
        .webhooks
        .create_subscription(
            CreateSubscriptionParams::new(&endpoint.id, "market.liquidation").filters(rule),
        )
        .await?;
    println!("subscription {} created", subscription.id);
    // The stored config is normalised: ">=" comes back canonicalised and every
    // declared parameter default is filled in.
    println!("  stored config: {}", subscription.filters);

    // 4. Fire a real signed delivery at the endpoint and watch the log.
    let fired = client.webhooks.test_endpoint(&endpoint.id).await?;
    println!("\ntest delivery {} queued (event {})", fired.delivery_id, fired.event_id);

    for attempt in client.webhooks.deliveries(&endpoint.id, Some(5)).await? {
        println!(
            "  {} {} attempts={} status={:?}",
            attempt.created_at, attempt.state, attempt.attempts, attempt.last_status_code
        );
    }

    // 5. What the receiving side does with it. Hash the bytes as received:
    //    re-serialising the JSON changes them and verification fails.
    let verifier = WebhookVerifier::new(&secret);
    println!(
        "\nreceiver: verify(raw_body, header) with a {}s replay window, \
         then deduplicate on 0xa-event-id and answer 2xx within 10s",
        verifier.tolerance()
    );

    // Tidy up so re-running the example does not exhaust the plan's endpoints.
    client.webhooks.delete_endpoint(&endpoint.id).await?;
    println!("endpoint {} deleted", endpoint.id);

    Ok(())
}
