// File: src/main.rs
mod binance;
mod config;
mod time_helpers;
mod ws_helpers;

use binance::*;
use config::Config;
use futures_util::StreamExt;
use local_ip_address::local_ip;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use warp::Filter;
use ws_helpers::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::load();
    if config.symbols.is_empty() {
        return;
    }

    let mut config_map = HashMap::new();
    let mut last_prices = HashMap::new();
    for cfg in &config.symbols {
        println!(
            "Symbol: {} => Big Trade Qty: {}, Spike %: {}",
            cfg.symbol.to_uppercase(),
            cfg.big_trade_qty,
            cfg.spike_pct
        );
        config_map.insert(cfg.symbol.clone(), cfg.clone());
    }

    let (tx, _rx) = broadcast::channel(config.broadcast_capacity);

    // Spawn Warp server
    let ws_route = warp::path("aggTrade").and(warp::ws()).map({
        let tx = tx.clone();
        move |ws: warp::ws::Ws| {
            let tx_inner = tx.clone();
            ws.on_upgrade(move |socket| handle_client(socket, tx_inner))
        }
    });
    let ip = local_ip().unwrap();
    println!(
        "WebSocket server running on ws://{}:{}/aggTrade",
        ip, config.port
    );
    tokio::spawn(warp::serve(ws_route).run(([0, 0, 0, 0], config.port)));

    // Binance connection
    let streams: Vec<String> = config
        .symbols
        .iter()
        .map(|c| format!("{}@aggTrade", c.symbol))
        .collect();
    let url = format!(
        "wss://data-stream.binance.vision/stream?streams={}",
        streams.join("/")
    );
    println!("Connecting to Binance: {}", url);
    let (ws_stream, _) = connect_async(&url)
        .await
        .expect("Failed to connect to Binance");
    let mut read = ws_stream;

    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if msg.is_text() {
                if let Some(agg) = parse_agg_trade(msg.to_text().unwrap()) {
                    let symbol = agg.s.to_lowercase();
                    let cfg = match config_map.get(&symbol) {
                        Some(c) => c,
                        None => continue,
                    };
                    let spike = calc_spike(
                        last_prices.get(&symbol).copied(),
                        agg.p.parse().unwrap_or(0.0),
                    );
                    last_prices.insert(symbol.clone(), agg.p.parse().unwrap_or(0.0));
                    log_and_broadcast(&tx, &agg, spike, cfg).await;
                }
            }
        }
    }
}
