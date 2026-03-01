use oxarchive::resources::trades::GetTradesParams;
use oxarchive::OxArchive;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");
    let client = OxArchive::new(api_key)?;

    // Paginate through BTC trades for a 24-hour window
    let mut all_trades = vec![];
    let mut cursor = None;
    let mut page = 0u32;

    loop {
        let result = client
            .hyperliquid
            .trades
            .list(
                "BTC",
                GetTradesParams {
                    start: 1704067200000_i64.into(), // 2024-01-01 00:00 UTC
                    end: 1704153600000_i64.into(),   // 2024-01-02 00:00 UTC
                    cursor,
                    limit: Some(1000),
                    side: None,
                },
            )
            .await?;

        page += 1;
        let count = result.data.len();
        all_trades.extend(result.data);
        println!("Page {page}: fetched {count} trades (total: {})", all_trades.len());

        cursor = result.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    println!("\nTotal BTC trades in 24h window: {}", all_trades.len());

    Ok(())
}
