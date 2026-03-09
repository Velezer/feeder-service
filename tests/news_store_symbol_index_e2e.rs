use feeder_service::news::store::NewsStore;
use feeder_service::news::types::NewsItem;
use rusqlite::{Connection, params};

fn test_db_path(name: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("valid time")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("feeder-service-{name}-{nanos}.sqlite"))
        .to_string_lossy()
        .to_string()
}

#[test]
fn backfills_symbol_table_and_prunes_dependent_rows() {
    let db_path = test_db_path("news-store-symbol-index");

    let conn = Connection::open(&db_path).expect("open sqlite");
    conn.execute_batch(
        "
        CREATE TABLE news_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider TEXT NOT NULL,
            article_id TEXT NOT NULL,
            published_at INTEGER NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            url TEXT NOT NULL,
            url_hash TEXT NOT NULL,
            symbols TEXT NOT NULL,
            sentiment_score REAL,
            inserted_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            UNIQUE(provider, article_id),
            UNIQUE(url_hash)
        );
        ",
    )
    .expect("create legacy schema");

    conn.execute(
        "
        INSERT INTO news_items
            (provider, article_id, published_at, title, summary, url, url_hash, symbols, sentiment_score)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            "legacy",
            "article-legacy",
            1_710_000_000_000_i64,
            "Legacy BTC article",
            "Legacy summary",
            "https://example.com/legacy",
            "legacy-hash",
            "[\"btcusdt\",\" BTCUSDT \",\"\",\"ethusdt\"]",
            0.4_f64,
        ],
    )
    .expect("insert legacy item");

    drop(conn);

    let store = NewsStore::new(db_path.clone());
    store.init().expect("run migration and backfill");

    let conn = Connection::open(&db_path).expect("open sqlite post-init");

    let index_count: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'index'
              AND name IN (
                'idx_news_item_symbols_symbol_news_item',
                'idx_news_item_symbols_symbol_published_at'
              )
            ",
            [],
            |row| row.get(0),
        )
        .expect("count indexes");
    assert_eq!(index_count, 2);

    let backfilled_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM news_item_symbols WHERE symbol = 'BTCUSDT'",
            [],
            |row| row.get(0),
        )
        .expect("count backfilled symbol rows");
    assert_eq!(backfilled_count, 1, "symbol rows should be normalized and deduplicated");

    let recent = store
        .get_recent_by_symbol("BTCUSDT", 1_709_999_999_000, 1_710_000_001_000, 10)
        .expect("query by symbol");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].article_id, "article-legacy");

    store
        .upsert_many(&[NewsItem {
            id: "article-new".to_string(),
            source: "new-provider".to_string(),
            published_at: 1_710_000_000_200,
            title: "New ETH/BTC article".to_string(),
            summary: "Something happened".to_string(),
            url: "https://example.com/new".to_string(),
            symbols: vec!["btcusdt".to_string(), "ethusdt".to_string()],
            sentiment_score: Some(0.9),
        }])
        .expect("upsert new item");

    let recent_after_upsert = store
        .get_recent_by_symbol("BTCUSDT", 1_709_999_999_000, 1_710_000_001_000, 10)
        .expect("query upserted items");
    assert_eq!(recent_after_upsert.len(), 2);
    assert_eq!(recent_after_upsert[0].article_id, "article-new");

    let pruned = store
        .prune_older_than(1_710_000_000_100)
        .expect("prune old rows");
    assert_eq!(pruned, 1);

    let legacy_symbol_rows: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM news_item_symbols nis
            JOIN news_items ni ON ni.id = nis.news_item_id
            WHERE ni.article_id = 'article-legacy'
            ",
            [],
            |row| row.get(0),
        )
        .expect("count symbol rows for pruned item");
    assert_eq!(legacy_symbol_rows, 0);

    let _ = std::fs::remove_file(db_path);
}
