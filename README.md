# lgtmcli

A CLI for exploring Grafana-backed **logs, metrics, and traces** from your terminal.

`lgtmcli` is optimized for both humans and agents:

- readable table output by default
- machine-readable `--json` output
- explicit datasource routing with `--ds <uid>`

## License

This project is licensed under the **MIT License**. See [LICENSE](./LICENSE).

---

## 1) Install

### Build and install locally

```bash
make            # release build (default target)
make install    # installs to ~/.local/bin/lgtmcli
```

Or run without installing:

```bash
cargo run -- --help
```

---

## 2) Authenticate

Set Grafana URL + token (read-only service account recommended):

```bash
export GRAFANA_URL="https://<cluster>.grafana.net"
export GRAFANA_TOKEN="<grafana_service_account_token>"
```

Validate auth:

```bash
lgtmcli auth status
```

---

## 3) Discover datasources

List all datasources:

```bash
lgtmcli datasources list
# alias:
lgtmcli ds list
```

Filter by type:

```bash
lgtmcli ds list --type loki
lgtmcli ds list --type prometheus
lgtmcli ds list --type tempo
lgtmcli ds list --type postgres
```

Use these UIDs with `--ds` in all query commands.

---

## 4) Run queries

## Logs

### Log lines

```bash
lgtmcli logs query '{service="api"}' --ds loki-prod --since 1h
```

### Log stats (metric-style LogQL)

```bash
lgtmcli logs stats 'rate({service="api"}[5m])' --ds loki-prod --since 1h --step 1m
```

Example (gunicorn p95 per minute over the last hour):

```bash
lgtmcli logs stats 'avg by () (quantile_over_time(0.95, ({host="app-1", role="web"} |= "gunicorn.access" | json | unwrap server_time_ms)[1m]))' \
  --ds loki-prod --since 1h --step 1m
```

## Metrics

Instant query:

```bash
lgtmcli metrics query 'up{job="api"}' --ds mimir-prod
```

Range query:

```bash
lgtmcli metrics range 'rate(http_requests_total[5m])' --ds mimir-prod --since 1h --step 30s
```

## Traces

Search:

```bash
lgtmcli traces search '{ status = error }' --ds tempo-prod --since 1h --limit 20
```

Get full trace by ID:

```bash
lgtmcli traces get <trace_id> --ds tempo-prod
```

## SQL (Postgres/MySQL/MSSQL datasources)

Discover tables:

```bash
lgtmcli sql tables --ds pg-read-replica
lgtmcli sql tables --ds pg-read-replica --schema public --like user%
```

Describe a table:

```bash
lgtmcli sql describe users --schema public --ds pg-read-replica
# or schema-qualified:
lgtmcli sql describe public.users --ds pg-read-replica
```

Run read-only SQL:

```bash
lgtmcli sql query 'select id, email from users order by id desc limit 20' --ds pg-read-replica
```

The SQL command enforces read-only statements (for example `SELECT`, `WITH`, `SHOW`, `EXPLAIN`) and rejects write statements.

Datasource type aliases such as `grafana-postgresql-datasource` are supported.
If you use a custom SQL plugin type, you can bypass the type gate with `--force` on `sql query`.

---

## 5) Output for scripts

Use `--json` with any command:

```bash
lgtmcli ds list --json
lgtmcli logs stats 'rate({service="api"}[5m])' --ds loki-prod --since 1h --step 1m --json
lgtmcli metrics range 'up' --ds mimir-prod --since 15m --step 30s --json
lgtmcli traces search '{}' --ds tempo-prod --since 1h --json
lgtmcli sql tables --ds pg-read-replica --json
lgtmcli sql describe users --schema public --ds pg-read-replica --json
lgtmcli sql query 'select now() as ts' --ds pg-read-replica --json
```

---

## Time options

Commands with time ranges support either:

- `--since <duration>` (examples: `15m`, `1h`, `24h`)
- `--from <RFC3339> --to <RFC3339>`

`--since` and `--from/--to` are mutually exclusive.

---

## Full capabilities

- **auth**
  - `auth status`
- **datasources**
  - `datasources list` (alias: `ds list`)
- **logs**
  - `logs query`
  - `logs stats`
- **metrics**
  - `metrics query`
  - `metrics range`
- **traces**
  - `traces search`
  - `traces get`
- **sql**
  - `sql tables`
  - `sql describe`
  - `sql query`

### API routes used via Grafana datasource proxy

- Metrics
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query`
  - `/api/datasources/proxy/uid/<metrics_uid>/api/v1/query_range`
- Logs
  - `/api/datasources/proxy/uid/<logs_uid>/loki/api/v1/query_range`
- Traces
  - `/api/datasources/proxy/uid/<traces_uid>/api/search`
  - `/api/datasources/proxy/uid/<traces_uid>/api/v2/traces/<trace_id>`
- SQL
  - `/api/datasources/uid/<sql_uid>` (datasource metadata lookup)
  - `/api/ds/query` (Grafana datasource query API)

---

## Contributing

For development setup and workflow, see [CONTRIBUTING.md](./CONTRIBUTING.md).
