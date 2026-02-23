use std::collections::VecDeque;

/// Rolling window of depth pressure snapshots per symbol.
/// Fires a big-move signal when:
///   1. The rolling average pressure is extreme (>= threshold), AND
///   2. Total notional exceeds a minimum, AND
///   3. The direction has been consistent for `min_consecutive` snapshots.
pub struct BigMoveDetector {
    window: VecDeque<DepthSnapshot>,
    window_size: usize,
    /// 0-100. e.g. 75.0 means 75% bid or sell pressure triggers alert.
    pressure_threshold: f64,
    /// Minimum total notional (bid+ask) to consider the signal meaningful.
    min_total_notional: f64,
    /// How many consecutive same-side readings are required.
    min_consecutive: usize,
}

#[derive(Clone)]
pub struct DepthSnapshot {
    pub bid_pressure_pct: f64,
    pub total_notional: f64,
}

#[derive(Debug, PartialEq)]
pub enum BigMoveSignal {
    BullishBreakout { avg_pressure: f64, total_notional: f64 },
    BearishBreakout { avg_pressure: f64, total_notional: f64 },
    None,
}

impl BigMoveDetector {
    pub fn new(
        window_size: usize,
        pressure_threshold: f64,
        min_total_notional: f64,
        min_consecutive: usize,
    ) -> Self {
        Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            pressure_threshold,
            min_total_notional,
            min_consecutive,
        }
    }

    /// Push a new depth snapshot and evaluate.
    pub fn push(&mut self, snapshot: DepthSnapshot) -> BigMoveSignal {
        if self.window.len() == self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(snapshot);

        if self.window.len() < self.min_consecutive {
            return BigMoveSignal::None;
        }

        // Check last `min_consecutive` snapshots for consistent extreme pressure
        let recent: Vec<&DepthSnapshot> = self.window.iter().rev().take(self.min_consecutive).collect();

        let all_bullish = recent.iter().all(|s| s.bid_pressure_pct >= self.pressure_threshold);
        let all_bearish = recent.iter().all(|s| (100.0 - s.bid_pressure_pct) >= self.pressure_threshold);

        if !all_bullish && !all_bearish {
            return BigMoveSignal::None;
        }

        let avg_pressure: f64 = recent.iter().map(|s| s.bid_pressure_pct).sum::<f64>() / recent.len() as f64;
        let avg_notional: f64 = recent.iter().map(|s| s.total_notional).sum::<f64>() / recent.len() as f64;

        if avg_notional < self.min_total_notional {
            return BigMoveSignal::None;
        }

        if all_bullish {
            BigMoveSignal::BullishBreakout { avg_pressure, total_notional: avg_notional }
        } else {
            BigMoveSignal::BearishBreakout { avg_pressure: 100.0 - avg_pressure, total_notional: avg_notional }
        }
    }
}
