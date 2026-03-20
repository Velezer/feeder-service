use crate::news::types::NewsItem;
use anyhow::{Context, Result};
use feed_rs::parser;
use reqwest::Client;
use serde::Deserialize;

const DEFAULT_RSS_FEEDS: [&str; 3] = [
    "https://www.coindesk.com/arc/outboundfeeds/rss/",
    "https://cointelegraph.com/rss",
    "https://decrypt.co/feed",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    Disabled,
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchDiagnostics {
    pub finnhub: ProviderStatus,
    pub newsapi: ProviderStatus,
    pub rss: ProviderStatus,
}

impl FetchDiagnostics {
    pub fn all_providers_disabled(&self) -> bool {
        self.finnhub == ProviderStatus::Disabled
            && self.newsapi == ProviderStatus::Disabled
            && self.rss == ProviderStatus::Disabled
    }

    pub fn failure_summary(&self) -> Option<&'static str> {
        let enabled = [self.finnhub, self.newsapi, self.rss]
            .into_iter()
            .filter(|status| *status != ProviderStatus::Disabled)
            .count();
        let failed = [self.finnhub, self.newsapi, self.rss]
            .into_iter()
            .filter(|status| *status == ProviderStatus::Failed)
            .count();
        let succeeded = [self.finnhub, self.newsapi, self.rss]
            .into_iter()
            .filter(|status| *status == ProviderStatus::Success)
            .count();

        match (enabled, failed, succeeded) {
            (0, _, _) => None,
            (_, failed_count, 0) if failed_count > 0 => Some("all enabled providers failed"),
            (1, 1, 0) => Some("the configured provider failed"),
            (_, failed_count, success_count) if failed_count > 0 && success_count > 0 => {
                Some("one provider failed")
            }
            _ => None,
        }
    }

    pub fn provider_state_summary(&self) -> String {
        format!(
            "finnhub={};newsapi={};rss={}",
            self.finnhub.as_label(),
            self.newsapi.as_label(),
            self.rss.as_label()
        )
    }

    pub fn fetch_reason(&self, fetched_count: usize) -> &'static str {
        if self.all_providers_disabled() {
            "no_provider_api_key"
        } else if let Some(summary) = self.failure_summary() {
            match summary {
                "all enabled providers failed" => "providers_failed",
                "the configured provider failed" => "provider_failed",
                "one provider failed" => "partial_provider_failure",
                _ => "provider_diagnostic",
            }
        } else if fetched_count == 0 {
            "providers_returned_no_articles"
        } else {
            "ok"
        }
    }
}

impl ProviderStatus {
    fn as_label(self) -> &'static str {
        match self {
            ProviderStatus::Disabled => "disabled",
            ProviderStatus::Success => "ok",
            ProviderStatus::Failed => "failed",
        }
    }
}

pub async fn fetch_all_news(
    client: &Client,
    config: &crate::config::NewsConfig,
) -> Result<(Vec<NewsItem>, FetchDiagnostics)> {
    let mut items = Vec::new();
    let mut diagnostics = FetchDiagnostics {
        finnhub: ProviderStatus::Disabled,
        newsapi: ProviderStatus::Disabled,
        rss: ProviderStatus::Success,
    };

    if let Some(api_key) = config.finnhub_api_key.as_deref() {
        diagnostics.finnhub = ProviderStatus::Success;
        match fetch_finnhub_news(client, api_key).await {
            Ok(mut provider_items) => items.append(&mut provider_items),
            Err(err) => {
                diagnostics.finnhub = ProviderStatus::Failed;
                eprintln!("[news] finnhub fetch failed: {err}");
            }
        }
    }

    if let Some(api_key) = config.newsapi_api_key.as_deref() {
        diagnostics.newsapi = ProviderStatus::Success;
        match fetch_newsapi_news(client, api_key).await {
            Ok(mut provider_items) => items.append(&mut provider_items),
            Err(err) => {
                diagnostics.newsapi = ProviderStatus::Failed;
                eprintln!("[news] newsapi fetch failed: {err}");
            }
        }
    }

    match fetch_rss_news(client).await {
        Ok(mut provider_items) => items.append(&mut provider_items),
        Err(err) => {
            diagnostics.rss = ProviderStatus::Failed;
            eprintln!("[news] rss fetch failed: {err}");
        }
    }

    Ok((items, diagnostics))
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

pub async fn fetch_rss_news(client: &Client) -> Result<Vec<NewsItem>> {
    let mut items = Vec::new();

    for feed_url in DEFAULT_RSS_FEEDS {
        let bytes = client
            .get(feed_url)
            .send()
            .await
            .with_context(|| format!("request feed {feed_url}"))?
            .error_for_status()
            .with_context(|| format!("http status for feed {feed_url}"))?
            .bytes()
            .await
            .with_context(|| format!("read feed body {feed_url}"))?;

        let feed = parser::parse(&bytes[..]).with_context(|| format!("parse feed {feed_url}"))?;
        let fallback_source = feed
            .title
            .as_ref()
            .map(|title| title.content.clone())
            .unwrap_or_else(|| "rss".to_string());

        for (index, entry) in feed.entries.into_iter().enumerate() {
            let published_at = entry
                .published
                .or(entry.updated)
                .map(|ts| ts.timestamp())
                .unwrap_or(0);
            let title = entry
                .title
                .as_ref()
                .map(|value| value.content.clone())
                .unwrap_or_default();
            let summary = entry
                .summary
                .as_ref()
                .map(|value| value.content.clone())
                .unwrap_or_default();
            let url = entry
                .links
                .iter()
                .find(|link| link.rel.as_deref() == Some("alternate"))
                .or_else(|| entry.links.first())
                .map(|link| link.href.clone())
                .unwrap_or_default();
            let provider = fallback_source.clone();

            if title.trim().is_empty() || url.trim().is_empty() {
                continue;
            }

            items.push(NewsItem {
                id: format!("rss-{provider}-{index}-{url}"),
                source: provider,
                published_at,
                title,
                summary,
                url,
                symbols: Vec::new(),
                sentiment_score: None,
            });
        }
    }

    Ok(items)
}

fn parse_iso8601_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.timestamp())
}

