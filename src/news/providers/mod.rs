use crate::news::types::NewsItem;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

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
}

impl FetchDiagnostics {
    pub fn all_providers_disabled(&self) -> bool {
        self.finnhub == ProviderStatus::Disabled && self.newsapi == ProviderStatus::Disabled
    }

    pub fn failure_summary(&self) -> Option<&'static str> {
        match (self.finnhub, self.newsapi) {
            (ProviderStatus::Failed, ProviderStatus::Failed) => {
                Some("all enabled providers failed")
            }
            (ProviderStatus::Failed, ProviderStatus::Disabled)
            | (ProviderStatus::Disabled, ProviderStatus::Failed) => {
                Some("the configured provider failed")
            }
            (ProviderStatus::Failed, ProviderStatus::Success)
            | (ProviderStatus::Success, ProviderStatus::Failed) => Some("one provider failed"),
            _ => None,
        }
    }

    pub fn provider_state_summary(&self) -> String {
        format!(
            "finnhub={};newsapi={}",
            self.finnhub.as_label(),
            self.newsapi.as_label()
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

fn parse_iso8601_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.timestamp())
}

#[cfg(test)]
mod tests {
    use super::{FetchDiagnostics, ProviderStatus};

    #[test]
    fn diagnostics_identify_disabled_providers() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Disabled,
            newsapi: ProviderStatus::Disabled,
        };

        assert!(diagnostics.all_providers_disabled());
        assert_eq!(diagnostics.failure_summary(), None);
        assert_eq!(
            diagnostics.provider_state_summary(),
            "finnhub=disabled;newsapi=disabled"
        );
        assert_eq!(diagnostics.fetch_reason(0), "no_provider_api_key");
    }

    #[test]
    fn diagnostics_identify_single_provider_failure() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Failed,
            newsapi: ProviderStatus::Disabled,
        };

        assert!(!diagnostics.all_providers_disabled());
        assert_eq!(
            diagnostics.failure_summary(),
            Some("the configured provider failed")
        );
        assert_eq!(
            diagnostics.provider_state_summary(),
            "finnhub=failed;newsapi=disabled"
        );
        assert_eq!(diagnostics.fetch_reason(0), "provider_failed");
    }

    #[test]
    fn diagnostics_identify_no_articles_case() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Success,
            newsapi: ProviderStatus::Disabled,
        };

        assert_eq!(diagnostics.fetch_reason(0), "providers_returned_no_articles");
        assert_eq!(diagnostics.fetch_reason(2), "ok");
    }

    #[test]
    fn diagnostics_identify_partial_failures() {
        let diagnostics = FetchDiagnostics {
            finnhub: ProviderStatus::Success,
            newsapi: ProviderStatus::Failed,
        };

        assert_eq!(diagnostics.failure_summary(), Some("one provider failed"));
        assert_eq!(diagnostics.provider_state_summary(), "finnhub=ok;newsapi=failed");
        assert_eq!(diagnostics.fetch_reason(0), "partial_provider_failure");
    }
}
