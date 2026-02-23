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

use feeder_service::refactor::big_move_detector::{BigMoveDetector, BigMoveSignal, DepthSnapshot};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::load();
    if config.symbols.is_empty() {
        return;
    }

    // Maps and state
    let mut config_map: HashMap<String, _> = HashMap::new();
    let mut last_prices: HashMap<String, f64> = HashMap::new();
    let mut big_move_detectors: HashMap<String, BigMoveDetector> = HashMap::new();

    // Symbol list (lowercase used later)
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
        // instantiate detector for each symbol (preserve previous behaviour if used later)
        big_move_detectors.insert(cfg.symbol.to_lowercase(), BigMoveDetector::new(5, 75.0, 0.0, 3));
    }

    let (tx, _rx) = broadcast::channel(config.broadcast_capacity);

    // Spawn Warp server for websocket clients
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
        "Depth filters => min_qty: {}, min_notional: {}, min_pressure: {}",
        config.big_depth_min_qty, config.big_depth_min_notional, config.big_depth_min_pressure_pct
    );

    tokio::spawn(warp::serve(ws_route).run(([0, 0, 0, 0], config.port)));

    // Build Binance streams: aggTrade for each symbol + diff depth streams
    let mut streams: Vec<String> = symbols.iter().map(|s| format!("{}@aggTrade", s)).collect();
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

    // Main loop: read messages from Binance websocket
    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if !msg.is_text() {
                continue;
            }

            let payload = match msg.to_text() {
                Ok(text) => text,
                Err(_) => continue,
            };

            // 1) aggTrade messages
            if let Some(agg) = parse_agg_trade(payload) {
                process_agg_trade(&agg, &config_map, &mut last_prices, &tx).await;
                continue;
            }

            // 2) depth updates
            if let Some(depth) = parse_depth_update(payload) {
                process_depth_update(&depth, &config_map, &config, &mut big_move_detectors, &tx);
                continue;
            }

            // Unknown / unhandled stream messages (optional logging controlled by env)
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

async fn process_agg_trade(
    agg: &feeder_service::binance::AggTrade,
    config_map: &HashMap<String, feeder_service::config::SymbolConfig>,
    last_prices: &mut HashMap<String, f64>,
    tx: &broadcast::Sender<String>,
) {
    let symbol = agg.s.to_lowercase();
    let cfg = match config_map.get(&symbol) {
        Some(c) => c,
        None => return,
    };

    let current_price = agg.p.parse::<f64>().unwrap_or(0.0);
    let prev_price = last_prices.get(&symbol).copied();
    let spike = calc_spike(prev_price, current_price);

    last_prices.insert(symbol.clone(), current_price);

    // Preserve asynchronous logging & broadcasting behaviour
    log_and_broadcast(tx, agg, spike, cfg).await;
}

fn process_depth_update(
    depth: &feeder_service::binance_depth::DepthUpdate,
    config_map: &HashMap<String, feeder_service::config::SymbolConfig>,
    config: &Config,
    big_move_detectors: &mut HashMap<String, BigMoveDetector>,
    tx: &broadcast::Sender<String>,
) {
    let symbol = depth.symbol.to_lowercase();
    let cfg = match config_map.get(&symbol) {
        Some(c) => c,
        None => return,
    };

    let matched_bids = collect_big_levels(&depth.bids, cfg.big_trade_qty, 3);
    let matched_asks = collect_big_levels(&depth.asks, cfg.big_trade_qty, 3);

    if !is_big_depth_update(&matched_bids, &matched_asks) {
        return;
    }

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
                if !price.is_finite() || !qty.is_finite() || price <= 0.0 || qty <= 0.0 {
                    return None;
                }
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
        return;
    }

    let bid_total_notional: f64 = big_bids.iter().map(|(price, qty)| price * qty).sum();
    let ask_total_notional: f64 = big_asks.iter().map(|(price, qty)| price * qty).sum();
    let total_notional = bid_total_notional + ask_total_notional;

    let mut bid_pressure_pct = if total_notional > 0.0 {
        (bid_total_notional / total_notional) * 100.0
    } else {
        0.0
    };
    bid_pressure_pct = bid_pressure_pct.clamp(0.0, 100.0);
    let sell_pressure_pct = (100.0 - bid_pressure_pct).clamp(0.0, 100.0);

    if !passes_pressure_filter(
        bid_pressure_pct,
        sell_pressure_pct,
        config.big_depth_min_pressure_pct,
    ) {
        return;
    }

    let dominant_side = if bid_pressure_pct > sell_pressure_pct {
        "BUY"
    } else if sell_pressure_pct > bid_pressure_pct {
        "SELL"
    } else {
        "BALANCED"
    };

    let top_bid = big_bids
        .first()
        .map(|(price, qty)| format!("{:.2}x{:.3}", price, qty))
        .unwrap_or_else(|| "-".to_string());
    let top_ask = big_asks
        .first()
        .map(|(price, qty)| format!("{:.2}x{:.3}", price, qty))
        .unwrap_or_else(|| "-".to_string());

    let pressure_bar = format_pressure_visual(bid_pressure_pct, 12);

    let depth_msg = format!(
        "[DEPTH] {} {} [{}] B:{:.1}% S:{:.1}% | notional {} vs {} | top {} / {}",
        depth.symbol.to_uppercase(),
        dominant_side,
        pressure_bar,
        bid_pressure_pct,
        sell_pressure_pct,
        format_notional_compact(bid_total_notional),
        format_notional_compact(ask_total_notional),
        top_bid,
        top_ask
    );

    println!("{}", depth_msg);
    let _ = tx.send(depth_msg.clone());

    if let Some(detector) = big_move_detectors.get_mut(&symbol) {
        let snap = DepthSnapshot {
            bid_pressure_pct,
            total_notional,
        };

        match detector.push(snap) {
            BigMoveSignal::BullishBreakout {
                avg_pressure,
                total_notional,
            } => {
                let alert = format!(
                    "[BIGMOVE] {} BULLISH BREAKOUT likely! avg_pressure={:.1}% notional={:.0}",
                    depth.symbol.to_uppercase(),
                    avg_pressure,
                    total_notional
                );
                println!("{}", alert);
                let _ = tx.send(alert);
            }
            BigMoveSignal::BearishBreakout {
                avg_pressure,
                total_notional,
            } => {
                let alert = format!(
                    "[BIGMOVE] {} BEARISH BREAKOUT likely! avg_pressure={:.1}% notional={:.0}",
                    depth.symbol.to_uppercase(),
                    avg_pressure,
                    total_notional
                );
                println!("{}", alert);
                let _ = tx.send(alert);
            }
            BigMoveSignal::None => {}
        }
    }
}
