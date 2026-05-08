use oxarchive::resources::spot::SpotTwapParams;
use oxarchive::resources::trades::GetTradesParams;
use oxarchive::OxArchive;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");
    let client = OxArchive::new(api_key)?;

    // List every spot pair (dashed canonical: HYPE-USDC, PURR-USDC, ...).
    let pairs = client.hyperliquid.spot.pairs.list().await?;
    println!("Spot: {} pairs", pairs.len());
    for p in pairs.iter().take(5) {
        println!("  {} (mark={:?}, mid={:?})", p.symbol, p.mark_price, p.mid_price);
    }

    // Get a single pair detail.
    let hype = client.hyperliquid.spot.pairs.get("HYPE-USDC").await?;
    println!("\nHYPE-USDC: base={:?}, quote={:?}", hype.base, hype.quote);

    // Current L2 orderbook for HYPE-USDC.
    let ob = client.hyperliquid.spot.orderbook.get("HYPE-USDC", None).await?;
    println!(
        "\nHYPE-USDC orderbook: {} bids, {} asks, mid={:?}",
        ob.bids.len(),
        ob.asks.len(),
        ob.mid_price
    );

    // Recent trades (last 24h). Spot fills backfill to 2025-03-22.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let day_ago_ms = now_ms - 24 * 60 * 60 * 1000;
    let trades = client
        .hyperliquid
        .spot
        .trades
        .list(
            "PURR-USDC",
            GetTradesParams {
                start: day_ago_ms.into(),
                end: now_ms.into(),
                cursor: None,
                limit: Some(100),
                side: None,
            },
        )
        .await?;
    println!("\nPURR-USDC trades (last 24h): {}", trades.data.len());

    // TWAP statuses for a symbol.
    let twap = client
        .hyperliquid
        .spot
        .twap
        .by_symbol("HYPE-USDC", SpotTwapParams::default())
        .await?;
    println!("HYPE-USDC TWAP statuses: {}", twap.data.len());

    // Per-symbol freshness across every spot table.
    let freshness = client.hyperliquid.spot.freshness("HYPE-USDC").await?;
    println!("\nHYPE-USDC freshness: {} data types", freshness.data_types.len());

    Ok(())
}
