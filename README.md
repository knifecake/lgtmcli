# lgtmcli

A lightweight CLI for exploring **metrics, logs, and traces** through Grafana datasource proxy APIs.

`lgtmcli` is designed for both humans and automation agents:

- human-friendly default table output
- machine-friendly `--json` output
- explicit datasource routing via `--ds <uid>` for reproducible scripts

## Architecture

`lgtmcli` uses **Grafana as a single API gateway** for telemetry queries:

- **Metrics** → Prometheus-compatible datasource (for example, Mimir)
- **Logs** → Loki
- **Traces** → Tempo

Instead of talking to telemetry backends directly, the CLI uses Grafana datasource proxy endpoints.

## Environment

```bash
export GRAFANA_URL="https://grafana.example.com"
export GRAFANA_TOKEN="<grafana_service_account_token>"
```

`lgtmcli` sends:

```http
Authorization: Bearer <GRAFANA_TOKEN>
```

Use a dedicated Grafana service account token with datasource query permissions.

## Install (local dev)

```bash
make            # release build (default target)
make install    # copies lgtmcli to ~/.local/bin/lgtmcli
```

## Command model

### Auth

```bash
lgtmcli auth status
```

### Datasources

```bash
lgtmcli datasources list
lgtmcli datasources list --type loki
lgtmcli ds list --type prometheus   # alias: ds
```

### Logs

```bash
lgtmcli logs query '{service="api"}' --ds loki-prod --since 1h
lgtmcli logs query '{service="api"}' --ds loki-prod --from 2026-03-25T10:00:00Z --to 2026-03-25T11:00:00Z
lgtmcli logs query '{service="api"}' --ds loki-prod --limit 200 --direction backward
```

### Metrics

```bash
# instant query (default time: now)
lgtmcli metrics query 'up{job="api"}' --ds mimir-prod

# instant query at explicit time
lgtmcli metrics query 'up{job="api"}' --ds mimir-prod --time 2026-03-25T11:00:00Z

# range query
lgtmcli metrics range 'rate(http_requests_total[5m])' --ds mimir-prod --since 1h --step 30s
lgtmcli metrics range 'up' --ds mimir-prod --from 2026-03-25T10:00:00Z --to 2026-03-25T11:00:00Z --step 1m
```

### Traces

```bash
lgtmcli traces search '{ status = error }' --ds tempo-prod --since 1h --limit 20
lgtmcli traces search '{}' --ds tempo-prod --from 2026-03-25T10:00:00Z --to 2026-03-25T11:00:00Z
lgtmcli traces get <trace_id> --ds tempo-prod
```

## Output modes

Default output is human-readable table/text.

Use `--json` for machine-readable output:

```bash
lgtmcli ds list --json
lgtmcli logs query '{service="api"}' --ds loki-prod --since 30m --json
lgtmcli metrics range 'up' --ds mimir-prod --since 15m --step 30s --json
lgtmcli traces search '{}' --ds tempo-prod --since 1h --json
```

## Time options

Commands that support ranges use one of:

- `--since <duration>` (examples: `15m`, `1h`, `24h`)
- `--from <RFC3339> --to <RFC3339>`

`--since` cannot be combined with `--from/--to`.

## Datasource proxy routes used

- Metrics
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query`
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query_range`
- Logs
  - `/api/datasources/proxy/uid/<logs_uid>/loki/api/v1/query_range`
- Traces
  - `/api/datasources/proxy/uid/<traces_uid>/api/search`
  - `/api/datasources/proxy/uid/<traces_uid>/api/v2/traces/<trace_id>`

## Development

```bash
make build
make lint
make test
```

## Security notes

- Never commit `GRAFANA_TOKEN`
- Prefer short-lived/rotatable tokens
- Use least-privilege service account scopes
- Return clear errors for 401/403 auth failures
