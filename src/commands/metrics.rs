use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::app::AppContext;
use crate::cli::{MetricsQueryArgs, MetricsRangeArgs};
use crate::grafana::models::PrometheusData;
use crate::output::{TableOutput, render_aligned_table};
use crate::time::{
    parse_rfc3339, parse_since_duration, resolve_range, seconds_to_rfc3339, to_unix_seconds_string,
};

const DEFAULT_SINCE: &str = "1h";

#[derive(Debug, Serialize)]
pub struct MetricsQueryResult {
    pub mode: String,
    pub datasource_uid: String,
    pub query: String,
    pub result_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_seconds: Option<u64>,
    pub count: usize,
    pub samples: Vec<MetricSample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSample {
    pub timestamp_unix: String,
    pub timestamp: String,
    pub labels: BTreeMap<String, String>,
    pub value: String,
}

pub fn query(ctx: &AppContext, args: MetricsQueryArgs) -> Result<MetricsQueryResult> {
    let query_time = match args.time {
        Some(raw) => {
            parse_rfc3339(&raw).with_context(|| format!("invalid --time value '{raw}'"))?
        }
        None => SystemTime::now(),
    };

    let query_time_seconds = to_unix_seconds_string(query_time)?;

    let data = ctx.grafana.query_prometheus_instant(
        &args.datasource_uid,
        &args.query,
        &query_time_seconds,
    )?;

    let samples = extract_samples(&data)?;

    Ok(MetricsQueryResult {
        mode: "instant".to_string(),
        datasource_uid: args.datasource_uid,
        query: args.query,
        result_type: data.result_type,
        time: Some(seconds_to_rfc3339(&query_time_seconds)),
        start: None,
        end: None,
        step_seconds: None,
        count: samples.len(),
        samples,
    })
}

pub fn range(ctx: &AppContext, args: MetricsRangeArgs) -> Result<MetricsQueryResult> {
    let range = resolve_range(
        args.since.as_deref(),
        args.from.as_deref(),
        args.to.as_deref(),
        DEFAULT_SINCE,
    )?;

    let step_duration = parse_since_duration(&args.step)
        .with_context(|| format!("invalid --step value '{}'", args.step))?;
    let step_seconds = step_duration.as_secs();
    if step_seconds == 0 {
        bail!("--step must be at least 1s");
    }

    let start_seconds = to_unix_seconds_string(range.start)?;
    let end_seconds = to_unix_seconds_string(range.end)?;

    let data = ctx.grafana.query_prometheus_range(
        &args.datasource_uid,
        &args.query,
        &start_seconds,
        &end_seconds,
        &step_seconds.to_string(),
    )?;

    let samples = extract_samples(&data)?;

    Ok(MetricsQueryResult {
        mode: "range".to_string(),
        datasource_uid: args.datasource_uid,
        query: args.query,
        result_type: data.result_type,
        time: None,
        start: Some(seconds_to_rfc3339(&start_seconds)),
        end: Some(seconds_to_rfc3339(&end_seconds)),
        step_seconds: Some(step_seconds),
        count: samples.len(),
        samples,
    })
}

impl TableOutput for MetricsQueryResult {
    fn render_table(&self) {
        if self.samples.is_empty() {
            println!("No metric samples found.");
            return;
        }

        let headers = ["TIMESTAMP", "LABELS", "VALUE"];
        let rows: Vec<Vec<String>> = self
            .samples
            .iter()
            .map(|sample| {
                vec![
                    sample.timestamp.clone(),
                    format_labels(&sample.labels),
                    sample.value.clone(),
                ]
            })
            .collect();

        render_aligned_table(&headers, &rows);
    }
}

fn extract_samples(data: &PrometheusData) -> Result<Vec<MetricSample>> {
    match data.result_type.as_str() {
        "vector" => extract_vector_samples(&data.result),
        "matrix" => extract_matrix_samples(&data.result),
        "scalar" | "string" => extract_scalar_sample(&data.result),
        other => bail!("unsupported Prometheus resultType '{other}'"),
    }
}

fn extract_vector_samples(result: &Value) -> Result<Vec<MetricSample>> {
    let items = result
        .as_array()
        .context("invalid Prometheus vector result: expected array")?;

    let mut samples = Vec::with_capacity(items.len());
    for item in items {
        let labels = extract_labels(item.get("metric"));
        let value = item
            .get("value")
            .and_then(Value::as_array)
            .context("invalid vector sample: expected value array")?;

        if value.len() != 2 {
            bail!("invalid vector sample: value should contain [timestamp, value]");
        }

        let ts = value_to_string(&value[0]);
        let val = value_to_string(&value[1]);
        samples.push(make_sample(ts, labels, val));
    }

    samples.sort_by(compare_samples);
    Ok(samples)
}

fn extract_matrix_samples(result: &Value) -> Result<Vec<MetricSample>> {
    let series = result
        .as_array()
        .context("invalid Prometheus matrix result: expected array")?;

    let mut samples = Vec::new();
    for item in series {
        let labels = extract_labels(item.get("metric"));
        let values = item
            .get("values")
            .and_then(Value::as_array)
            .context("invalid matrix sample: expected values array")?;

        for point in values {
            let pair = point
                .as_array()
                .context("invalid matrix point: expected [timestamp, value]")?;
            if pair.len() != 2 {
                bail!("invalid matrix point: expected exactly two values");
            }

            let ts = value_to_string(&pair[0]);
            let val = value_to_string(&pair[1]);
            samples.push(make_sample(ts, labels.clone(), val));
        }
    }

    samples.sort_by(compare_samples);
    Ok(samples)
}

fn extract_scalar_sample(result: &Value) -> Result<Vec<MetricSample>> {
    let pair = result
        .as_array()
        .context("invalid scalar/string result: expected [timestamp, value]")?;

    if pair.len() != 2 {
        bail!("invalid scalar/string result: expected exactly two values");
    }

    let ts = value_to_string(&pair[0]);
    let val = value_to_string(&pair[1]);

    Ok(vec![make_sample(ts, BTreeMap::new(), val)])
}

fn make_sample(
    timestamp_unix: String,
    labels: BTreeMap<String, String>,
    value: String,
) -> MetricSample {
    MetricSample {
        timestamp: seconds_to_rfc3339(&timestamp_unix),
        timestamp_unix,
        labels,
        value,
    }
}

fn extract_labels(raw: Option<&Value>) -> BTreeMap<String, String> {
    let Some(raw_labels) = raw.and_then(Value::as_object) else {
        return BTreeMap::new();
    };

    raw_labels
        .iter()
        .map(|(k, v)| (k.clone(), value_to_string(v)))
        .collect()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(v) => v.clone(),
        Value::Number(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn compare_samples(left: &MetricSample, right: &MetricSample) -> Ordering {
    let ts_order = compare_numeric_string(&left.timestamp_unix, &right.timestamp_unix);
    if ts_order != Ordering::Equal {
        return ts_order;
    }

    format_labels(&left.labels).cmp(&format_labels(&right.labels))
}

fn compare_numeric_string(left: &str, right: &str) -> Ordering {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
        _ => left.cmp(right),
    }
}

fn format_labels(labels: &BTreeMap<String, String>) -> String {
    if labels.is_empty() {
        return "{}".to_string();
    }

    let rendered = labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join(",");

    format!("{{{rendered}}}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{PrometheusData, extract_samples};

    #[test]
    fn extracts_vector_samples() {
        let data = PrometheusData {
            result_type: "vector".to_string(),
            result: json!([
                {
                    "metric": {"job": "api"},
                    "value": [1710000000, "1"]
                }
            ]),
        };

        let samples = extract_samples(&data).expect("extract vector");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].value, "1");
        assert_eq!(samples[0].labels.get("job").unwrap(), "api");
    }

    #[test]
    fn extracts_matrix_samples() {
        let data = PrometheusData {
            result_type: "matrix".to_string(),
            result: json!([
                {
                    "metric": {"job": "api"},
                    "values": [
                        [1710000000, "1"],
                        [1710000060, "2"]
                    ]
                }
            ]),
        };

        let samples = extract_samples(&data).expect("extract matrix");
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].value, "1");
        assert_eq!(samples[1].value, "2");
    }
}
