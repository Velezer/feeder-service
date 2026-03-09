use feeder_service::binance::*;
use feeder_service::binance_depth::*;
use feeder_service::binance_kline::*;
use feeder_service::config::Config;
use feeder_service::correlation::engine::CorrelationEngine;
use feeder_service::correlation::model::{MarketEvent, MarketEventKind, parse_news_event};
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

    let enable_depth = std::env::var("ENABLE_DEPTH")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false); // default enabled

    let enable_kline_quant = std::env::var("ENABLE_KLINE_QUANT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false); // inactive by default in production

    // Maps and state
    let mut config_map: HashMap<String, _> = HashMap::new();
    let mut last_prices: HashMap<String, f64> = HashMap::new();
    let mut big_move_detectors: HashMap<String, BigMoveDetector> = HashMap::new();
    let mut correlation_engine = CorrelationEngine::new(
        config.corr_min_move_pct,
        config.corr_max_lag_seconds,
        config.corr_min_confidence,
    );

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
        big_move_detectors.insert(
            cfg.symbol.to_lowercase(),
            BigMoveDetector::new(5, 75.0, 0.0, 3),
        );
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

    if config.disable_depth_stream {
        println!(
            "Depth stream DISABLED (DISABLE_DEPTH_STREAM raw value: {:?})",
            std::env::var("DISABLE_DEPTH_STREAM").unwrap_or_default()
        );
    } else {
        println!(
            "Depth filters => min_qty: {}, min_notional: {}, min_pressure: {}",
            config.big_depth_min_qty,
            config.big_depth_min_notional,
            config.big_depth_min_pressure_pct
        );
    }

    tokio::spawn(warp::serve(ws_route).run(([0, 0, 0, 0], config.port)));

    // Build Binance streams: aggTrade for each symbol + diff depth streams (unless disabled)
    let mut streams: Vec<String> = symbols.iter().map(|s| format!("{}@aggTrade", s)).collect();
    if enable_depth {
        streams.extend(build_diff_depth_streams(&symbols, 100));
    } else {
        println!("[INFO] Depth streams are disabled by feature flag.");
    }

    if enable_kline_quant {
        streams.extend(build_kline_streams(&symbols, "4h"));
    } else {
        println!(
            "[INFO] Kline quant analysis is inactive (set ENABLE_KLINE_QUANT=true to enable)."
        );
    }

    if !config.news_streams.is_empty() {
        streams.extend(config.news_streams.iter().cloned());
        println!("[INFO] Added extra news streams: {:?}", config.news_streams);
    }

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
                process_agg_trade(
                    &agg,
                    &config_map,
                    &mut last_prices,
                    &mut correlation_engine,
                    &tx,
                )
                .await;
                continue;
            }

            // 2) depth updates (skipped when DISABLE_DEPTH_STREAM is set)
            if !config.disable_depth_stream {
                if let Some(depth) = parse_depth_update(payload) {
                    process_depth_update(
                        &depth,
                        &config_map,
                        &config,
                        &mut big_move_detectors,
                        &mut correlation_engine,
                        &tx,
                    );
                    continue;
                }
            }

            // 3) kline updates (4h quant vector signal on closed candles)
            if enable_kline_quant {
                if let Some(kline_event) = parse_kline_event(payload) {
                    process_kline_event(&kline_event, &config_map, &mut correlation_engine, &tx);
                    continue;
                }
            }

            // 4) external news events used for correlation
            if let Some(news_event) = parse_news_event(payload) {
                correlation_engine.ingest_news(news_event);
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
    correlation_engine: &mut CorrelationEngine,
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
    let qty = agg.q.parse::<f64>().unwrap_or(0.0);
    let direction = if let Some(prev) = prev_price {
        if current_price > prev {
            1
        } else if current_price < prev {
            -1
        } else {
            0
        }
    } else {
        0
    };

    let market_event = MarketEvent {
        symbol: symbol.clone(),
        timestamp_ms: agg.t,
        kind: MarketEventKind::AggTrade,
        move_pct: spike,
        notional: current_price * qty,
        direction,
    };

    emit_correlation(correlation_engine.on_market_event(market_event), tx);

    last_prices.insert(symbol.clone(), current_price);

    // Preserve asynchronous logging & broadcasting behaviour
    log_and_broadcast(tx, agg, spike, cfg).await;
}

