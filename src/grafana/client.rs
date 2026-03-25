use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use reqwest::blocking::Client;

use crate::app::GrafanaConfig;

use super::models::DataSource;

const REQUEST_TIMEOUT_SECS: u64 = 15;

pub struct GrafanaClient {
    http: Client,
    base_url: String,
    token: String,
}

impl GrafanaClient {
    pub fn new(config: GrafanaConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url: config.base_url,
            token: config.token,
        })
    }

    pub fn fetch_datasources(&self) -> Result<Vec<DataSource>> {
        let endpoint = format!("{}/api/datasources", self.base_url.trim_end_matches('/'));

        let response = self
            .http
            .get(&endpoint)
            .bearer_auth(&self.token)
            .send()
            .with_context(|| format!("request to {endpoint} failed"))?;

        let status = response.status();
        let body = response.text().context("failed to read response body")?;

        ensure_grafana_success(status, &body, "calling Grafana datasources API")?;

        let datasources: Vec<DataSource> =
            serde_json::from_str(&body).context("failed to parse datasource list JSON")?;

        Ok(datasources)
    }
}

fn ensure_grafana_success(status: StatusCode, body: &str, action: &str) -> Result<()> {
    match status {
        s if s.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => {
            bail!("HTTP 401 Unauthorized while {action}. Check GRAFANA_TOKEN.")
        }
        StatusCode::FORBIDDEN => {
            bail!("HTTP 403 Forbidden while {action}. Token lacks required Grafana permissions.")
        }
        StatusCode::NOT_FOUND => bail!("HTTP 404 Not Found while {action}. Check GRAFANA_URL."),
        _ => {
            let snippet = truncate_for_log(body, 400);
            bail!(
                "HTTP {} while {action}. Response: {}",
                status.as_u16(),
                snippet
            )
        }
    }
}

fn truncate_for_log(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    format!("{}...", &value[..max_len])
}
