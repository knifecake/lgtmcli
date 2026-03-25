use assert_cmd::Command;
use httpmock::Method::GET;
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
