use std::env;

use chrono::{DateTime, FixedOffset, TimeZone, Utc, offset::LocalResult};
use dotenv::dotenv;
use futures_util::StreamExt;
use serde::Deserialize;
use simd_json::serde::from_slice;
use tokio_tungstenite::connect_async;
use url::Url;

#[derive(Debug, Deserialize)]
struct AggTrade {
    p: String, // price
    q: String, // quantity
    T: u64,    // timestamp in ms
    m: bool,   // is_buyer_maker
}

#[tokio::main]
async fn main() {
    dotenv().ok(); // load .env

    let big_trade_qty: f64 = env::var("BIG_TRADE_QTY")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap();

    let spike_pct: f64 = env::var("SPIKE_PCT")
        .unwrap_or_else(|_| "0.4".to_string())
        .parse()
        .unwrap();

    println!(
        "Thresholds => Big Trade Qty: {}, Spike %: {}",
        big_trade_qty, spike_pct
    );

    // Binance aggTrade WebSocket
    let url = Url::parse("wss://stream.binance.com:9443/ws/btcusdt@aggTrade").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    println!("Connected to Binance WebSocket");

    let mut read = ws_stream;

    let mut last_price: Option<f64> = None;

    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if msg.is_text() {
                let mut bytes = msg.to_text().unwrap().as_bytes().to_vec(); // Vec<u8> is mutable
                let agg: AggTrade = from_slice(&mut bytes).unwrap();
                let price: f64 = agg.p.parse().unwrap();
                let qty: f64 = agg.q.parse().unwrap();

                let spike = last_price
                    .map(|last| ((price - last).abs() / last) * 100.0)
                    .unwrap_or(0.0);

                last_price = Some(price);

                let utc_ts = match Utc.timestamp_millis_opt(agg.T as i64) {
                    LocalResult::Single(dt) => dt,
                    _ => continue, // invalid timestamp
                };

                // Create FixedOffset safely (UTC+7)
                let offset = match FixedOffset::east_opt(7 * 3600) {
                    Some(fx) => fx,
                    None => continue, // should not happen
                };

                let dt_utc7: DateTime<FixedOffset> = utc_ts.with_timezone(&offset);

                // Log only if big trade or spike
                if qty >= big_trade_qty || spike >= spike_pct {
                    let now_ms = Utc::now().timestamp_millis();
                    let delay_ms = now_ms - agg.T as i64;

                    println!(
                        "[{}] BTCUSDT - Price: {:.2}, Qty: {:.4}, Spike: {:.4}%, BuyerMaker: {}, Delay: {} ms",
                        dt_utc7.format("%Y-%m-%d %H:%M:%S"),
                        price,
                        qty,
                        spike,
                        agg.m,
                        delay_ms
                    );
                }
            }
        }
    }
}
