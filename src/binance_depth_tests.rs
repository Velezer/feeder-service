use super::*;

#[test]
fn parse_depth_update_from_combined_stream() {
    let msg = r#"{"stream":"btcusdt@depth@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"BTCUSDT","U":157,"u":160,"b":[["24100.10","1.20"]],"a":[["24100.20","0.80"]]}}"#;
    let depth = parse_depth_update(msg).expect("depth should parse");

    assert_eq!(depth.symbol, "BTCUSDT");
    assert_eq!(depth.event_time, 1672515782136);
    assert_eq!(depth.first_update_id, 157);
    assert_eq!(depth.final_update_id, 160);
    assert_eq!(depth.bids[0], ["24100.10".to_string(), "1.20".to_string()]);
    assert_eq!(depth.asks[0], ["24100.20".to_string(), "0.80".to_string()]);
}

#[test]
fn parse_depth_level_rejects_non_finite_and_non_positive_values() {
    let invalid_nan = ["NaN".to_string(), "5.0".to_string()];
    let invalid_inf = ["24100.10".to_string(), "inf".to_string()];
    let invalid_zero = ["24100.10".to_string(), "0".to_string()];
    let invalid_negative = ["-1".to_string(), "1.0".to_string()];

    assert!(parse_depth_level(&invalid_nan).is_none());
    assert!(parse_depth_level(&invalid_inf).is_none());
    assert!(parse_depth_level(&invalid_zero).is_none());
    assert!(parse_depth_level(&invalid_negative).is_none());
}

#[test]
fn collect_big_levels_orders_by_notional_and_limits_matches() {
    let levels = vec![
        ["24100.10".to_string(), "1.0".to_string()],
        ["24100.00".to_string(), "2.0".to_string()],
        ["24000.00".to_string(), "5.0".to_string()],
    ];

    let matches = collect_big_levels(&levels, 1.0, 2);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].price, 24000.0);
    assert_eq!(matches[0].qty, 5.0);
    assert_eq!(matches[1].price, 24100.0);
    assert_eq!(matches[1].qty, 2.0);
}

#[test]
fn collect_big_levels_skips_malformed_levels() {
    let levels = vec![
        ["oops".to_string(), "2.0".to_string()],
        ["24100.00".to_string(), "bad".to_string()],
        ["24100.50".to_string(), "2.5".to_string()],
    ];

    let matches = collect_big_levels(&levels, 2.0, 10);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].price, 24100.50);
    assert_eq!(matches[0].qty, 2.5);
}

#[test]
fn is_big_depth_update_requires_any_side_match() {
    let none: Vec<ParsedDepthLevel> = vec![];
    let some = vec![ParsedDepthLevel {
        price: 24100.10,
        qty: 3.0,
        notional: 72300.3,
    }];

    assert!(!is_big_depth_update(&none, &none));
    assert!(is_big_depth_update(&some, &none));
}

#[test]
fn passes_pressure_filter_accepts_balanced_when_disabled() {
    assert!(passes_pressure_filter(50.0, 50.0, 0.0));
}

#[test]
fn passes_pressure_filter_requires_directional_imbalance() {
    assert!(!passes_pressure_filter(52.0, 48.0, 60.0));
    assert!(passes_pressure_filter(70.0, 30.0, 60.0));
    assert!(passes_pressure_filter(35.0, 65.0, 60.0));
}

#[test]
fn passes_pressure_filter_clamps_out_of_range_values() {
    assert!(passes_pressure_filter(120.0, -20.0, 60.0));
    assert!(!passes_pressure_filter(-10.0, -5.0, 60.0));
}

#[test]
fn format_depth_levels_uses_compact_representation() {
    let levels = vec![
        ParsedDepthLevel {
            price: 24100.10,
            qty: 12.5,
            notional: 301251.25,
        },
        ParsedDepthLevel {
            price: 24100.20,
            qty: 10.0,
            notional: 241002.0,
        },
    ];

    assert_eq!(
        format_depth_levels(&levels),
        "24100.10 x 12.5000,24100.20 x 10.0000"
    );
    assert_eq!(format_depth_levels(&[]), "-");
}

#[test]
fn format_pressure_visual_renders_balance_bar() {
    assert_eq!(format_pressure_visual(0.0, 10), "░░░░░░░░░░");
    assert_eq!(format_pressure_visual(50.0, 10), "█████░░░░░");
    assert_eq!(format_pressure_visual(100.0, 10), "██████████");
}

#[test]
fn format_notional_compact_uses_suffixes() {
    assert_eq!(format_notional_compact(980.0), "980");
    assert_eq!(format_notional_compact(1_540.0), "1.5K");
    assert_eq!(format_notional_compact(2_750_000.0), "2.75M");
}

#[test]
fn build_depth_stream_names() {
    let symbols = vec!["btcusdt".to_string(), "ETHUSDT".to_string()];
    let streams = build_depth_streams(&symbols, 20, 100);

    assert_eq!(
        streams,
        vec![
            "btcusdt@depth20@100ms".to_string(),
            "ethusdt@depth20@100ms".to_string(),
        ]
    );
}

#[test]
fn build_diff_depth_stream_names() {
    let symbols = vec!["btcusdt".to_string(), "ETHUSDT".to_string()];
    let streams = build_diff_depth_streams(&symbols, 100);

    assert_eq!(
        streams,
        vec![
            "btcusdt@depth@100ms".to_string(),
            "ethusdt@depth@100ms".to_string(),
        ]
    );
}
