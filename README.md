# lgtmcli

A lightweight CLI for exploring **metrics, logs, and traces** from a Grafana LGTM stack.

## Architecture (current)

`lgtmcli` uses **Grafana as the single API gateway** for telemetry queries:

- **Metrics** → Prometheus-compatible datasource (for example, Mimir)
- **Logs** → Loki (LogQL API)
- **Traces** → Tempo (TraceQL / trace APIs)

Instead of talking directly to backend services, the CLI calls Grafana datasource proxy endpoints.

## Why this design

- One auth model (single token)
- One public endpoint (`GRAFANA_URL`)
- No need to expose telemetry backends directly
- Keeps backend topology private and stable

## Environment

```bash
export GRAFANA_URL="https://grafana.example.com"
export GRAFANA_TOKEN="<grafana_service_account_token>"
```

## Authentication

`lgtmcli` sends:

```http
Authorization: Bearer <GRAFANA_TOKEN>
```

Use a dedicated Grafana Service Account token with datasource query permissions.

## Datasource routing

The CLI uses Grafana datasource proxy routes by signal type:

- Metrics:
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query`
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query_range`
- Logs:
  - `/api/datasources/proxy/uid/<logs_uid>/loki/api/v1/query`
  - `/api/datasources/proxy/uid/<logs_uid>/loki/api/v1/query_range`
- Traces:
  - `/api/datasources/proxy/uid/<traces_uid>/api/search`
  - `/api/datasources/proxy/uid/<traces_uid>/api/v2/traces/<trace_id>`

## Command model

Implemented:

```bash
lgtmcli auth status
lgtmcli datasources list
lgtmcli datasources list --type loki

# shorthand
lgtmcli ds list --type prometheus
```

Output defaults to human-readable table/text. Pass `--json` for machine-readable output.

Planned:

```bash
lgtmcli metrics '<promql>'
lgtmcli logs '<logql>'
lgtmcli traces '<traceql>'
```

## Current MVP (implemented)

The current binary can:

- verify credentials against the Grafana API (`auth status`)
- list available datasources (`datasources list` / `ds list`)
- filter datasources by type (`--type`)

Examples:

```bash
cargo run -- auth status
cargo run -- ds list
cargo run -- ds list --type loki
cargo run -- ds list --type loki --json

# or after build:
# lgtmcli auth status
# lgtmcli ds list --type tempo --json
```

It requires:

- `GRAFANA_URL`
- `GRAFANA_TOKEN`

A successful run means token + datasource proxy access are working.

## Security notes

- Never commit `GRAFANA_TOKEN`
- Prefer short-lived/rotatable tokens
- Use least-privilege service account scopes
- Return clear errors for 401/403 auth failures
