// File: src/time_helpers.rs
use chrono::{DateTime, FixedOffset, TimeZone, Utc};

const MS_PER_MINUTE: i64 = 60_000;
const MINUTES_PER_DAY: i64 = 1_440;
const MS_PER_DAY: i64 = 86_400_000;
const LUNAR_CYCLE_DAYS: f64 = 29.530_588_853;
const KNOWN_NEW_MOON_MS: i64 = 947_182_440_000; // 2000-01-06 18:14:00 UTC

pub fn utc_to_utc7(ms: i64) -> DateTime<FixedOffset> {
    let utc_ts = Utc.timestamp_millis_opt(ms).unwrap(); // safe unwrap if you trust timestamps
    let offset = FixedOffset::east_opt(7 * 3600).unwrap();
    utc_ts.with_timezone(&offset)
}

pub fn format_ts(dt: DateTime<FixedOffset>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn next_aligned_boundary_ms(now_ms: i64, interval_minutes: i64) -> Option<i64> {
    if now_ms < 0 || interval_minutes <= 0 {
        return None;
    }

    let interval_ms = interval_minutes.checked_mul(MS_PER_MINUTE)?;
    let periods_elapsed = now_ms.div_euclid(interval_ms);
    let next_period = periods_elapsed.checked_add(1)?;
    next_period.checked_mul(interval_ms)
}

/// Returns the next UTC timestamp (milliseconds) at which a candle closes for the given
/// `interval_minutes`.
pub fn next_time_resistance_ms(now_ms: i64, interval_minutes: i64) -> Option<i64> {
    next_aligned_boundary_ms(now_ms, interval_minutes)
}

/// Returns the next daily time-resistance boundary (milliseconds).
pub fn next_daily_time_resistance_ms(now_ms: i64, session_utc_offset_hours: i32) -> Option<i64> {
    if now_ms < 0 {
        return None;
    }

    let offset_minutes = i64::from(session_utc_offset_hours).checked_mul(60)?;
    let offset_ms = offset_minutes.checked_mul(MS_PER_MINUTE)?;
    let interval_ms = MINUTES_PER_DAY.checked_mul(MS_PER_MINUTE)?;

    let shifted = now_ms.checked_add(offset_ms)?;
    let periods_elapsed = shifted.div_euclid(interval_ms);
    let next_period = periods_elapsed.checked_add(1)?;
    next_period.checked_mul(interval_ms)?.checked_sub(offset_ms)
}

/// Base probability from time distance to boundary.
pub fn daily_reversal_probability_pct(
    now_ms: i64,
    next_boundary_ms: i64,
    window_minutes: i64,
) -> Option<f64> {
    if now_ms < 0 || next_boundary_ms < now_ms || window_minutes <= 0 {
        return None;
    }

    let window_ms = window_minutes.checked_mul(MS_PER_MINUTE)?;
    let time_to_boundary_ms = next_boundary_ms.checked_sub(now_ms)?;

    if time_to_boundary_ms >= window_ms {
        return Some(0.0);
    }

    let ratio = 1.0 - (time_to_boundary_ms as f64 / window_ms as f64);
    Some((ratio * 100.0).clamp(0.0, 100.0))
}

/// Astrology factor using lunar synodic cycle phase.
///
/// Returns value in [0, 1], where 1 means a stronger reversal tendency.
///
/// We emphasize New Moon and Full Moon by using `abs(cos(phase * PI))` where
/// phase is in [0,1) across a lunar cycle.
pub fn lunar_phase_reversal_factor(now_ms: i64) -> Option<f64> {
    if now_ms < 0 {
        return None;
    }

    let elapsed_days = (now_ms - KNOWN_NEW_MOON_MS) as f64 / MS_PER_DAY as f64;
    let phase = elapsed_days.rem_euclid(LUNAR_CYCLE_DAYS) / LUNAR_CYCLE_DAYS;
    let factor = (std::f64::consts::PI * phase).cos().abs();
    Some(factor.clamp(0.0, 1.0))
}

/// Dynamic probability = weighted blend of base time score and lunar-phase score.
///
/// `astro_weight` is clamped to [0, 1].
pub fn astrology_adjusted_reversal_probability_pct(
    now_ms: i64,
    next_boundary_ms: i64,
    window_minutes: i64,
    astro_weight: f64,
) -> Option<f64> {
    let base_pct = daily_reversal_probability_pct(now_ms, next_boundary_ms, window_minutes)?;
    let lunar_pct = lunar_phase_reversal_factor(now_ms)? * 100.0;
    let w = astro_weight.clamp(0.0, 1.0);
    Some((base_pct * (1.0 - w) + lunar_pct * w).clamp(0.0, 100.0))
}

pub fn format_daily_time_resistance_log(
    now_ms: i64,
    session_utc_offset_hours: i32,
    reversal_window_minutes: i64,
    astro_weight: f64,
) -> Option<String> {
    let next_ms = next_daily_time_resistance_ms(now_ms, session_utc_offset_hours)?;
    let next_utc = Utc.timestamp_millis_opt(next_ms).single()?;
    let session_offset = FixedOffset::east_opt(session_utc_offset_hours.checked_mul(3600)?)?;
    let next_session = next_utc.with_timezone(&session_offset);

    let base_prob = daily_reversal_probability_pct(now_ms, next_ms, reversal_window_minutes)?;
    let lunar_factor = lunar_phase_reversal_factor(now_ms)?;
    let adjusted_prob = astrology_adjusted_reversal_probability_pct(
        now_ms,
        next_ms,
        reversal_window_minutes,
        astro_weight,
    )?;

    Some(format!(
        "[TIME_RESISTANCE][1D] next={} UTC | session_offset={:+} | session_time={} | ts_ms={} | reversal_prob={:.2}% | base_prob={:.2}% | lunar_factor={:.4} | astro_weight={:.2} | window={}m",
        next_utc.format("%Y-%m-%d %H:%M:%S"),
        session_utc_offset_hours,
        next_session.format("%Y-%m-%d %H:%M:%S %:z"),
        next_ms,
        adjusted_prob,
        base_prob,
        lunar_factor,
        astro_weight.clamp(0.0, 1.0),
        reversal_window_minutes
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        astrology_adjusted_reversal_probability_pct, daily_reversal_probability_pct,
        format_daily_time_resistance_log, lunar_phase_reversal_factor,
        next_daily_time_resistance_ms, next_time_resistance_ms,
    };

    #[test]
    fn rounds_up_to_next_hour_boundary() {
        let now_ms = 1_773_234_256_000;
        let next = next_time_resistance_ms(now_ms, 60).unwrap();
        assert_eq!(next, 1_773_237_600_000);
    }

    #[test]
    fn exact_boundary_moves_to_next_boundary() {
        let now_ms = 1_773_237_600_000;
        let next = next_time_resistance_ms(now_ms, 60).unwrap();
        assert_eq!(next, 1_773_241_200_000);
    }

    #[test]
    fn daily_boundary_uses_utc_midnight_when_offset_zero() {
        let now_ms = 1_773_234_256_000;
        let next = next_daily_time_resistance_ms(now_ms, 0).unwrap();
        assert_eq!(next, 1_773_273_600_000);
    }

    #[test]
    fn daily_boundary_supports_session_offset() {
        let now_ms = 1_773_234_256_000;
        let next = next_daily_time_resistance_ms(now_ms, 8).unwrap();
        assert_eq!(next, 1_773_244_800_000);
    }

    #[test]
    fn reversal_probability_is_zero_far_from_boundary() {
        let now_ms = 1_773_216_000_000;
        let next_ms = 1_773_244_800_000;
        let pct = daily_reversal_probability_pct(now_ms, next_ms, 360).unwrap();
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn reversal_probability_grows_inside_window() {
        let now_ms = 1_773_234_000_000;
        let next_ms = 1_773_244_800_000;
        let pct = daily_reversal_probability_pct(now_ms, next_ms, 360).unwrap();
        assert_eq!(pct, 50.0);
    }

    #[test]
    fn lunar_factor_is_normalized() {
        let factor = lunar_phase_reversal_factor(1_773_234_256_000).unwrap();
        assert!((0.0..=1.0).contains(&factor));
    }

    #[test]
    fn astrology_adjustment_changes_probability_when_weight_is_nonzero() {
        let now_ms = 1_773_234_000_000;
        let next_ms = 1_773_244_800_000;
        let base = daily_reversal_probability_pct(now_ms, next_ms, 360).unwrap();
        let adjusted =
            astrology_adjusted_reversal_probability_pct(now_ms, next_ms, 360, 0.35).unwrap();
        assert_ne!(base, adjusted);
    }

    #[test]
    fn astrology_weight_boundaries_work() {
        let now_ms = 1_773_234_000_000;
        let next_ms = 1_773_244_800_000;
        let base = daily_reversal_probability_pct(now_ms, next_ms, 360).unwrap();
        let lunar = lunar_phase_reversal_factor(now_ms).unwrap() * 100.0;

        let w0 = astrology_adjusted_reversal_probability_pct(now_ms, next_ms, 360, 0.0).unwrap();
        let w1 = astrology_adjusted_reversal_probability_pct(now_ms, next_ms, 360, 1.0).unwrap();
        let w_over =
            astrology_adjusted_reversal_probability_pct(now_ms, next_ms, 360, 2.0).unwrap();

        assert_eq!(w0, base);
        assert_eq!(w1, lunar);
        assert_eq!(w_over, lunar);
    }

    #[test]
    fn daily_log_line_includes_astrology_fields() {
        let now_ms = 1_773_234_256_000;
        let line = format_daily_time_resistance_log(now_ms, 8, 360, 0.35).unwrap();

        assert!(line.contains("[TIME_RESISTANCE][1D]"));
        assert!(line.contains("reversal_prob="));
        assert!(line.contains("base_prob="));
        assert!(line.contains("lunar_factor="));
        assert!(line.contains("astro_weight=0.35"));
        assert!(line.contains("window=360m"));
    }

    #[test]
    fn returns_none_for_invalid_inputs() {
        assert_eq!(next_time_resistance_ms(-1, 60), None);
        assert_eq!(next_time_resistance_ms(0, 0), None);
        assert_eq!(next_time_resistance_ms(0, -15), None);
        assert_eq!(next_daily_time_resistance_ms(-1, 0), None);
        assert_eq!(daily_reversal_probability_pct(-1, 10, 60), None);
        assert_eq!(daily_reversal_probability_pct(10, 5, 60), None);
        assert_eq!(daily_reversal_probability_pct(10, 20, 0), None);
        assert_eq!(lunar_phase_reversal_factor(-1), None);
        assert_eq!(
            astrology_adjusted_reversal_probability_pct(-1, 20, 60, 0.5),
            None
        );
        assert_eq!(format_daily_time_resistance_log(-1, 0, 360, 0.35), None);
    }
}
