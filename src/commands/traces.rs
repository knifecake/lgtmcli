use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::app::AppContext;
use crate::cli::{TraceGetArgs, TracesSearchArgs};
use crate::output::{TableOutput, render_aligned_table};
use crate::time::{resolve_range, seconds_to_rfc3339, to_unix_seconds_string};

const DEFAULT_SINCE: &str = "1h";

#[derive(Debug, Serialize)]
pub struct TracesSearchResult {
    pub datasource_uid: String,
    pub query: String,
    pub start: String,
    pub end: String,
    pub limit: u32,
    pub count: usize,
    pub traces: Vec<TraceSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceSummary {
    pub trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time_ns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_count: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct TraceGetResult {
    pub datasource_uid: String,
    pub trace_id: String,
    pub detected_span_count: usize,
    pub trace: Value,
}

pub fn search(ctx: &AppContext, args: TracesSearchArgs) -> Result<TracesSearchResult> {
    let range = resolve_range(
        args.since.as_deref(),
        args.from.as_deref(),
        args.to.as_deref(),
        DEFAULT_SINCE,
    )?;

    let start_seconds = to_unix_seconds_string(range.start)?;
    let end_seconds = to_unix_seconds_string(range.end)?;

    let traces = ctx.grafana.search_tempo(
        &args.datasource_uid,
        &args.query,
        &start_seconds,
        &end_seconds,
        args.limit,
    )?;

    let summaries: Vec<TraceSummary> = traces.into_iter().map(|t| summarize_trace(&t)).collect();

    Ok(TracesSearchResult {
        datasource_uid: args.datasource_uid,
        query: args.query,
        start: seconds_to_rfc3339(&start_seconds),
        end: seconds_to_rfc3339(&end_seconds),
        limit: args.limit,
        count: summaries.len(),
        traces: summaries,
    })
}

pub fn get(ctx: &AppContext, args: TraceGetArgs) -> Result<TraceGetResult> {
    let trace = ctx
        .grafana
        .fetch_trace(&args.datasource_uid, &args.trace_id)?;

    Ok(TraceGetResult {
        datasource_uid: args.datasource_uid,
        trace_id: args.trace_id,
        detected_span_count: count_spans_in_value(&trace),
        trace,
    })
}

impl TableOutput for TracesSearchResult {
    fn render_table(&self) {
        if self.traces.is_empty() {
            println!("No traces found.");
            return;
        }

        let headers = [
            "TRACE_ID",
            "SERVICE",
            "NAME",
            "START",
            "DURATION_MS",
            "SPANS",
        ];
        let rows: Vec<Vec<String>> = self
            .traces
            .iter()
            .map(|trace| {
                vec![
                    trace.trace_id.clone(),
                    trace
                        .root_service
                        .clone()
                        .unwrap_or_else(|| "-".to_string()),
                    trace.root_name.clone().unwrap_or_else(|| "-".to_string()),
                    trace.start_time.clone().unwrap_or_else(|| "-".to_string()),
                    trace
                        .duration_ms
                        .map(|d| format!("{d:.3}"))
                        .unwrap_or_else(|| "-".to_string()),
                    trace
                        .span_count
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ]
            })
            .collect();

        render_aligned_table(&headers, &rows);
    }
}

impl TableOutput for TraceGetResult {
    fn render_table(&self) {
        println!("Trace ID: {}", self.trace_id);
        println!("Datasource: {}", self.datasource_uid);
        println!("Detected spans: {}", self.detected_span_count);
        println!("Use --json to print full trace payload.");
    }
}

fn summarize_trace(trace: &Value) -> TraceSummary {
    let trace_id = extract_string_field(trace, &["traceID", "traceId", "trace_id"])
        .unwrap_or_else(|| "<unknown-trace-id>".to_string());

    let root_service = extract_string_field(
        trace,
        &["rootServiceName", "serviceName", "root_service_name"],
    );
    let root_name = extract_string_field(trace, &["rootTraceName", "traceName", "name"]);

    let start_time_ns = extract_string_field(trace, &["startTimeUnixNano", "startTimeNano"]);
    let start_time = start_time_ns.as_deref().map(crate::time::ns_to_rfc3339);

    let duration_ms = extract_f64_field(trace, &["durationMs", "duration_ms"]);

    let span_count = extract_span_count(trace);

    TraceSummary {
        trace_id,
        root_service,
        root_name,
        start_time_ns,
        start_time,
        duration_ms,
        span_count,
    }
}

fn extract_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(found) = value.get(*key) {
            match found {
                Value::String(v) => return Some(v.clone()),
                Value::Number(v) => return Some(v.to_string()),
                _ => {}
            }
        }
    }
    None
}

fn extract_f64_field(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(found) = value.get(*key) {
            if let Some(num) = found.as_f64() {
                return Some(num);
            }
            if let Some(raw) = found.as_str()
                && let Ok(parsed) = raw.parse::<f64>()
            {
                return Some(parsed);
            }
        }
    }
    None
}

fn extract_span_count(value: &Value) -> Option<usize> {
    // Tempo search response commonly has spanSets: [{ spans: [...] }]
    let mut total = 0usize;
    let mut found_any = false;

    if let Some(span_sets) = value.get("spanSets").and_then(Value::as_array) {
        for set in span_sets {
            if let Some(spans) = set.get("spans").and_then(Value::as_array) {
                total += spans.len();
                found_any = true;
            }
        }
    }

    if found_any { Some(total) } else { None }
}

fn count_spans_in_value(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut total = 0usize;

            for (key, val) in map {
                if key == "spans"
                    && let Some(arr) = val.as_array()
                {
                    total += arr.len();
                    continue;
                }
                total += count_spans_in_value(val);
            }

            total
        }
        Value::Array(values) => values.iter().map(count_spans_in_value).sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{count_spans_in_value, summarize_trace};

    #[test]
    fn summarize_trace_extracts_common_fields() {
        let input = json!({
            "traceID": "abc123",
            "rootServiceName": "checkout",
            "rootTraceName": "POST /pay",
            "startTimeUnixNano": "1710000000000000000",
            "durationMs": 123.4,
            "spanSets": [
                {"spans": [{"spanID": "1"}, {"spanID": "2"}]}
            ]
        });

        let summary = summarize_trace(&input);
        assert_eq!(summary.trace_id, "abc123");
        assert_eq!(summary.root_service.as_deref(), Some("checkout"));
        assert_eq!(summary.root_name.as_deref(), Some("POST /pay"));
        assert_eq!(summary.duration_ms, Some(123.4));
        assert_eq!(summary.span_count, Some(2));
    }

    #[test]
    fn span_counter_counts_nested_spans_arrays() {
        let input = json!({
            "resourceSpans": [
                {
                    "scopeSpans": [
                        {"spans": [{"id": 1}, {"id": 2}]},
                        {"spans": [{"id": 3}]}
                    ]
                }
            ]
        });

        assert_eq!(count_spans_in_value(&input), 3);
    }
}
