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
    println!(
        "Depth filters => min_qty: {}, min_notional: {}",
        config.big_depth_min_qty, config.big_depth_min_notional
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
                let min_qty = config.big_depth_min_qty;
                let min_notional = config.big_depth_min_notional;

                let is_level_big = |price: f64, qty: f64| {
                    if min_qty <= 0.0 && min_notional <= 0.0 {
                        return true;
                    }

                    let qty_ok = min_qty > 0.0 && qty >= min_qty;
                    let notional_ok = min_notional > 0.0 && (price * qty) >= min_notional;
                    qty_ok || notional_ok
                };

                let extract_big_levels = |levels: &[[String; 2]]| {
                    levels
                        .iter()
                        .filter_map(|level| {
                            let price = level[0].parse::<f64>().ok()?;
                            let qty = level[1].parse::<f64>().ok()?;

                            if !is_level_big(price, qty) {
                                return None;
                            }

                            Some((price, qty))
                        })
                        .collect::<Vec<(f64, f64)>>()
                };

                let big_bids = extract_big_levels(&depth.bids);
                let big_asks = extract_big_levels(&depth.asks);

                if big_bids.is_empty() && big_asks.is_empty() {
                    continue;
                }

                let format_levels = |levels: &[(f64, f64)]| {
                    if levels.is_empty() {
                        return "-".to_string();
                    }

                    levels
                        .iter()
                        .map(|(price, qty)| format!("{price:.8} x {qty:.8} (n:{:.8})", price * qty))
                        .collect::<Vec<String>>()
                        .join(",")
                };

                let depth_msg = format!(
                    "[DEPTH] {} E:{} U:{} u:{} big_bids:[{}] big_asks:[{}]",
                    depth.symbol.to_uppercase(),
                    depth.event_time,
                    depth.first_update_id,
                    depth.final_update_id,
                    format_levels(&big_bids),
                    format_levels(&big_asks)
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
