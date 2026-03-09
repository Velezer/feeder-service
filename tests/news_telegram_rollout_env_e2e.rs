use feeder_service::config::Config;

fn clear_env(keys: &[&str]) {
    for key in keys {
        unsafe { std::env::remove_var(key) };
    }
}

#[test]
fn rollout_flags_and_thresholds_have_safe_defaults_and_expected_overrides() {
    let keys = [
        "SYMBOLS",
        "ENABLE_NEWS_INGEST",
        "ENABLE_TELEGRAM_NOTIFIER",
        "DISABLE_DEPTH_STREAM",
        "TELEGRAM_MIN_CORRELATION_SCORE",
        "TELEGRAM_RATE_LIMIT_INTERVAL_SECS",
        "NEWS_DB_PATH",
    ];

    clear_env(&keys);

    let defaults = Config::load();
    assert!(!defaults.news.enabled);
    assert!(!defaults.telegram.enabled);
    assert!(!defaults.disable_depth_stream);
    assert_eq!(defaults.telegram.min_correlation_score, 0.0);
    assert_eq!(defaults.telegram.rate_limit_interval_secs, 30);
    assert_eq!(defaults.news.db_path, "news.sqlite");

    unsafe { std::env::set_var("SYMBOLS", "btcusdt") };
    unsafe { std::env::set_var("ENABLE_NEWS_INGEST", "true") };
    unsafe { std::env::set_var("ENABLE_TELEGRAM_NOTIFIER", "1") };
    unsafe { std::env::set_var("DISABLE_DEPTH_STREAM", "yes") };
    unsafe { std::env::set_var("TELEGRAM_MIN_CORRELATION_SCORE", "0.55") };
    unsafe { std::env::set_var("TELEGRAM_RATE_LIMIT_INTERVAL_SECS", "45") };
    unsafe { std::env::set_var("NEWS_DB_PATH", "/tmp/canary-news.sqlite") };

    let configured = Config::load();
    assert!(configured.news.enabled);
    assert!(configured.telegram.enabled);
    assert!(configured.disable_depth_stream);
    assert!((configured.telegram.min_correlation_score - 0.55).abs() < f64::EPSILON);
    assert_eq!(configured.telegram.rate_limit_interval_secs, 45);
    assert_eq!(configured.news.db_path, "/tmp/canary-news.sqlite");

    clear_env(&keys);
}
