use chrono::{DateTime, FixedOffset, TimeZone, Utc, offset::LocalResult};
use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use local_ip_address::local_ip;
use serde::Deserialize;
use simd_json::serde::from_slice;
use std::env;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::{self, Duration};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};
use url::Url;

#[derive(Debug, Deserialize)]
struct AggTrade {
    p: String,
    q: String,
    T: u64,
    m: bool,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let big_trade_qty: f64 = env::var("BIG_TRADE_QTY")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap();

    let spike_pct: f64 = env::var("SPIKE_PCT")
        .unwrap_or_else(|_| "0.4".to_string())
        .parse()
        .unwrap();

    let port = env::var("PORT").unwrap_or("9001".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!(
        "Thresholds => Big Trade Qty: {}, Spike %: {}",
        big_trade_qty, spike_pct
    );

    let broadcast_capacity: usize = env::var("BROADCAST_CAPACITY")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .expect("BROADCAST_CAPACITY must be a number");

    // Broadcast channel with larger buffer
    let (tx, _rx) = broadcast::channel::<String>(broadcast_capacity);

    // --- Android listener ---
    let tx_listener = tx.clone();
    tokio::spawn(async move {
        let listener = TcpListener::bind(&addr)
            .await
            .expect("Failed to bind TCP listener");
        let ip = local_ip().unwrap();
        println!("Android WebSocket listener running on ws://{}:{}", ip, port);

        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream)
                .await
                .expect("Failed to accept WebSocket");
            let mut rx = tx_listener.subscribe();
            let (mut ws_sink, mut ws_stream) = ws_stream.split();

            // Single task handles heartbeat, broadcast, and ping/pong
            tokio::spawn(async move {
                let mut heartbeat = time::interval(Duration::from_secs(15));

                loop {
                    tokio::select! {
                        // Heartbeat ping every 15s
                        _ = heartbeat.tick() => {
                            if let Err(e) = ws_sink.send(Message::Ping(vec![])).await {
                                eprintln!("Heartbeat ping failed: {}", e);
                                break;
                            }
                        }
                        // Broadcast messages to this client
                        Ok(msg) = rx.recv() => {
                            if let Err(e) = ws_sink.send(Message::Text(msg)).await {
                                eprintln!("Failed to send to client: {}", e);
                                break;
                            }
                        }
                        // Handle incoming messages from client (Ping/Close)
                        Some(Ok(msg)) = ws_stream.next() => {
                            match msg {
                                Message::Ping(p) => {
                                    let _ = ws_sink.send(Message::Pong(p)).await;
                                }
                                Message::Close(_) => {
                                    let _ = ws_sink.send(Message::Close(None)).await;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                println!("Client disconnected");
            });
        }
    });

    // --- Connect to Binance aggTrade ---
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
                let offset = FixedOffset::east_opt(7 * 3600).unwrap();
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

                    println!("{}", log_msg);
                    let _ = tx.send(log_msg);
                }
            }
        }
    }
}
