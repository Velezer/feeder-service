use std::collections::HashMap;
use tokio::sync::broadcast;

use crate::{
    binance::{calc_spike, log_and_broadcast, AggTrade},
    binance_depth::{
        collect_big_levels, format_notional_compact, format_pressure_visual, is_big_depth_update,
        passes_pressure_filter, DepthUpdate,
    },
    config::{Config, SymbolConfig},
};

use self::big_move_detector::{BigMoveDetector, BigMoveSignal, DepthSnapshot};

pub mod big_move_detector;

/// Shared application state
pub struct AppState {
    /// App configuration
    pub config: Config,
    /// Map of symbol to symbol-specific configuration
    config_map: HashMap<String, SymbolConfig>,
    /// Map of symbol to last trade price
    last_prices: HashMap<String, f64>,
    /// Map of symbol to big move detector
    big_move_detectors: HashMap<String, BigMoveDetector>,
}

impl AppState {
    /// Create a new AppState
    pub fn new(config: Config) -> Self {
        let mut config_map = HashMap::new();
        let mut big_move_detectors = HashMap::new();

        for cfg in &config.symbols {
            println!(
                "Symbol: {} => Big Trade Qty: {}, Spike %: {}",
                cfg.symbol.to_uppercase(),
                cfg.big_trade_qty,
                cfg.spike_pct
            );
            config_map.insert(cfg.symbol.clone(), cfg.clone());
            big_move_detectors.insert(cfg.symbol.to_lowercase(), BigMoveDetector::new(5, 75.0, 0.0, 3));
        }

        Self {
            config,
            config_map,
            last_prices: HashMap::new(),
            big_move_detectors,
        }
    }

    pub async fn process_agg_trade(
        &mut self,
        agg: &AggTrade,
        tx: &broadcast::Sender<String>,
    ) {
        let symbol = agg.s.to_lowercase();
        let cfg = match self.config_map.get(&symbol) {
            Some(c) => c,
            None => return,
        };

        let current_price = agg.p.parse::<f64>().unwrap_or(0.0);
        let prev_price = self.last_prices.get(&symbol).copied();
        let spike = calc_spike(prev_price, current_price);

        self.last_prices.insert(symbol.clone(), current_price);

        log_and_broadcast(tx, agg, spike, cfg).await;
    }

    pub fn process_depth_update(
        &mut self,
        depth: &DepthUpdate,
        tx: &broadcast::Sender<String>,
    ) {
        let symbol = depth.symbol.to_lowercase();
        let cfg = match self.config_map.get(&symbol) {
            Some(c) => c,
            None => return,
        };

        let matched_bids = collect_big_levels(&depth.bids, cfg.big_trade_qty, 3);
        let matched_asks = collect_big_levels(&depth.asks, cfg.big_trade_qty, 3);

        if !is_big_depth_update(&matched_bids, &matched_asks) {
            return;
        }

        let (big_bids, big_asks) = self.extract_big_levels(depth);

        if big_bids.is_empty() && big_asks.is_empty() {
            return;
        }

        let (bid_pressure_pct, sell_pressure_pct, total_notional) =
            Self::calculate_pressure(&big_bids, &big_asks);

        if !passes_pressure_filter(
            bid_pressure_pct,
            sell_pressure_pct,
            self.config.big_depth_min_pressure_pct,
        ) {
            return;
        }

        let depth_msg =
            Self::format_depth_message(depth, &big_bids, &big_asks, bid_pressure_pct);

        println!("{}", depth_msg);
        let _ = tx.send(depth_msg.clone());

        self.detect_big_move(
            &symbol,
            bid_pressure_pct,
            total_notional,
            depth,
            tx,
        );
    }

    fn is_level_big(&self, price: f64, qty: f64) -> bool {
        let min_qty = self.config.big_depth_min_qty;
        let min_notional = self.config.big_depth_min_notional;

        if min_qty <= 0.0 && min_notional <= 0.0 {
            return true;
        }
        let qty_ok = min_qty > 0.0 && qty >= min_qty;
        let notional_ok = min_notional > 0.0 && (price * qty) >= min_notional;
        qty_ok || notional_ok
    }

    fn extract_big_levels(
        &self,
        depth: &DepthUpdate,
    ) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let extract = |levels: &[[String; 2]]| {
            levels
                .iter()
                .filter_map(|level| {
                    let price = level[0].parse::<f64>().ok()?;
                    let qty = level[1].parse::<f64>().ok()?;
                    if !price.is_finite() || !qty.is_finite() || price <= 0.0 || qty <= 0.0 {
                        return None;
                    }
                    if !self.is_level_big(price, qty) {
                        return None;
                    }
                    Some((price, qty))
                })
                .collect::<Vec<(f64, f64)>>()
        };

        (extract(&depth.bids), extract(&depth.asks))
    }

    fn calculate_pressure(
        big_bids: &[(f64, f64)],
        big_asks: &[(f64, f64)],
    ) -> (f64, f64, f64) {
        let bid_total_notional: f64 = big_bids.iter().map(|(price, qty)| price * qty).sum();
        let ask_total_notional: f64 = big_asks.iter().map(|(price, qty)| price * qty).sum();
        let total_notional = bid_total_notional + ask_total_notional;

        let bid_pressure_pct = if total_notional > 0.0 {
            (bid_total_notional / total_notional) * 100.0
        } else {
            0.0
        };

        let bid_pressure_pct = bid_pressure_pct.clamp(0.0, 100.0);
        let sell_pressure_pct = (100.0 - bid_pressure_pct).clamp(0.0, 100.0);

        (bid_pressure_pct, sell_pressure_pct, total_notional)
    }

    fn format_depth_message(
        depth: &DepthUpdate,
        big_bids: &[(f64, f64)],
        big_asks: &[(f64, f64)],
        bid_pressure_pct: f64,
    ) -> String {
        let sell_pressure_pct = (100.0 - bid_pressure_pct).clamp(0.0, 100.0);
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
        let bid_total_notional: f64 = big_bids.iter().map(|(price, qty)| price * qty).sum();
        let ask_total_notional: f64 = big_asks.iter().map(|(price, qty)| price * qty).sum();

        format!(
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
        )
    }

    fn detect_big_move(
        &mut self,
        symbol: &str,
        bid_pressure_pct: f64,
        total_notional: f64,
        depth: &DepthUpdate,
        tx: &broadcast::Sender<String>,
    ) {
        if let Some(detector) = self.big_move_detectors.get_mut(symbol) {
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
}
