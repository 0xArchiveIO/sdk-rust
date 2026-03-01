use oxarchive::OxArchive;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");
    let client = OxArchive::new(api_key)?;

    // List all Hyperliquid instruments
    let instruments = client.hyperliquid.instruments.list().await?;
    println!("Hyperliquid: {} instruments", instruments.len());
    for inst in instruments.iter().take(5) {
        println!("  {} ({}x leverage)", inst.name, inst.max_leverage.unwrap_or(0));
    }

    // Get BTC orderbook
    let ob = client.hyperliquid.orderbook.get("BTC", None).await?;
    println!(
        "\nBTC orderbook: {} bids, {} asks, mid={:?}",
        ob.bids.len(),
        ob.asks.len(),
        ob.mid_price
    );

    // Get current ETH funding rate
    let funding = client.hyperliquid.funding.current("ETH").await?;
    println!("\nETH funding rate: {}", funding.funding_rate);

    // Get current BTC open interest
    let oi = client.hyperliquid.open_interest.current("BTC").await?;
    println!("BTC open interest: {}", oi.open_interest);

    // Lighter.xyz
    let lighter = client.lighter.instruments.list().await?;
    println!("\nLighter: {} instruments", lighter.len());

    // HIP-3
    let hip3 = client.hyperliquid.hip3.instruments.list().await?;
    println!("HIP-3: {} instruments", hip3.len());

    // Data quality
    let status = client.data_quality.status().await?;
    println!("\nSystem status: {}", status.status);

    Ok(())
}
