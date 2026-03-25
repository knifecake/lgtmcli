use std::cmp::Ordering;
use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;

use crate::app::AppContext;
use crate::cli::{LogDirectionArg, LogsQueryArgs};
use crate::grafana::models::LokiStream;
use crate::output::{TableOutput, render_aligned_table};
use crate::time::{ns_to_rfc3339, resolve_range, to_unix_ns_string};

const DEFAULT_SINCE: &str = "1h";

#[derive(Debug, Serialize)]
pub struct LogsQueryResult {
    pub datasource_uid: String,
    pub query: String,
    pub start_ns: String,
    pub end_ns: String,
    pub direction: String,
    pub limit: u32,
    pub count: usize,
    pub lines: Vec<LogLine>,
}

#[derive(Debug, Serialize)]
pub struct LogLine {
    pub timestamp_ns: String,
    pub timestamp: String,
    pub labels: BTreeMap<String, String>,
    pub line: String,
}

pub fn query(ctx: &AppContext, args: LogsQueryArgs) -> Result<LogsQueryResult> {
    let range = resolve_range(
        args.since.as_deref(),
        args.from.as_deref(),
        args.to.as_deref(),
        DEFAULT_SINCE,
    )?;

    let start_ns = to_unix_ns_string(range.start)?;
    let end_ns = to_unix_ns_string(range.end)?;

    let streams = ctx.grafana.query_loki_range(
        &args.datasource_uid,
        &args.query,
        &start_ns,
        &end_ns,
        args.limit,
        args.direction.as_loki_param(),
    )?;

    let lines = flatten_streams(streams, args.direction);

    Ok(LogsQueryResult {
        datasource_uid: args.datasource_uid,
        query: args.query,
        start_ns,
        end_ns,
        direction: args.direction.as_loki_param().to_string(),
        limit: args.limit,
        count: lines.len(),
        lines,
    })
}

impl TableOutput for LogsQueryResult {
    fn render_table(&self) {
        if self.lines.is_empty() {
            println!("No log lines found.");
            return;
        }

        let headers = ["TIMESTAMP", "LABELS", "LINE"];
        let rows: Vec<Vec<String>> = self
            .lines
            .iter()
            .map(|line| {
                vec![
                    line.timestamp.clone(),
                    format_labels(&line.labels),
                    line.line.clone(),
                ]
            })
            .collect();

        render_aligned_table(&headers, &rows);
    }
}

fn flatten_streams(streams: Vec<LokiStream>, direction: LogDirectionArg) -> Vec<LogLine> {
    let mut lines: Vec<LogLine> = streams
        .into_iter()
        .flat_map(|stream| {
            stream.values.into_iter().map(move |(ts, line)| LogLine {
                timestamp: ns_to_rfc3339(&ts),
                timestamp_ns: ts,
                labels: stream.stream.clone(),
                line,
            })
        })
        .collect();

    lines.sort_by(|a, b| compare_timestamp_ns(&a.timestamp_ns, &b.timestamp_ns));

    if matches!(direction, LogDirectionArg::Backward) {
        lines.reverse();
    }

    lines
}

fn compare_timestamp_ns(left: &str, right: &str) -> Ordering {
    match (left.parse::<u128>(), right.parse::<u128>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
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
    use std::collections::BTreeMap;

    use crate::{cli::LogDirectionArg, grafana::models::LokiStream};

    use super::flatten_streams;

    #[test]
    fn backward_direction_returns_newest_first() {
        let stream = LokiStream {
            stream: BTreeMap::from([("service".to_string(), "api".to_string())]),
            values: vec![
                ("1710000000000000000".to_string(), "first".to_string()),
                ("1710000001000000000".to_string(), "second".to_string()),
            ],
        };

        let lines = flatten_streams(vec![stream], LogDirectionArg::Backward);
        let messages: Vec<&str> = lines.iter().map(|l| l.line.as_str()).collect();

        assert_eq!(messages, vec!["second", "first"]);
    }
}
