use feeder_service::binance::*;
use feeder_service::binance_depth::*;
use feeder_service::config::Config;
use feeder_service::refactor::AppState;
use feeder_service::ws_helpers::*;
use futures_util::StreamExt;
use local_ip_address::local_ip;
use std::sync::{Arc, Mutex};
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

    let app_state = Arc::new(Mutex::new(AppState::new(config.clone())));

    let symbols: Vec<String> = config
        .symbols
        .iter()
        .map(|cfg| cfg.symbol.clone())
        .collect();

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
                let mut state = app_state.lock().unwrap();
                state.process_agg_trade(&agg, &tx).await;
                continue;
            }

            // 2) depth updates
            if let Some(depth) = parse_depth_update(payload) {
                let mut state = app_state.lock().unwrap();
                state.process_depth_update(&depth, &tx);
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
