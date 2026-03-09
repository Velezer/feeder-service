use crate::news::types::NewsItem;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
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
            ",
        )?;
        Ok(())
    }

    pub fn upsert_many(&self, items: &[NewsItem]) -> Result<usize> {
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;
        let mut inserted = 0usize;

        for item in items {
            let url_hash = hash_url(&item.url);
            let symbols_json = serde_json::to_string(&item.symbols)?;

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
        }

        tx.commit()?;
        Ok(inserted)
    }

    pub fn prune_older_than(&self, min_published_at: i64) -> Result<usize> {
        let conn = Connection::open(&self.db_path)?;
        let deleted = conn.execute(
            "DELETE FROM news_items WHERE published_at > 0 AND published_at < ?1",
            params![min_published_at],
        )?;
        Ok(deleted)
    }
}

fn hash_url(url: &str) -> String {
    let canonical = url.trim().to_lowercase();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
