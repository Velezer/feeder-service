use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use local_ip_address::local_ip;
use serde::Deserialize;
use simd_json::serde::from_slice;
use std::collections::HashMap;
use std::env;
use tokio::sync::broadcast;
use tokio::time::{Duration, interval};
use tokio_tungstenite::connect_async;
use url::Url;
use warp::Filter;
use warp::ws::{Message, WebSocket};

#[derive(Debug, Deserialize)]
struct AggTrade {
    s: String,
    p: String,
    q: String,
    T: u64,
    m: bool,
}

#[derive(Debug, Clone)]
struct SymbolConfig {
    symbol: String,
    big_trade_qty: f64,
    spike_pct: f64,
}

/// Parse symbol configs from environment variables.
///
/// Expects:
///   SYMBOLS=btcusdt,ethusdt
///   BTCUSDT_BIG_TRADE_QTY=20    (optional, falls back to BIG_TRADE_QTY or 20)
///   BTCUSDT_SPIKE_PCT=0.4       (optional, falls back to SPIKE_PCT or 0.4)
fn parse_symbol_configs() -> Vec<SymbolConfig> {
    let default_qty: f64 = env::var("BIG_TRADE_QTY")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .expect("BIG_TRADE_QTY must be a number");

    let default_spike: f64 = env::var("SPIKE_PCT")
        .unwrap_or_else(|_| "0.4".to_string())
        .parse()
        .expect("SPIKE_PCT must be a number");

    let symbols_str = env::var("SYMBOLS").unwrap_or_else(|_| "btcusdt".to_string());

    symbols_str
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|symbol| {
            let upper = symbol.to_uppercase();

            let big_trade_qty: f64 = env::var(format!("{}_BIG_TRADE_QTY", upper))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_qty);

            let spike_pct: f64 = env::var(format!("{}_SPIKE_PCT", upper))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_spike);

            SymbolConfig {
                symbol,
                big_trade_qty,
                spike_pct,
            }
        })
        .collect()
}

async fn handle_client(ws: WebSocket, tx: broadcast::Sender<String>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if let Err(_) = ws_tx.send(Message::ping(vec![])).await {
                    break;
                }
            }
            Ok(msg) = rx.recv() => {
                if let Err(_) = ws_tx.send(Message::text(msg)).await {
                    break;
                }
            }
            Some(Ok(msg)) = ws_rx.next() => {
                if msg.is_ping() {
                    let _ = ws_tx.send(Message::pong(msg.as_bytes().to_vec())).await;
                } else if msg.is_close() {
                    let _ = ws_tx.send(Message::close()).await;
                    break;
                }
            }
        }
    }

    println!("Client disconnected");
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let configs = parse_symbol_configs();
    if configs.is_empty() {
        eprintln!("No symbols configured. Set SYMBOLS env var.");
        return;
    }

    let port: u16 = env::var("PORT")
        .unwrap_or("9001".to_string())
        .parse()
        .unwrap();

    let broadcast_capacity: usize = env::var("BROADCAST_CAPACITY")
        .unwrap_or_else(|_| "16".to_string())
        .parse()
        .expect("BROADCAST_CAPACITY must be a number");

    // Build per-symbol lookup
    let mut config_map: HashMap<String, SymbolConfig> = HashMap::new();
    let mut last_prices: HashMap<String, f64> = HashMap::new();

    for cfg in &configs {
        println!(
            "Symbol: {} => Big Trade Qty: {}, Spike %: {}",
            cfg.symbol.to_uppercase(),
            cfg.big_trade_qty,
            cfg.spike_pct
        );
        config_map.insert(cfg.symbol.clone(), cfg.clone());
    }

    // Broadcast channel
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

    // Build combined stream URL for all symbols
    let streams: Vec<String> = configs.iter().map(|c| format!("{}@aggTrade", c.symbol)).collect();
    let url = Url::parse(&format!(
        "wss://stream.binance.com:9443/stream?streams={}",
        streams.join("/")
    ))
    .unwrap();

    println!("Connecting to Binance: {}", url);

    let (ws_stream, _) = connect_async(url)
        .await
        .expect("Failed to connect to Binance");
    println!("Connected to Binance WebSocket");

    let mut read = ws_stream;

    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if msg.is_text() {
                let mut bytes = msg.to_text().unwrap().as_bytes().to_vec();

                // Combined stream wraps data in {"stream":"...","data":{...}}
                let wrapper: CombinedStreamMsg = match from_slice(&mut bytes) {
                    Ok(w) => w,
                    Err(_) => continue,
                };

                let agg = wrapper.data;
                let symbol = agg.s.to_lowercase();

                let cfg = match config_map.get(&symbol) {
                    Some(c) => c,
                    None => continue,
                };

                let price: f64 = match agg.p.parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let qty: f64 = match agg.q.parse() {
                    Ok(q) => q,
                    Err(_) => continue,
                };

                let spike = last_prices
                    .get(&symbol)
                    .map(|last| ((price - last).abs() / last) * 100.0)
                    .unwrap_or(0.0);
                last_prices.insert(symbol.clone(), price);

                let utc_ts = Utc.timestamp_millis_opt(agg.T as i64).unwrap();
                let offset = FixedOffset::east_opt(7 * 3600).unwrap();
                let dt_utc7: DateTime<FixedOffset> = utc_ts.with_timezone(&offset);

                if qty >= cfg.big_trade_qty || spike >= cfg.spike_pct {
                    let now_ms = Utc::now().timestamp_millis();
                    let delay_ms = now_ms - agg.T as i64;

                    let log_msg = format!(
                        "[{}] {} - Price: {:.2}, Qty: {:.4}, Spike: {:.4}%, BuyerMaker: {}, Delay: {} ms",
                        dt_utc7.format("%Y-%m-%d %H:%M:%S"),
                        symbol.to_uppercase(),
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

#[derive(Debug, Deserialize)]
struct CombinedStreamMsg {
    data: AggTrade,
}
