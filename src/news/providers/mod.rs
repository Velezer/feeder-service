use crate::news::types::NewsItem;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

pub async fn fetch_all_news(
    client: &Client,
    config: &crate::config::NewsConfig,
) -> Result<Vec<NewsItem>> {
    let mut items = Vec::new();

    if let Some(api_key) = config.finnhub_api_key.as_deref() {
        match fetch_finnhub_news(client, api_key).await {
            Ok(mut provider_items) => items.append(&mut provider_items),
            Err(err) => eprintln!("[news] finnhub fetch failed: {err}"),
        }
    }

    if let Some(api_key) = config.newsapi_api_key.as_deref() {
        match fetch_newsapi_news(client, api_key).await {
            Ok(mut provider_items) => items.append(&mut provider_items),
            Err(err) => eprintln!("[news] newsapi fetch failed: {err}"),
        }
    }

    Ok(items)
}

#[derive(Debug, Deserialize)]
struct FinnhubArticle {
    id: i64,
    datetime: i64,
    headline: String,
    summary: String,
    url: String,
    source: String,
}

async fn fetch_finnhub_news(client: &Client, api_key: &str) -> Result<Vec<NewsItem>> {
    let url = "https://finnhub.io/api/v1/news?category=crypto";
    let payload = client
        .get(url)
        .query(&[("token", api_key)])
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<FinnhubArticle>>()
        .await?;

    let items = payload
        .into_iter()
        .map(|article| NewsItem {
            id: article.id.to_string(),
            source: article.source,
            published_at: article.datetime,
            title: article.headline,
            summary: article.summary,
            url: article.url,
            symbols: Vec::new(),
            sentiment_score: None,
        })
        .collect();

    Ok(items)
}

#[derive(Debug, Deserialize)]
struct NewsApiEnvelope {
    articles: Vec<NewsApiArticle>,
}

#[derive(Debug, Deserialize)]
struct NewsApiArticle {
    title: Option<String>,
    description: Option<String>,
    url: String,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    source: NewsApiSource,
}

#[derive(Debug, Deserialize)]
struct NewsApiSource {
    name: Option<String>,
}

async fn fetch_newsapi_news(client: &Client, api_key: &str) -> Result<Vec<NewsItem>> {
    let url = "https://newsapi.org/v2/everything";
    let payload = client
        .get(url)
        .query(&[
            ("q", "crypto OR bitcoin OR ethereum"),
            ("language", "en"),
            ("sortBy", "publishedAt"),
            ("pageSize", "50"),
            ("apiKey", api_key),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<NewsApiEnvelope>()
        .await?;

    let items = payload
        .articles
        .into_iter()
        .enumerate()
        .map(|(index, article)| {
            let timestamp = article
                .published_at
                .as_deref()
                .and_then(parse_iso8601_timestamp)
                .unwrap_or(0);

            NewsItem {
                id: format!("newsapi-{index}-{}", article.url),
                source: article.source.name.unwrap_or_else(|| "newsapi".to_string()),
                published_at: timestamp,
                title: article.title.unwrap_or_default(),
                summary: article.description.unwrap_or_default(),
                url: article.url,
                symbols: Vec::new(),
                sentiment_score: None,
            }
        })
        .collect();

    Ok(items)
}

fn parse_iso8601_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.timestamp())
}
