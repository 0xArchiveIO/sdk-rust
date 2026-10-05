use oxarchive::resources::trades::GetTradesParams;
use oxarchive::OxArchive;

#[tokio::main]
async fn main() -> oxarchive::Result<()> {
    let api_key = std::env::var("OXARCHIVE_API_KEY").expect("Set OXARCHIVE_API_KEY");
    let client = OxArchive::new(api_key)?;

    // Paginate through BTC trades for the last ten minutes
    let end = chrono::Utc::now().timestamp_millis();
    let start = end - 10 * 60 * 1000;
    let mut all_trades = vec![];
    let mut cursor = None;
    let mut page = 0u32;

    loop {
        let result = client
            .hyperliquid
            .trades
            .history(
                "BTC",
                GetTradesParams {
                    start: start.into(),
                    end: end.into(),
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

        // Follow next_cursor, with the same filters, while has_more is true.
        if !result.has_more {
            break;
        }
        cursor = result.next_cursor;
    }

    println!("\nTotal BTC trades in the last ten minutes: {}", all_trades.len());

    Ok(())
}
