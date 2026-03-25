use assert_cmd::Command;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;
use serde_json::Value;

#[test]
fn auth_status_succeeds_with_valid_credentials() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"[
                    {"id": 1, "uid": "loki-prod", "name": "Loki Prod", "type": "loki", "isDefault": true}
                ]"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("credentials look good"))
        .stdout(predicate::str::contains("Visible datasources: 1"));

    datasource_mock.assert();
}

#[test]
fn auth_status_fails_on_unauthorized() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET).path("/api/datasources");
        then.status(401)
            .header("content-type", "application/json")
            .body(r#"{"message":"unauthorized"}"#);
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "bad-token")
        .args(["auth", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HTTP 401 Unauthorized"));

    datasource_mock.assert();
}

#[test]
fn datasources_list_json_applies_type_filter() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET).path("/api/datasources");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"[
                    {"id": 1, "uid": "loki-prod", "name": "Loki Prod", "type": "loki", "isDefault": true},
                    {"id": 2, "uid": "mimir-prod", "name": "Mimir Prod", "type": "prometheus", "isDefault": false},
                    {"id": 3, "uid": "loki-dev", "name": "Loki Dev", "type": "Loki", "isDefault": false}
                ]"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args(["ds", "list", "--type", "loki", "--json"]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["ds_type"], "loki");
    assert_eq!(payload["count"], 2);
    assert_eq!(payload["datasources"].as_array().unwrap().len(), 2);
    assert!(
        payload["datasources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["type"].as_str().unwrap().eq_ignore_ascii_case("loki"))
    );

    datasource_mock.assert();
}

#[test]
fn logs_query_json_returns_lines() {
    let server = MockServer::start();
    let logs_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/loki-prod/loki/api/v1/query_range")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "status": "success",
                    "data": {
                        "resultType": "streams",
                        "result": [
                            {
                                "stream": {"service": "api", "level": "error"},
                                "values": [
                                    ["1710000000000000000", "boom"],
                                    ["1710000001000000000", "still bad"]
                                ]
                            }
                        ]
                    }
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "logs",
            "query",
            "{service=\"api\"}",
            "--ds",
            "loki-prod",
            "--from",
            "2024-01-01T00:00:00Z",
            "--to",
            "2024-01-01T00:10:00Z",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["datasource_uid"], "loki-prod");
    assert_eq!(payload["query"], "{service=\"api\"}");
    assert_eq!(payload["count"], 2);
    assert_eq!(payload["lines"].as_array().unwrap().len(), 2);
    assert_eq!(payload["lines"][0]["line"], "still bad");
    assert_eq!(payload["lines"][1]["line"], "boom");

    logs_mock.assert();
}

#[test]
fn logs_stats_json_returns_timeseries_samples() {
    let server = MockServer::start();
    let stats_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/loki-prod/loki/api/v1/query_range")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "status": "success",
                    "data": {
                        "resultType": "matrix",
                        "result": [
                            {
                                "metric": {"host": "app-1", "quantile": "0.95"},
                                "values": [
                                    [1710000000, "120.5"],
                                    [1710000060, "98.0"]
                                ]
                            }
                        ]
                    }
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "logs",
            "stats",
            "quantile_over_time(0.95, ({host=\"app-1\", role=\"web\"} |= \"gunicorn.access\" | json | unwrap server_time_ms)[1m])",
            "--ds",
            "loki-prod",
            "--from",
            "2024-01-01T00:00:00Z",
            "--to",
            "2024-01-01T01:00:00Z",
            "--step",
            "1m",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["result_type"], "matrix");
    assert_eq!(payload["count"], 2);
    assert_eq!(payload["samples"][0]["value"], "120.5");
    assert_eq!(payload["samples"][1]["value"], "98.0");

    stats_mock.assert();
}

#[test]
fn logs_stats_fails_for_stream_queries() {
    let server = MockServer::start();
    let stats_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/loki-prod/loki/api/v1/query_range")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "status": "success",
                    "data": {
                        "resultType": "streams",
                        "result": [
                            {
                                "stream": {"service": "api"},
                                "values": [["1710000000000000000", "line"]]
                            }
                        ]
                    }
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "logs",
            "stats",
            "{service=\"api\"}",
            "--ds",
            "loki-prod",
            "--from",
            "2024-01-01T00:00:00Z",
            "--to",
            "2024-01-01T01:00:00Z",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expects metric LogQL"));

    stats_mock.assert();
}

