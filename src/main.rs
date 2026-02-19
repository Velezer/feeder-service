use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use local_ip_address::local_ip;
use serde::Deserialize;
use simd_json::serde::from_slice;
use std::env;
use tokio::sync::broadcast;
use tokio::time::{Duration, interval};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use url::Url;
use warp::Filter;
use warp::ws::{Message, WebSocket};

#[derive(Debug, Deserialize)]
struct AggTrade {
    p: String,
    q: String,
    T: u64,
    m: bool,
}

// Handle individual WebSocket clients
async fn handle_client(ws: WebSocket, tx: broadcast::Sender<String>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            // heartbeat ping every 15s
            _ = heartbeat.tick() => {
                if let Err(_) = ws_tx.send(Message::ping(vec![])).await {
                    break;
                }
            }
            // broadcast messages
            Ok(msg) = rx.recv() => {
                if let Err(_) = ws_tx.send(Message::text(msg)).await {
                    break;
                }
            }
            // handle incoming messages
            Some(Ok(msg)) = ws_rx.next() => {
            if msg.is_ping() {
                let _ = ws_tx.send(Message::pong(msg.as_bytes().to_vec())).await;
            } else if msg.is_close() {
                let _ = ws_tx.send(Message::close()).await;
                break;
            }
        }
            // optional: default branch if nothing else is ready
            // default => { /* can do something here, or just skip */ }
        }
    }

    println!("Client disconnected");
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // thresholds
    let big_trade_qty: f64 = env::var("BIG_TRADE_QTY")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap();

    let spike_pct: f64 = env::var("SPIKE_PCT")
        .unwrap_or_else(|_| "0.4".to_string())
        .parse()
        .unwrap();

    let port: u16 = env::var("PORT")
        .unwrap_or("9001".to_string())
        .parse()
        .unwrap();

    println!(
        "Thresholds => Big Trade Qty: {}, Spike %: {}",
        big_trade_qty, spike_pct
    );

    let broadcast_capacity: usize = env::var("BROADCAST_CAPACITY")
        .unwrap_or_else(|_| "16".to_string())
        .parse()
        .expect("BROADCAST_CAPACITY must be a number");

    // broadcast channel
    let (tx, _rx) = broadcast::channel::<String>(broadcast_capacity);

    // Warp WebSocket route
    let ws_route = warp::path("aggTrade").and(warp::ws()).map({
        let tx = tx.clone();
        move |ws: warp::ws::Ws| {
            let tx_inner = tx.clone();
            ws.on_upgrade(move |socket| handle_client(socket, tx_inner))
        }
    });

    // Spawn warp server
    let ip = local_ip().unwrap();
    println!("WebSocket server running on ws://{}:{}/aggTrade", ip, port);
    tokio::spawn(warp::serve(ws_route).run(([0, 0, 0, 0], port)));

    // Connect to Binance aggTrade
    let url = Url::parse("wss://stream.binance.com:9443/ws/btcusdt@aggTrade").unwrap();
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("Failed to connect to Binance");
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

                let utc_ts = Utc.timestamp_millis_opt(agg.T as i64).unwrap();
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
