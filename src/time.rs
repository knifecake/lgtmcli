use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: SystemTime,
    pub end: SystemTime,
}

pub fn resolve_range(
    since: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    default_since: &str,
) -> Result<TimeRange> {
    resolve_range_at(since, from, to, default_since, SystemTime::now())
}

pub fn resolve_range_at(
    since: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    default_since: &str,
    now: SystemTime,
) -> Result<TimeRange> {
    if since.is_some() && (from.is_some() || to.is_some()) {
        bail!("--since cannot be used together with --from/--to");
    }

    let (start, end) = match (from, to) {
        (Some(start), Some(end)) => (parse_rfc3339(start)?, parse_rfc3339(end)?),
        (Some(_), None) | (None, Some(_)) => {
            bail!("--from and --to must be provided together")
        }
        (None, None) => {
            let duration = parse_since_duration(since.unwrap_or(default_since))?;
            let start = now
                .checked_sub(duration)
                .context("computed start time underflow")?;
            (start, now)
        }
    };

    if start >= end {
        bail!("start time must be before end time");
    }

    Ok(TimeRange { start, end })
}

pub fn parse_since_duration(raw: &str) -> Result<Duration> {
    let normalized = raw.trim().trim_start_matches('-');
    humantime::parse_duration(normalized)
        .with_context(|| format!("invalid duration '{raw}' (examples: 15m, 1h, 24h)"))
}

pub fn parse_rfc3339(raw: &str) -> Result<SystemTime> {
    humantime::parse_rfc3339_weak(raw).with_context(|| format!("invalid RFC3339 timestamp '{raw}'"))
}

pub fn to_unix_ns_string(value: SystemTime) -> Result<String> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .context("time value is before Unix epoch")?;
    Ok(duration.as_nanos().to_string())
}

pub fn to_unix_seconds_string(value: SystemTime) -> Result<String> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .context("time value is before Unix epoch")?;
    Ok(duration.as_secs().to_string())
}

pub fn ns_to_rfc3339(ns: &str) -> String {
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

pub fn seconds_to_rfc3339(seconds: &str) -> String {
    let secs = match seconds.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return seconds.to_string(),
    };
    let ts = UNIX_EPOCH + Duration::from_secs(secs);
    humantime::format_rfc3339_millis(ts).to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{resolve_range_at, to_unix_ns_string};

    #[test]
    fn since_and_from_to_are_mutually_exclusive() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let result = resolve_range_at(
            Some("1h"),
            Some("2024-01-01T00:00:00Z"),
            Some("2024-01-01T01:00:00Z"),
            "1h",
            now,
        );

        assert!(result.is_err());
    }

    #[test]
    fn default_since_is_respected() {
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let range = resolve_range_at(None, None, None, "30m", now).expect("range resolves");

        let expected_end = to_unix_ns_string(now).expect("end ns");
        let expected_start = to_unix_ns_string(now - Duration::from_secs(1800)).expect("start ns");

        assert_eq!(to_unix_ns_string(range.start).unwrap(), expected_start);
        assert_eq!(to_unix_ns_string(range.end).unwrap(), expected_end);
    }
}
