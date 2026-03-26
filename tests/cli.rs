use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;
use serde_json::Value;

fn make_temp_home(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "lgtmcli-tests-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp home");
    path
}

fn write_profile(home: &Path, url: &str, token: &str) {
    write_profile_in_config_dir(&home.join(".config"), url, token);
}

fn write_profile_in_config_dir(config_dir: &Path, url: &str, token: &str) {
    let profile_dir = config_dir.join("lgtmcli");
    fs::create_dir_all(&profile_dir).expect("create profile directory");
    let profile_path = profile_dir.join("profiles.json");
    let body = serde_json::json!({
        "schema_version": 1,
        "active_profile": "default",
        "profiles": {
            "default": {
                "grafana_url": url,
                "grafana_token": token
            }
        }
    });
    fs::write(
        &profile_path,
        serde_json::to_string_pretty(&body).expect("json profile"),
    )
    .expect("write profile");
}

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
fn auth_uses_saved_profile_when_flags_and_env_are_missing() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer profile-token");

        then.status(200)
            .header("content-type", "application/json")
            .body("[]");
    });

    let home = make_temp_home("profile-only");
    write_profile(&home, &server.url(""), "profile-token");

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GRAFANA_URL")
        .env_remove("GRAFANA_TOKEN")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("credentials look good"))
        .stdout(predicate::str::contains("Token source: saved profile"));

    datasource_mock.assert();
}

#[test]
fn auth_uses_xdg_config_home_for_saved_profile() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer xdg-token");

        then.status(200)
            .header("content-type", "application/json")
            .body("[]");
    });

    let home = make_temp_home("xdg-config-home-home");
    let xdg_config_home = make_temp_home("xdg-config-home-root");
    write_profile_in_config_dir(&xdg_config_home, &server.url(""), "xdg-token");

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("HOME", &home)
        .env("XDG_CONFIG_HOME", &xdg_config_home)
        .env_remove("GRAFANA_URL")
        .env_remove("GRAFANA_TOKEN")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("credentials look good"))
        .stdout(predicate::str::contains("Token source: saved profile"));

    datasource_mock.assert();
}

#[test]
fn auth_env_overrides_saved_profile() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer env-token");

        then.status(200)
            .header("content-type", "application/json")
            .body("[]");
    });

    let home = make_temp_home("env-overrides-profile");
    write_profile(&home, "https://example.invalid", "profile-token");

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env("GRAFANA_URL", server.url(""))
        .env("GRAFANA_TOKEN", "env-token")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Token source: environment variable",
        ));

    datasource_mock.assert();
}

#[test]
fn auth_flags_override_env_and_saved_profile() {
    let server = MockServer::start();
    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer flag-token");

        then.status(200)
            .header("content-type", "application/json")
            .body("[]");
    });

    let home = make_temp_home("flags-override-env-profile");
    write_profile(&home, "https://example.invalid", "profile-token");

    let url = server.url("");

    let mut cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    cmd.env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env("GRAFANA_URL", "https://another.invalid")
        .env("GRAFANA_TOKEN", "env-token")
        .args(["--url", &url, "--token", "flag-token", "auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Token source: --url/--token flag"));

    datasource_mock.assert();
}

#[test]
fn auth_login_saves_profile_and_allows_followup_status_without_env() {
    let server = MockServer::start();

    let home = make_temp_home("login-saves-profile");

    let url = server.url("");

    let mut login_cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    login_cmd
        .env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GRAFANA_URL")
        .env_remove("GRAFANA_TOKEN")
        .args([
            "--url",
            &url,
            "--token",
            "saved-token",
            "auth",
            "login",
            "--no-verify",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("saved Grafana credentials"));

    let status_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources")
            .header("authorization", "Bearer saved-token");

        then.status(200)
            .header("content-type", "application/json")
            .body("[]");
    });

    let mut status_cmd = Command::cargo_bin("lgtmcli").expect("binary exists");
    status_cmd
        .env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GRAFANA_URL")
        .env_remove("GRAFANA_TOKEN")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Token source: saved profile"));

    status_mock.assert();
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
                "type": "grafana-postgresql-datasource",
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
    assert_eq!(payload["datasource_type"], "grafana-postgresql-datasource");
    assert_eq!(payload["row_count"], 1);
    assert_eq!(payload["total_row_count"], 2);
    assert_eq!(payload["truncated"], true);
    assert_eq!(payload["rows"][0]["id"], 1);
    assert_eq!(payload["rows"][0]["email"], "a@example.com");

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_query_json_coerces_time_columns_to_rfc3339() {
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
                "type": "grafana-postgresql-datasource",
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
                                        {"name": "opened_at", "type": "time"},
                                        {"name": "flag", "type": "boolean"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        [0],
                                        [true]
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
            "select opened_at, true as flag from users",
            "--ds",
            "pg-ro",
            "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["rows"][0]["opened_at"], "1970-01-01T00:00:00Z");
    assert_eq!(payload["rows"][0]["flag"], true);

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_schemas_lists_rows() {
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
                                        {"name": "schema_name"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        ["public", "app"]
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
        .args(["sql", "schemas", "--ds", "pg-ro", "--json"]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["columns"][0], "schema_name");
    assert_eq!(payload["row_count"], 2);
    assert_eq!(payload["rows"][0]["schema_name"], "public");

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_tables_lists_rows() {
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
                                        {"name": "schema_name"},
                                        {"name": "table_name"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        ["public", "public"],
                                        ["users", "orders"]
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
        .args(["sql", "tables", "--ds", "pg-ro", "--json"]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["columns"][0], "schema_name");
    assert_eq!(payload["columns"][1], "table_name");
    assert_eq!(payload["row_count"], 2);
    assert_eq!(payload["rows"][0]["table_name"], "users");

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_describe_lists_columns() {
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
                                        {"name": "schema_name"},
                                        {"name": "column_name"},
                                        {"name": "data_type"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        ["public", "public"],
                                        ["id", "email"],
                                        ["bigint", "text"]
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
            "sql", "describe", "users", "--schema", "public", "--ds", "pg-ro", "--json",
        ]);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let payload: Value = serde_json::from_str(&stdout).expect("valid json output");

    assert_eq!(payload["row_count"], 2);
    assert_eq!(payload["rows"][0]["column_name"], "id");
    assert_eq!(payload["rows"][1]["data_type"], "text");

    datasource_mock.assert();
    query_mock.assert();
}

#[test]
fn sql_describe_fails_when_table_not_found() {
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
                                        {"name": "schema_name"},
                                        {"name": "column_name"},
                                        {"name": "data_type"}
                                    ]
                                },
                                "data": {
                                    "values": [
                                        [],
                                        [],
                                        []
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
        .args(["sql", "describe", "does_not_exist", "--ds", "pg-ro"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "table 'does_not_exist' was not found",
        ));

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
            "is not recognized as SQL by lgtmcli",
        ));

    datasource_mock.assert();
}

#[test]
fn sql_query_force_allows_unknown_datasource_type() {
    let server = MockServer::start();

    let datasource_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/datasources/uid/custom-sql")
            .header("authorization", "Bearer test-token");

        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "id": 77,
                "uid": "custom-sql",
                "name": "Custom SQL Plugin",
                "type": "my-custom-sql-plugin",
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
                                    "fields": [{"name": "value"}]
                                },
                                "data": {
                                    "values": [[1]]
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
            "select 1 as value",
            "--ds",
            "custom-sql",
            "--force",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"row_count\": 1"));

    datasource_mock.assert();
    query_mock.assert();
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
