use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde_json::Value;

use crate::app::GrafanaConfig;

use super::models::{
    DataSource, LokiQueryRangeResponse, LokiStream, PrometheusData, PrometheusQueryResponse,
    TempoSearchResponse,
};

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
        let body = self.get_text(&endpoint, None, "calling Grafana datasources API")?;

        let datasources: Vec<DataSource> =
            serde_json::from_str(&body).context("failed to parse datasource list JSON")?;

        Ok(datasources)
    }

    pub fn fetch_datasource_by_uid(&self, uid: &str) -> Result<DataSource> {
        let encoded_uid = encode_path_segment(uid);
        let endpoint = format!(
            "{}/api/datasources/uid/{}",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );
        let body = self.get_text(&endpoint, None, "fetching datasource by UID")?;

        serde_json::from_str(&body).context("failed to parse datasource JSON")
    }

    pub fn query_datasource_sql(
        &self,
        datasource_uid: &str,
        datasource_type: &str,
        raw_sql: &str,
    ) -> Result<Value> {
        let endpoint = format!("{}/api/ds/query", self.base_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "queries": [
                {
                    "refId": "A",
                    "datasource": {
                        "uid": datasource_uid,
                        "type": datasource_type,
                    },
                    "rawSql": raw_sql,
                    "rawQuery": true,
                    "format": "table",
                    "intervalMs": 1000,
                    "maxDataPoints": 1000,
                }
            ],
            "from": "now-1h",
            "to": "now",
        });

        let response_body = self.post_json_text(&endpoint, &body, "querying SQL datasource")?;

        serde_json::from_str(&response_body).context("failed to parse SQL query response JSON")
    }

    pub fn query_loki_range(
        &self,
        datasource_uid: &str,
        query: &str,
        start_ns: &str,
        end_ns: &str,
        limit: u32,
        direction: &str,
    ) -> Result<Vec<LokiStream>> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/loki/api/v1/query_range",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );

        let params = vec![
            ("query", query.to_string()),
            ("start", start_ns.to_string()),
            ("end", end_ns.to_string()),
            ("limit", limit.to_string()),
            ("direction", direction.to_string()),
        ];

        let body = self.get_text(&endpoint, Some(&params), "querying Loki logs")?;

        let response: LokiQueryRangeResponse =
            serde_json::from_str(&body).context("failed to parse Loki query response JSON")?;

        if response.status != "success" {
            bail!(
                "Loki query returned non-success status: {}",
                response.status
            );
        }

        Ok(response.data.streams)
    }

    pub fn query_loki_stats_range(
        &self,
        datasource_uid: &str,
        query: &str,
        start_ns: &str,
        end_ns: &str,
        step_seconds: &str,
    ) -> Result<PrometheusData> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/loki/api/v1/query_range",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );

        let params = vec![
            ("query", query.to_string()),
            ("start", start_ns.to_string()),
            ("end", end_ns.to_string()),
            ("step", step_seconds.to_string()),
        ];

        let body = self.get_text(&endpoint, Some(&params), "querying Loki stats")?;

        parse_query_response(&body, "Loki stats query")
    }

    pub fn query_prometheus_instant(
        &self,
        datasource_uid: &str,
        query: &str,
        time_seconds: &str,
    ) -> Result<PrometheusData> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/api/v1/query",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );

        let params = vec![
            ("query", query.to_string()),
            ("time", time_seconds.to_string()),
        ];

        let body = self.get_text(&endpoint, Some(&params), "querying Prometheus instant API")?;

        parse_query_response(&body, "Prometheus instant query")
    }

    pub fn query_prometheus_range(
        &self,
        datasource_uid: &str,
        query: &str,
        start_seconds: &str,
        end_seconds: &str,
        step_seconds: &str,
    ) -> Result<PrometheusData> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/api/v1/query_range",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );

        let params = vec![
            ("query", query.to_string()),
            ("start", start_seconds.to_string()),
            ("end", end_seconds.to_string()),
            ("step", step_seconds.to_string()),
        ];

        let body = self.get_text(&endpoint, Some(&params), "querying Prometheus range API")?;

        parse_query_response(&body, "Prometheus range query")
    }

    pub fn search_tempo(
        &self,
        datasource_uid: &str,
        query: &str,
        start_seconds: &str,
        end_seconds: &str,
        limit: u32,
    ) -> Result<Vec<Value>> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/api/search",
            self.base_url.trim_end_matches('/'),
            encoded_uid
        );

        let params = vec![
            ("q", query.to_string()),
            ("start", start_seconds.to_string()),
            ("end", end_seconds.to_string()),
            ("limit", limit.to_string()),
        ];

        let body = self.get_text(&endpoint, Some(&params), "searching traces in Tempo")?;

        let response: TempoSearchResponse =
            serde_json::from_str(&body).context("failed to parse Tempo search response JSON")?;

        Ok(response.traces)
    }

    pub fn fetch_trace(&self, datasource_uid: &str, trace_id: &str) -> Result<Value> {
        let encoded_uid = encode_path_segment(datasource_uid);
        let encoded_trace_id = encode_path_segment(trace_id);
        let endpoint = format!(
            "{}/api/datasources/proxy/uid/{}/api/v2/traces/{}",
            self.base_url.trim_end_matches('/'),
            encoded_uid,
            encoded_trace_id
        );

        let body = self.get_text(&endpoint, None, "fetching trace by ID")?;
        serde_json::from_str(&body).context("failed to parse trace JSON")
    }

    fn get_text(
        &self,
        endpoint: &str,
        params: Option<&[(&str, String)]>,
        action: &str,
    ) -> Result<String> {
        let mut request = self.http.get(endpoint).bearer_auth(&self.token);
        if let Some(params) = params {
            request = request.query(params);
        }

        let response = request
            .send()
            .with_context(|| format!("request to {endpoint} failed"))?;

        let status = response.status();
        let body = response.text().context("failed to read response body")?;

        ensure_grafana_success(status, &body, action)?;

        Ok(body)
    }

    fn post_json_text(&self, endpoint: &str, body: &Value, action: &str) -> Result<String> {
        let response = self
            .http
            .post(endpoint)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .with_context(|| format!("request to {endpoint} failed"))?;

        let status = response.status();
        let response_body = response.text().context("failed to read response body")?;

        ensure_grafana_success(status, &response_body, action)?;

        Ok(response_body)
    }
}

