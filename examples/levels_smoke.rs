//! Price levels and liquidations: hourly candles, projected liquidation
//! levels and their history, HIP-3 trigger levels, and the liquidations of
//! one wallet.
//!
//! Run: OXARCHIVE_API_KEY=... cargo run --example levels_smoke

use oxarchive::resources::candles::CandleHistoryParams;
use oxarchive::resources::liquidations::LiquidationsByUserParams;
use oxarchive::types::{CandleInterval, Timestamp};
use oxarchive::{LevelsHistoryParams, LiquidationLevelsParams, OxArchive, TriggerLevelsParams};

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("OXARCHIVE_API_KEY")?;
    let client = OxArchive::new(&key)?;
    let now = chrono::Utc::now().timestamp_millis();

    // Hourly BTC candles for the last six hours.
    let candles = client
        .hyperliquid
        .candles
        .history(
            "BTC",
            CandleHistoryParams {
                start: Timestamp::Millis(now - 6 * HOUR_MS),
                end: Timestamp::Millis(now),
                cursor: None,
                limit: Some(5),
                interval: Some(CandleInterval::OneHour),
            },
        )
        .await?;
    println!(
        "candles: {} rows, first close {:?}",
        candles.data.len(),
        candles.data.first().map(|c| &c.close)
    );

    // Projected liquidation levels, grouped into 12 price buckets.
    let liq = client
        .hyperliquid
        .liquidations
        .levels("BTC", LiquidationLevelsParams { buckets: Some(12), ..Default::default() })
        .await?;
    println!(
        "liquidation levels: snapshot {}, {} buckets, total long {:.0}",
        liq.snapshot_ts,
        liq.levels.len(),
        liq.total_long
    );

    // Liquidation level history in summary form (bucket detail left out).
    let hist = client
        .hyperliquid
        .liquidations
        .levels_history(
            "BTC",
            LevelsHistoryParams { limit: Some(3), summary: Some(true), ..Default::default() },
        )
        .await?;
    println!(
        "liquidation level history: {} snapshots, next cursor {:?}, levels omitted: {}",
        hist.data.len(),
        hist.next_cursor,
        hist.data.iter().all(|item| item.levels.is_none())
    );

    // Pending HIP-3 trigger orders (stop-loss and take-profit), by price bucket.
    let trig = client
        .hyperliquid
        .hip3
        .orders
        .trigger_levels("xyz:TSLA", TriggerLevelsParams { buckets: Some(10), ..Default::default() })
        .await?;
    println!("HIP-3 trigger levels: as of {}, {} buckets", trig.as_of, trig.levels.len());

    // Liquidations of one wallet over the last seven days.
    let by_user = client
        .hyperliquid
        .liquidations
        .by_user(
            "0x32fe14732b5b54dc08c6eee46b9ba319ca38e9f4",
            LiquidationsByUserParams {
                start: Timestamp::Millis(now - 7 * DAY_MS),
                end: Timestamp::Millis(now),
                coin: None,
                cursor: None,
                limit: Some(5),
            },
        )
        .await?;
    println!("liquidations by user: {} rows", by_user.data.len());

    Ok(())
}
