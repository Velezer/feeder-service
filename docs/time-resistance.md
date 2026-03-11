# Time Resistance (Daily Timeframe)

In your TA naming, **time resistance** is the next daily time boundary where price is more likely
to react or reverse.

This implementation is lightweight and deterministic:
- no external API calls
- O(1) arithmetic only
- UTC-millisecond input/output

## Daily boundary model

Daily time resistance is the **next daily session rollover**.

You can choose session anchor by UTC offset:
- `0`  => boundary at `00:00 UTC`
- `8`  => boundary at `00:00 UTC+8` (`16:00 UTC`)
- `-5` => boundary at `00:00 UTC-5` (`05:00 UTC`)

## Dynamic astrology logic (non-static)

Time resistance probability is no longer static. It now combines:

1. **Base time proximity score** (near boundary => higher)
2. **Lunar cycle factor** (astrology component)

### Base time score

`base_prob = max(0, 1 - time_to_boundary / window) * 100`

### Lunar factor

- Uses lunar synodic cycle (`~29.53 days`) from a fixed new-moon epoch.
- Produces `lunar_factor` in `[0, 1]`.
- Emphasizes new/full moon reversal tendency with a cosine-based curve.

### Final probability

`reversal_prob = base_prob * (1 - astro_weight) + (lunar_factor * 100) * astro_weight`

- `astro_weight` is clamped to `[0,1]`.
- `0` => pure time model.
- `1` => pure lunar astrology model.

## API

- `next_daily_time_resistance_ms(now_ms, session_utc_offset_hours)`
- `daily_reversal_probability_pct(now_ms, next_boundary_ms, window_minutes)`
- `lunar_phase_reversal_factor(now_ms)`
- `astrology_adjusted_reversal_probability_pct(now_ms, next_boundary_ms, window_minutes, astro_weight)`
- `format_daily_time_resistance_log(now_ms, session_utc_offset_hours, reversal_window_minutes, astro_weight)`
- `next_time_resistance_ms(now_ms, interval_minutes)`

## Log output

At runtime, service prints one daily time-resistance line on startup:

`[TIME_RESISTANCE][1D] next=<UTC datetime> UTC | session_offset=<+/-hours> | session_time=<local session datetime> | ts_ms=<unix_ms> | reversal_prob=<percent> | base_prob=<percent> | lunar_factor=<0..1> | astro_weight=<0..1> | window=<minutes>m`

## Env vars

- `TIME_RESISTANCE_DAILY_UTC_OFFSET_HOURS` (default: `0`)
- `TIME_RESISTANCE_REVERSAL_WINDOW_MINUTES` (default: `360`)
- `TIME_RESISTANCE_ASTRO_WEIGHT` (default: `0.35`)
