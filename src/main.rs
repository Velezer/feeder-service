// File: src/main.rs
use feeder_service::binance::*;
use feeder_service::binance_depth::*;
use feeder_service::config::Config;
use feeder_service::ws_helpers::*;
use futures_util::StreamExt;
use local_ip_address::local_ip;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use warp::Filter;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::load();
    if config.symbols.is_empty() {
        return;
    }

    let mut config_map = HashMap::new();
    let mut last_prices = HashMap::new();
    let symbols: Vec<String> = config
        .symbols
        .iter()
        .map(|cfg| cfg.symbol.clone())
        .collect();

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
    let ip_display = local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    println!(
        "WebSocket server running on ws://{}:{}/aggTrade",
        ip_display, config.port
    );
    tokio::spawn(warp::serve(ws_route).run(([0, 0, 0, 0], config.port)));

    // Binance connection (aggTrade + depth)
    let mut streams: Vec<String> = symbols
        .iter()
        .map(|symbol| format!("{}@aggTrade", symbol))
        .collect();
    streams.extend(build_diff_depth_streams(&symbols, 100));

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
            if !msg.is_text() {
                continue;
            }

            let payload = match msg.to_text() {
                Ok(text) => text,
                Err(_) => continue,
            };

            if let Some(agg) = parse_agg_trade(payload) {
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
                continue;
            }

            if let Some(depth) = parse_depth_update(payload) {
                let best_bid = depth
                    .bids
                    .first()
                    .map(|level| format!("{} x {}", level[0], level[1]))
                    .unwrap_or_else(|| "-".to_string());
                let best_ask = depth
                    .asks
                    .first()
                    .map(|level| format!("{} x {}", level[0], level[1]))
                    .unwrap_or_else(|| "-".to_string());

                let depth_msg = format!(
                    "[DEPTH] {} E:{} U:{} u:{} bid:{} ask:{}",
                    depth.symbol.to_uppercase(),
                    depth.event_time,
                    depth.first_update_id,
                    depth.final_update_id,
                    best_bid,
                    best_ask
                );
                println!("{}", depth_msg);
                let _ = tx.send(depth_msg);
                continue;
            }

            if std::env::var_os("LOG_UNKNOWN_STREAM_MESSAGES").is_some() {
                let snippet: String = payload.chars().take(180).collect();
                let suffix = if payload.chars().count() > 180 {
                    "..."
                } else {
                    ""
                };
                eprintln!("[stream] unhandled text message: '{}{}'", snippet, suffix);
            }
        }
    }
}
