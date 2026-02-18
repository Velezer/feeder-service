use chrono::{DateTime, FixedOffset, TimeZone, Utc, offset::LocalResult};
use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use local_ip_address::local_ip;
use serde::Deserialize;
use simd_json::serde::from_slice;
use std::env;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};
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

    // Create a broadcast channel for connected clients
    let (tx, _rx) = broadcast::channel::<String>(1);

    // --- Spawn Android listener (non-blocking) ---
    let tx_listener = tx.clone();
    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:9001")
            .await
            .expect("Failed to bind TCP listener");

        let ip = local_ip().unwrap(); // detects local IP
        let port = 9001;
        println!("Android WebSocket listener running on ws://{}:{}", ip, port);

        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream)
                .await
                .expect("Failed to accept WebSocket");

            let mut rx = tx_listener.subscribe();
            let (mut ws_sink, _) = ws_stream.split();

            tokio::spawn(async move {
                while let Ok(msg) = rx.recv().await {
                    let _ = ws_sink.send(Message::Text(msg)).await;
                }
            });
        }
    });

    // --- Connect to Binance aggTrade WebSocket ---
    let url = Url::parse("wss://stream.binance.com:9443/ws/btcusdt@aggTrade").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("Connected to Binance WebSocket");

    let mut read = ws_stream;
    let mut last_price: Option<f64> = None;

    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if msg.is_text() {
                let mut bytes = msg.to_text().unwrap().as_bytes().to_vec();
                let agg: AggTrade = from_slice(&mut bytes).unwrap();
                let price: f64 = agg.p.parse().unwrap();
                let qty: f64 = agg.q.parse().unwrap();

                let spike = last_price
                    .map(|last| ((price - last).abs() / last) * 100.0)
                    .unwrap_or(0.0);

                last_price = Some(price);

                let utc_ts = match Utc.timestamp_millis_opt(agg.T as i64) {
                    LocalResult::Single(dt) => dt,
                    _ => continue,
                };

                let offset = match FixedOffset::east_opt(7 * 3600) {
                    Some(fx) => fx,
                    None => continue,
                };

                let dt_utc7: DateTime<FixedOffset> = utc_ts.with_timezone(&offset);

                if qty >= big_trade_qty || spike >= spike_pct {
                    let now_ms = Utc::now().timestamp_millis();
                    let delay_ms = now_ms - agg.T as i64;

                    let log_msg = format!(
                        "[{}] BTCUSDT - Price: {:.2}, Qty: {:.4}, Spike: {:.4}%, BuyerMaker: {}, Delay: {} ms",
                        dt_utc7.format("%Y-%m-%d %H:%M:%S"),
                        price,
                        qty,
                        spike,
                        agg.m,
                        delay_ms
                    );

                    // Print locally
                    println!("{}", log_msg);

                    // Broadcast to any subscribed Android clients (non-blocking)
                    let _ = tx.send(log_msg);
                }
            }
        }
    }
}
