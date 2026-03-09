use crate::news::types::NewsItem;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct NewsRecord {
    pub provider: String,
    pub article_id: String,
    pub published_at: i64,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub symbols: Vec<String>,
    pub sentiment_score: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct NewsStore {
    db_path: String,
}

impl NewsStore {
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    pub fn init(&self) -> Result<()> {
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;

        tx.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS news_items (
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
            CREATE INDEX IF NOT EXISTS idx_news_items_published_at
                ON news_items(published_at DESC);

            CREATE TABLE IF NOT EXISTS news_item_symbols (
                news_item_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                published_at INTEGER NOT NULL,
                PRIMARY KEY(news_item_id, symbol),
                FOREIGN KEY(news_item_id) REFERENCES news_items(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_news_item_symbols_symbol_news_item
                ON news_item_symbols(symbol, news_item_id);
            CREATE INDEX IF NOT EXISTS idx_news_item_symbols_symbol_published_at
                ON news_item_symbols(symbol, published_at DESC, news_item_id);
            ",
        )?;

        let mut backfill_stmt = tx.prepare("SELECT id, published_at, symbols FROM news_items")?;
        let rows = backfill_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (news_item_id, published_at, raw_symbols) = row?;
            for symbol in parse_symbols(&raw_symbols) {
                tx.execute(
                    "
                    INSERT OR IGNORE INTO news_item_symbols (news_item_id, symbol, published_at)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![news_item_id, symbol, published_at],
                )?;
            }
        }

        drop(backfill_stmt);
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_many(&self, items: &[NewsItem]) -> Result<usize> {
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;
        let mut inserted = 0usize;

        for item in items {
            let url_hash = hash_url(&item.url);
            let symbols = normalize_symbols(&item.symbols);
            let symbols_json = serde_json::to_string(&symbols)?;

            let changed = tx.execute(
                "
                INSERT OR IGNORE INTO news_items
                    (provider, article_id, published_at, title, summary, url, url_hash, symbols, sentiment_score)
                VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
                params![
                    item.source,
                    item.id,
                    item.published_at,
                    item.title,
                    item.summary,
                    item.url,
                    url_hash,
                    symbols_json,
                    item.sentiment_score,
                ],
            )?;
            inserted += changed;

            let news_item_id: i64 = tx.query_row(
                "SELECT id FROM news_items WHERE provider = ?1 AND article_id = ?2",
                params![item.source, item.id],
                |row| row.get(0),
            )?;

            for symbol in symbols {
                tx.execute(
                    "
                    INSERT INTO news_item_symbols (news_item_id, symbol, published_at)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(news_item_id, symbol)
                    DO UPDATE SET published_at = excluded.published_at
                    ",
                    params![news_item_id, symbol, item.published_at],
                )?;
            }
        }

        tx.commit()?;
        Ok(inserted)
    }

    pub fn prune_older_than(&self, min_published_at: i64) -> Result<usize> {
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;

        tx.execute(
            "
            DELETE FROM news_item_symbols
            WHERE news_item_id IN (
                SELECT id FROM news_items WHERE published_at > 0 AND published_at < ?1
            )
            ",
            params![min_published_at],
        )?;

        let deleted = tx.execute(
            "DELETE FROM news_items WHERE published_at > 0 AND published_at < ?1",
            params![min_published_at],
        )?;

        tx.commit()?;
        Ok(deleted)
    }

    pub fn get_recent_by_symbol(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
        limit: usize,
    ) -> Result<Vec<NewsRecord>> {
        let conn = Connection::open(&self.db_path)?;
        let query_limit = limit.max(1) as i64;
        let mut stmt = conn.prepare(
            "
            SELECT ni.provider, ni.article_id, ni.published_at, ni.title, ni.summary, ni.url, ni.symbols, ni.sentiment_score
            FROM news_item_symbols nis
            INNER JOIN news_items ni ON ni.id = nis.news_item_id
            WHERE nis.symbol = ?1
              AND nis.published_at >= ?2
              AND nis.published_at <= ?3
            ORDER BY nis.published_at DESC, nis.news_item_id DESC
            LIMIT ?4
            ",
        )?;

        let symbol_upper = symbol.to_ascii_uppercase();
        let rows = stmt.query_map(params![symbol_upper, from_ts, to_ts, query_limit], |row| {
            let raw_symbols: String = row.get(6)?;
            Ok(NewsRecord {
                provider: row.get(0)?,
                article_id: row.get(1)?,
                published_at: row.get(2)?,
                title: row.get(3)?,
                summary: row.get(4)?,
                url: row.get(5)?,
                symbols: parse_symbols(&raw_symbols),
                sentiment_score: row.get(7)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }
}

fn normalize_symbols(symbols: &[String]) -> Vec<String> {
    let mut deduped = HashSet::new();
    let mut normalized = Vec::new();

    for symbol in symbols {
        let trimmed = symbol.trim();
        if trimmed.is_empty() {
            continue;
        }

        let upper = trimmed.to_ascii_uppercase();
        if deduped.insert(upper.clone()) {
            normalized.push(upper);
        }
    }

    normalized
}

fn parse_symbols(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw)
        .map(|items| normalize_symbols(&items))
        .map(|items| {
            items
                .into_iter()
                .filter(|item| !item.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn hash_url(url: &str) -> String {
    let canonical = url.trim().to_lowercase();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
