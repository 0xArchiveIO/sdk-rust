use oxarchive::ws::{OxArchiveWs, ServerMsg, WsOptions};

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");

    // --- Real-time streaming (orderbook + trades + liquidations + HIP-4) ---
    let mut ws = OxArchiveWs::new(WsOptions::new(&api_key));
    ws.connect().await?;

    ws.subscribe("orderbook", Some("BTC")).await?;
    ws.subscribe("trades", Some("ETH")).await?;
    // Liquidations now stream live (same wire shape as trades, with
    // `is_liquidation: true` on each fill row).
    ws.subscribe("liquidations", Some("BTC")).await?;
    // HIP-4 outcome markets use the bare `#<id>` symbol form.
    ws.subscribe("hip4_orderbook", Some("#0")).await?;
    ws.subscribe("hip4_trades", Some("#0")).await?;

    let mut rx = ws.rx.take().expect("receiver");
    let mut count = 0u32;

    while let Some(msg) = rx.recv().await {
        match &msg {
            ServerMsg::Subscribed { channel, coin, .. } => {
                println!("Subscribed to {channel} {}", coin.as_deref().unwrap_or(""));
            }
            ServerMsg::Data { channel, coin, .. } => {
                count += 1;
                println!(
                    "[{count}] {channel} {} update",
                    coin.as_deref().unwrap_or("")
                );
                if count >= 10 {
                    break;
                }
            }
            ServerMsg::OutcomeSettled { coin, settlement_value, .. } => {
                println!("HIP-4 settled: {coin} -> {:?}", settlement_value);
                // Server has already auto-unsubscribed our hip4_* subs for
                // this coin. Treat as terminal for the coin.
            }
            ServerMsg::Error { message } => {
                eprintln!("Error: {message}");
                break;
            }
            ServerMsg::Pong => println!("pong"),
            _ => {}
        }
    }

    ws.unsubscribe("orderbook", Some("BTC")).await?;
    ws.unsubscribe("trades", Some("ETH")).await?;
    ws.unsubscribe("liquidations", Some("BTC")).await?;
    ws.unsubscribe("hip4_orderbook", Some("#0")).await?;
    ws.unsubscribe("hip4_trades", Some("#0")).await?;
    ws.disconnect().await;

    // --- Historical replay ---
    let mut ws = OxArchiveWs::new(WsOptions::new(&api_key));
    ws.connect().await?;

    ws.replay(
        "orderbook",
        "BTC",
        1704067200000, // 2024-01-01 00:00 UTC
        Some(1704070800000), // 2024-01-01 01:00 UTC
        Some(100.0),   // 100x speed
    )
    .await?;

    let mut rx = ws.rx.take().expect("receiver");
    let mut snapshots = 0u32;

    while let Some(msg) = rx.recv().await {
        match &msg {
            ServerMsg::ReplayStarted { .. } => println!("Replay started"),
            ServerMsg::HistoricalData { timestamp, .. } => {
                snapshots += 1;
                if snapshots % 100 == 0 {
                    println!("Received {snapshots} snapshots (ts={timestamp})");
                }
            }
            ServerMsg::ReplayCompleted { snapshots_sent, .. } => {
                println!("Replay complete: {} snapshots", snapshots_sent.unwrap_or(0));
                break;
            }
            ServerMsg::Error { message } => {
                eprintln!("Replay error: {message}");
                break;
            }
            _ => {}
        }
    }

    ws.disconnect().await;

    // --- Bulk streaming ---
    let mut ws = OxArchiveWs::new(WsOptions::new(&api_key));
    ws.connect().await?;

    ws.stream(
        "trades",
        "ETH",
        1704067200000, // 2024-01-01 00:00 UTC
        1704153600000, // 2024-01-02 00:00 UTC
        Some(5000),    // batch size
    )
    .await?;

    let mut rx = ws.rx.take().expect("receiver");
    let mut total = 0u64;

    while let Some(msg) = rx.recv().await {
        match &msg {
            ServerMsg::StreamStarted { .. } => println!("Stream started"),
            ServerMsg::HistoricalBatch { data, .. } => {
                total += data.len() as u64;
                println!("Batch: {} records (total: {total})", data.len());
            }
            ServerMsg::StreamProgress { snapshots_sent } => {
                println!("Progress: {} sent", snapshots_sent.unwrap_or(0));
            }
            ServerMsg::StreamCompleted { snapshots_sent, .. } => {
                println!("Stream complete: {} records", snapshots_sent.unwrap_or(0));
                break;
            }
            ServerMsg::Error { message } => {
                eprintln!("Stream error: {message}");
                break;
            }
            _ => {}
        }
    }

    ws.disconnect().await;
    Ok(())
}
