use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::app::AppContext;
use crate::cli::{LogDirectionArg, LogsQueryArgs};
use crate::grafana::models::LokiStream;
use crate::output::{TableOutput, render_aligned_table};

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
    let range = resolve_time_range(
        args.since.as_deref(),
        args.from.as_deref(),
        args.to.as_deref(),
    )?;

    let streams = ctx.grafana.query_loki_range(
        &args.datasource_uid,
        &args.query,
        &range.start_ns,
        &range.end_ns,
        args.limit,
        args.direction.as_loki_param(),
    )?;

    let lines = flatten_streams(streams, args.direction);

    Ok(LogsQueryResult {
        datasource_uid: args.datasource_uid,
        query: args.query,
        start_ns: range.start_ns,
        end_ns: range.end_ns,
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

#[derive(Debug)]
struct TimeRange {
    start_ns: String,
    end_ns: String,
}

fn resolve_time_range(
    since: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<TimeRange> {
    resolve_time_range_at(since, from, to, SystemTime::now())
}

fn resolve_time_range_at(
    since: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    now: SystemTime,
) -> Result<TimeRange> {
    if since.is_some() && (from.is_some() || to.is_some()) {
        bail!("--since cannot be used together with --from/--to");
    }

    let (start, end) = match (from, to) {
        (Some(start), Some(end)) => {
            let start = parse_rfc3339(start)?;
            let end = parse_rfc3339(end)?;
            (start, end)
        }
        (Some(_), None) | (None, Some(_)) => {
            bail!("--from and --to must be provided together")
        }
        (None, None) => {
            let parsed_since = parse_since_duration(since.unwrap_or(DEFAULT_SINCE))?;
            let start = now
                .checked_sub(parsed_since)
                .context("computed start time underflow")?;
            (start, now)
        }
    };

    if start >= end {
        bail!("start time must be before end time");
    }

    Ok(TimeRange {
        start_ns: to_unix_ns_string(start)?,
        end_ns: to_unix_ns_string(end)?,
    })
}

fn parse_since_duration(raw: &str) -> Result<Duration> {
    let normalized = raw.trim().trim_start_matches('-');
    humantime::parse_duration(normalized)
        .with_context(|| format!("invalid --since value '{raw}' (examples: 15m, 1h, 24h)"))
}

fn parse_rfc3339(raw: &str) -> Result<SystemTime> {
    humantime::parse_rfc3339_weak(raw).with_context(|| format!("invalid RFC3339 timestamp '{raw}'"))
}

fn to_unix_ns_string(value: SystemTime) -> Result<String> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .context("time value is before Unix epoch")?;
    Ok(duration.as_nanos().to_string())
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

fn ns_to_rfc3339(ns: &str) -> String {
    let parsed = match ns.parse::<u128>() {
        Ok(value) => value,
        Err(_) => return ns.to_string(),
    };

    let secs = match u64::try_from(parsed / 1_000_000_000) {
        Ok(value) => value,
        Err(_) => return ns.to_string(),
    };
    let nanos = (parsed % 1_000_000_000) as u32;

    let timestamp = UNIX_EPOCH + Duration::new(secs, nanos);
    humantime::format_rfc3339_millis(timestamp).to_string()
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
    use std::time::{Duration, UNIX_EPOCH};

    use super::{resolve_time_range_at, to_unix_ns_string};

    #[test]
    fn since_and_from_to_are_mutually_exclusive() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let result = resolve_time_range_at(
            Some("1h"),
            Some("2024-01-01T00:00:00Z"),
            Some("2024-01-01T01:00:00Z"),
            now,
        );

        assert!(result.is_err());
    }

    #[test]
    fn from_and_to_must_be_provided_together() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let result = resolve_time_range_at(Some("1h"), Some("2024-01-01T00:00:00Z"), None, now);

        assert!(result.is_err());
    }

    #[test]
    fn default_since_is_one_hour() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let range = resolve_time_range_at(None, None, None, now).expect("range resolves");

        let expected_end = to_unix_ns_string(now).expect("end ns");
        let expected_start = to_unix_ns_string(now - Duration::from_secs(3600)).expect("start ns");

        assert_eq!(range.start_ns, expected_start);
        assert_eq!(range.end_ns, expected_end);
    }
}
