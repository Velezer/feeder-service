// File: src/time_helpers.rs
use chrono::{DateTime, FixedOffset, TimeZone, Utc};

pub fn utc_to_utc7(ms: i64) -> DateTime<FixedOffset> {
    let utc_ts = Utc.timestamp_millis_opt(ms).unwrap(); // safe unwrap if you trust timestamps
    let offset = FixedOffset::east_opt(7 * 3600).unwrap();
    utc_ts.with_timezone(&offset)
}

pub fn format_ts(dt: DateTime<FixedOffset>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}