fn process_depth_update(
    depth: &feeder_service::binance_depth::DepthUpdate,
    config_map: &HashMap<String, feeder_service::config::SymbolConfig>,
    config: &Config,
    big_move_detectors: &mut HashMap<String, BigMoveDetector>,
    correlation_engine: &mut CorrelationEngine,
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

    let pressure_move = ((bid_pressure_pct - 50.0).abs() / 50.0) * 100.0;
    let depth_market_event = MarketEvent {
        symbol: symbol.clone(),
        timestamp_ms: depth.event_time,
        kind: MarketEventKind::DepthPressure,
        move_pct: pressure_move,
        notional: total_notional,
        direction: if dominant_side == "BUY" {
            1
        } else if dominant_side == "SELL" {
            -1
        } else {
            0
        },
    };

    emit_correlation(correlation_engine.on_market_event(depth_market_event), tx);

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

fn process_kline_event(
    event: &feeder_service::binance_kline::KlineEvent,
    config_map: &HashMap<String, feeder_service::config::SymbolConfig>,
    correlation_engine: &mut CorrelationEngine,
    tx: &broadcast::Sender<String>,
) {
    let symbol = event.symbol.to_lowercase();
    if !config_map.contains_key(&symbol) {
        return;
    }

    if let Some(signal) = build_quant_signal_from_kline(event) {
        let market_event = MarketEvent {
            symbol: symbol.clone(),
            timestamp_ms: signal.interval_end_ms,
            kind: MarketEventKind::KlineClose,
            move_pct: signal.return_pct.abs(),
            notional: signal.quote_volume,
            direction: if signal.return_pct > 0.0 {
                1
            } else if signal.return_pct < 0.0 {
                -1
            } else {
                0
            },
        };

        emit_correlation(correlation_engine.on_market_event(market_event), tx);

        let direction = if signal.return_pct > 0.0 {
            "BULLISH"
        } else if signal.return_pct < 0.0 {
            "BEARISH"
        } else {
            "FLAT"
        };

        let msg = format!(
            "[QUANT4H] {} {} | window={}..{} | O:{:.2} C:{:.2} H:{:.2} L:{:.2} ret={:+.2}% range={:.2}% taker_buy={:.1}% qvol={:.0} trades={}",
            signal.symbol.to_uppercase(),
            direction,
            signal.interval_start_ms,
            signal.interval_end_ms,
            signal.open,
            signal.close,
            signal.high,
            signal.low,
            signal.return_pct,
            signal.range_pct,
            signal.taker_buy_ratio_pct,
            signal.quote_volume,
            signal.trade_count
        );

        println!("{}", msg);
        let _ = tx.send(msg);
    }
}

fn emit_correlation(
    signal: Option<feeder_service::correlation::model::CorrelationSignal>,
    tx: &broadcast::Sender<String>,
) {
    let Some(signal) = signal else {
        return;
    };

    let kind = match signal.market_event_kind {
        MarketEventKind::AggTrade => "aggTrade",
        MarketEventKind::DepthPressure => "depth",
        MarketEventKind::KlineClose => "kline",
    };

    let corr_msg = format!(
        "[NEWS_CORR] {} kind={} conf={:.2} lag={}ms move={:+.3}% notional={:.0} windows=5m:{} 15m:{} 1h:{} headline=\"{}\"",
        signal.symbol.to_uppercase(),
        kind,
        signal.confidence,
        signal.lag_ms,
        signal.move_pct,
        signal.notional,
        signal.window_5m_count,
        signal.window_15m_count,
        signal.window_1h_count,
        signal.news_headline,
    );

    println!("{}", corr_msg);
    let _ = tx.send(corr_msg);
}