#[test]
fn metrics_query_json_returns_vector_samples() {
    let server = MockServer::start();
    let metrics_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/mimir-prod/api/v1/query")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "status": "success",
                    "data": {
                        "resultType": "vector",
                        "result": [
                            {
                                "metric": {"__name__": "up", "job": "api"},
                                "value": [1710000000, "1"]
                            }
                        ]
                    }
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "metrics",
            "query",
            "up{job=\"api\"}",
            "--ds",
            "mimir-prod",
            "--time",
            "2024-01-01T00:00:00Z",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["mode"], "instant");
    assert_eq!(payload["result_type"], "vector");
    assert_eq!(payload["count"], 1);
    assert_eq!(payload["samples"][0]["value"], "1");

    metrics_mock.assert();
}

#[test]
fn metrics_range_json_returns_matrix_samples() {
    let server = MockServer::start();
    let metrics_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/mimir-prod/api/v1/query_range")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "status": "success",
                    "data": {
                        "resultType": "matrix",
                        "result": [
                            {
                                "metric": {"__name__": "up", "job": "api"},
                                "values": [
                                    [1710000000, "1"],
                                    [1710000060, "0"]
                                ]
                            }
                        ]
                    }
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "metrics",
            "range",
            "up{job=\"api\"}",
            "--ds",
            "mimir-prod",
            "--from",
            "2024-01-01T00:00:00Z",
            "--to",
            "2024-01-01T00:02:00Z",
            "--step",
            "1m",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["mode"], "range");
    assert_eq!(payload["count"], 2);
    assert_eq!(payload["samples"][0]["value"], "1");
    assert_eq!(payload["samples"][1]["value"], "0");

    metrics_mock.assert();
}

#[test]
fn sql_query_json_returns_rows_and_truncates_with_limit() {
    let server = MockServer::start();

    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/uid/pg-ro")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "id": 42,
                "uid": "pg-ro",
                "name": "Postgres Read Replica",
                "type": "postgres",
                "isDefault": false
            }"#,
            );
    });

    let query_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/ds/query")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "results": {
                    "A": {
                        "frames": [
                            {
                                "schema": {
                                    "fields": [
                                        {"name": "id"},
                                        {"name": "email"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        [1, 2],
                                        ["a@example.com", "b@example.com"]
                                    ]
                                }
                            }
                        ]
                    }
                }
            }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "sql",
            "query",
            "select id, email from users order by id",
            "--ds",
            "pg-ro",
            "--limit",
            "1",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["datasource_uid"], "pg-ro");
    assert_eq!(payload["datasource_type"], "postgres");
    assert_eq!(payload["row_count"], 1);
    assert_eq!(payload["total_row_count"], 2);
    assert_eq!(payload["truncated"], true);
    assert_eq!(payload["rows"][0]["id"], "1");
    assert_eq!(payload["rows"][0]["email"], "a@example.com");

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_query_rejects_non_sql_datasource_types() {
    let server = MockServer::start();

    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/uid/loki-prod")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "id": 7,
                "uid": "loki-prod",
                "name": "Loki Prod",
                "type": "loki",
                "isDefault": false
            }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args(["sql", "query", "select 1", "--ds", "loki-prod"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not a supported SQL datasource type",
        ));

    datasource_mock.assert();
}

#[test]
fn traces_search_json_returns_trace_summaries() {
    let server = MockServer::start();
    let traces_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/tempo-main/api/search")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "traces": [
                        {
                            "traceID": "abc123",
                            "rootServiceName": "checkout",
                            "rootTraceName": "POST /pay",
                            "startTimeUnixNano": "1710000000000000000",
                            "durationMs": 95.1,
                            "spanSets": [{"spans": [{"spanID": "1"}, {"spanID": "2"}]}]
                        }
                    ]
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args([
            "traces",
            "search",
            "{ status = error }",
            "--ds",
            "tempo-main",
            "--from",
            "2024-01-01T00:00:00Z",
            "--to",
            "2024-01-01T01:00:00Z",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["count"], 1);
    assert_eq!(payload["traces"][0]["trace_id"], "abc123");
    assert_eq!(payload["traces"][0]["root_service"], "checkout");
    assert_eq!(payload["traces"][0]["span_count"], 2);

    traces_mock.assert();
}

#[test]
fn traces_get_prints_summary_in_table_mode() {
    let server = MockServer::start();
    let trace_get_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/proxy/uid/tempo-main/api/v2/traces/abc123")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                    "resourceSpans": [
                        {
                            "scopeSpans": [
                                {"spans": [{"spanId": "1"}, {"spanId": "2"}]}
                            ]
                        }
                    ]
                }"#,
            );
    });

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "test-token")
        .args(["traces", "get", "abc123", "--ds", "tempo-main"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Trace ID: abc123"))
        .stdout(predicate::str::contains("Detected spans: 2"));

    trace_get_mock.assert();
}
