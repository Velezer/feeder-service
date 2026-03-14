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

#[derive(Clone, Debug, PartialEq)]
pub enum BigMoveSignal {
    BullishBreakout {
        avg_pressure: f64,
        total_notional: f64,
    },
    BearishBreakout {
        avg_pressure: f64,
        total_notional: f64,
    },
    None,
}

#[derive(Debug, PartialEq)]
pub struct BigMoveEvaluation {
    pub signal: BigMoveSignal,
    pub self_explanation_log: String,
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
        self.push_with_self_explanation(snapshot).signal
    }

    /// Push a new depth snapshot and evaluate with an operator-facing explanation line.
    pub fn push_with_self_explanation(&mut self, snapshot: DepthSnapshot) -> BigMoveEvaluation {
        if self.window.len() == self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(snapshot);

        if self.window.len() < self.min_consecutive {
            return BigMoveEvaluation {
                signal: BigMoveSignal::None,
                self_explanation_log: format!(
                    "[BIGMOVE][SELF_EXPLAIN] insufficient history ({}/{})",
                    self.window.len(),
                    self.min_consecutive
                ),
            };
        }

        // Check last `min_consecutive` snapshots for consistent extreme pressure
        let recent: Vec<&DepthSnapshot> = self
            .window
            .iter()
            .rev()
            .take(self.min_consecutive)
            .collect();

        let all_bullish = recent
            .iter()
            .all(|s| s.bid_pressure_pct >= self.pressure_threshold);
        let all_bearish = recent
            .iter()
            .all(|s| (100.0 - s.bid_pressure_pct) >= self.pressure_threshold);

        if !all_bullish && !all_bearish {
            return BigMoveEvaluation {
                signal: BigMoveSignal::None,
                self_explanation_log: format!(
                    "[BIGMOVE][SELF_EXPLAIN] no consistent extreme pressure in last {} snapshots (threshold={:.1}%)",
                    self.min_consecutive,
                    self.pressure_threshold
                ),
            };
        }

        let avg_pressure: f64 =
            recent.iter().map(|s| s.bid_pressure_pct).sum::<f64>() / recent.len() as f64;
        let avg_notional: f64 =
            recent.iter().map(|s| s.total_notional).sum::<f64>() / recent.len() as f64;

        if avg_notional < self.min_total_notional {
            return BigMoveEvaluation {
                signal: BigMoveSignal::None,
                self_explanation_log: format!(
                    "[BIGMOVE][SELF_EXPLAIN] pressure extreme but notional too small ({:.0} < {:.0})",
                    avg_notional,
                    self.min_total_notional
                ),
            };
        }

        if all_bullish {
            BigMoveEvaluation {
                signal: BigMoveSignal::BullishBreakout {
                    avg_pressure,
                    total_notional: avg_notional,
                },
                self_explanation_log: format!(
                    "[BIGMOVE][SELF_EXPLAIN] bullish breakout: avg_pressure={:.1}% avg_notional={:.0} threshold={:.1}%",
                    avg_pressure,
                    avg_notional,
                    self.pressure_threshold
                ),
            }
        } else {
            let bearish_pressure = 100.0 - avg_pressure;
            BigMoveEvaluation {
                signal: BigMoveSignal::BearishBreakout {
                    avg_pressure: bearish_pressure,
                    total_notional: avg_notional,
                },
                self_explanation_log: format!(
                    "[BIGMOVE][SELF_EXPLAIN] bearish breakout: avg_pressure={:.1}% avg_notional={:.0} threshold={:.1}%",
                    bearish_pressure,
                    avg_notional,
                    self.pressure_threshold
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BigMoveDetector, BigMoveSignal, DepthSnapshot};

    #[test]
    fn self_explanation_reports_insufficient_history() {
        let mut detector = BigMoveDetector::new(5, 75.0, 0.0, 3);
        let result = detector.push_with_self_explanation(DepthSnapshot {
            bid_pressure_pct: 80.0,
            total_notional: 10_000.0,
        });

        assert_eq!(result.signal, BigMoveSignal::None);
        assert!(result.self_explanation_log.contains("insufficient history"));
    }

    #[test]
    fn self_explanation_reports_bullish_breakout() {
        let mut detector = BigMoveDetector::new(5, 75.0, 0.0, 3);
        for pressure in [80.0, 82.0, 85.0] {
            let _ = detector.push_with_self_explanation(DepthSnapshot {
                bid_pressure_pct: pressure,
                total_notional: 50_000.0,
            });
        }

        let result = detector.push_with_self_explanation(DepthSnapshot {
            bid_pressure_pct: 88.0,
            total_notional: 50_000.0,
        });

        assert!(matches!(result.signal, BigMoveSignal::BullishBreakout { .. }));
        assert!(result.self_explanation_log.contains("bullish breakout"));
    }

    #[test]
    fn self_explanation_reports_notional_filter_failure() {
        let mut detector = BigMoveDetector::new(5, 75.0, 100_000.0, 3);
        let _ = detector.push_with_self_explanation(DepthSnapshot {
            bid_pressure_pct: 80.0,
            total_notional: 1_000.0,
        });
        let _ = detector.push_with_self_explanation(DepthSnapshot {
            bid_pressure_pct: 81.0,
            total_notional: 1_000.0,
        });
        let result = detector.push_with_self_explanation(DepthSnapshot {
            bid_pressure_pct: 82.0,
            total_notional: 1_000.0,
        });

        assert_eq!(result.signal, BigMoveSignal::None);
        assert!(result.self_explanation_log.contains("notional too small"));
    }
}