#[cfg(test)]
mod tests {
    use super::{FetchDiagnostics, ProviderStatus};
    use crate::config::NewsConfig;

    #[test]
    fn diagnostics_identify_disabled_api_providers_when_rss_is_available() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Disabled,
            newsapi: ProviderStatus::Disabled,
            rss: ProviderStatus::Success,
        };

        assert!(!diagnostics.all_providers_disabled());
        assert_eq!(diagnostics.failure_summary(), None);
        assert_eq!(
            diagnostics.provider_state_summary(),
            "finnhub=disabled;newsapi=disabled;rss=ok"
        );
        assert_eq!(diagnostics.fetch_reason(3), "ok");
    }

    #[test]
    fn diagnostics_identify_single_provider_failure() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Failed,
            newsapi: ProviderStatus::Disabled,
            rss: ProviderStatus::Disabled,
        };

        assert!(!diagnostics.all_providers_disabled());
        assert_eq!(
            diagnostics.failure_summary(),
            Some("all enabled providers failed")
        );
        assert_eq!(
            diagnostics.provider_state_summary(),
            "finnhub=failed;newsapi=disabled;rss=disabled"
        );
        assert_eq!(diagnostics.fetch_reason(0), "providers_failed");
    }

    #[test]
    fn diagnostics_identify_no_articles_case() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Success,
            newsapi: ProviderStatus::Disabled,
            rss: ProviderStatus::Success,
        };

        assert_eq!(
            diagnostics.fetch_reason(0),
            "providers_returned_no_articles"
        );
    }

    #[test]
    fn diagnostics_identify_partial_failures() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Disabled,
            newsapi: ProviderStatus::Failed,
            rss: ProviderStatus::Success,
        };

        assert_eq!(diagnostics.failure_summary(), Some("one provider failed"));
        assert_eq!(
            diagnostics.provider_state_summary(),
            "finnhub=disabled;newsapi=failed;rss=ok"
        );
        assert_eq!(diagnostics.fetch_reason(5), "partial_provider_failure");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_all_news_uses_rss_without_api_keys() {
        let client = reqwest::Client::builder()
            .user_agent("feeder-service-news-test/0.1")
            .build()
            .expect("build client");
        let config = NewsConfig {
            enabled: true,
            db_path: "news.sqlite".to_string(),
            poll_interval_secs: 300,
            retention_hours: 24,
            finnhub_api_key: None,
            newsapi_api_key: None,
        };

        let (items, diagnostics) = super::fetch_all_news(&client, &config)
            .await
            .expect("fetch rss news without api keys");

        assert!(
            !items.is_empty(),
            "rss fallback should return live articles"
        );
        assert_eq!(diagnostics.rss, ProviderStatus::Success);
        assert_eq!(diagnostics.fetch_reason(items.len()), "ok");
    }
}
