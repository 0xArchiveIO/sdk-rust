//! Quick live smoke test for the 1.9.0 additions and fixes.
//! Run: OXARCHIVE_API_KEY=... cargo run --example levels_smoke

use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::liquidations::LiquidationsByUserParams;
use oxarchive::types::{CandleInterval, Timestamp};
use oxarchive::{LevelsHistoryParams, LiquidationLevelsParams, OxArchive, TriggerLevelsParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("OXARCHIVE_API_KEY")?;
    let client = OxArchive::new(&key)?;
    let now = chrono::Utc::now().timestamp_millis();

    // C13 regression: candles previously failed on every call (String vs number)
    let candles = client
        .hyperliquid
        .candles
        .history(
            "BTC",
            CandleHistoryParams {
                start: Timestamp::Millis(now - 6 * 3_600_000),
                end: Timestamp::Millis(now),
                cursor: None,
                limit: Some(5),
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await?;
    println!("candles: {} rows, first close {}", candles.data.len(), candles.data[0].close);

    // New: liquidation levels
    let liq = client
        .hyperliquid
        .liquidations
        .levels("BTC", LiquidationLevelsParams { buckets: Some(12), ..Default::default() })
        .await?;
    println!(
        "liq levels: snapshot {} buckets {} total_long {:.0}",
        liq.snapshot_ts,
        liq.levels.len(),
        liq.total_long
    );

    // New: liquidation levels history (summary)
    let hist = client
        .hyperliquid
        .liquidations
        .levels_history(
            "BTC",
            LevelsHistoryParams { limit: Some(3), summary: Some(true), ..Default::default() },
        )
        .await?;
    println!(
        "liq history: {} items, cursor {:?}, levels omitted: {}",
        hist.data.len(),
        hist.next_cursor,
        hist.data[0].levels.is_none()
    );

    // New: hip3 trigger levels
    let trig = client
        .hyperliquid
        .hip3
        .orders
        .trigger_levels("xyz:TSLA", TriggerLevelsParams { buckets: Some(10), ..Default::default() })
        .await?;
    println!("hip3 trigger: as_of {} buckets {}", trig.as_of, trig.levels.len());

    // C12 regression: by_user previously hit the wrong path
    let by_user = client
        .hyperliquid
        .liquidations
        .by_user(
            "0x32fe14732b5b54dc08c6eee46b9ba319ca38e9f4",
            LiquidationsByUserParams {
                start: Timestamp::Millis(now - 200 * 86_400_000),
                end: Timestamp::Millis(now),
                coin: None,
                cursor: None,
                limit: Some(5),
            },
        )
        .await?;
    println!("by_user: {} rows", by_user.data.len());

    Ok(())
}
