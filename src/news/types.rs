use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub source: String,
    pub published_at: i64,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub symbols: Vec<String>,
    pub sentiment_score: Option<f64>,
}