fn parse_query_response(body: &str, query_kind: &str) -> Result<PrometheusData> {
    let response: PrometheusQueryResponse = serde_json::from_str(body)
        .with_context(|| format!("failed to parse {query_kind} response JSON"))?;

    if response.status != "success" {
        bail!(
            "{query_kind} returned non-success status: {}",
            response.status
        );
    }

    Ok(response.data)
}

fn ensure_grafana_success(status: StatusCode, body: &str, action: &str) -> Result<()> {
    match status {
        s if s.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => {
            bail!("HTTP 401 Unauthorized while {action}. Check Grafana token credentials.")
        }
        StatusCode::FORBIDDEN => {
            bail!("HTTP 403 Forbidden while {action}. Token lacks required Grafana permissions.")
        }
        StatusCode::NOT_FOUND => {
            bail!("HTTP 404 Not Found while {action}. Check Grafana base URL.")
        }
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
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let truncated: String = value.chars().take(max_len).collect();
    format!("{truncated}...")
}

fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());

    for byte in segment.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        if is_unreserved {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{encode_path_segment, truncate_for_log};

    #[test]
    fn truncate_for_log_handles_utf8_without_panicking() {
        let value = "🔥error";
        let truncated = truncate_for_log(value, 1);
        assert_eq!(truncated, "🔥...");
    }

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("pg/ro"), "pg%2Fro");
        assert_eq!(encode_path_segment("trace id"), "trace%20id");
        assert_eq!(encode_path_segment("abc-_.~123"), "abc-_.~123");
    }
}
