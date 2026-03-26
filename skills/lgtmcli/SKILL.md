---
name: lgtmcli
description: >
  Use this skill when users mention Grafana, Loki, Mimir/Prometheus, Tempo,
  logs, traces, metrics, or SQL investigations. It covers lgtmcli
  authentication, datasource discovery, SQL schema/table inspection, and
  practical query workflows for incidents and bug reports.
---

# lgtmcli

Use this skill when a user wants to investigate incidents, bug reports, or product behavior via Grafana-backed logs, metrics, traces, or SQL from the terminal.

## 1) First: verify authentication

Always start with:

```bash
lgtmcli auth status
```

If auth is missing or invalid:

```bash
# Option A: interactive login (recommended)
lgtmcli auth login

# Option B: environment variables
export GRAFANA_URL="https://<your-grafana-url>"
export GRAFANA_TOKEN="<grafana-token>"
lgtmcli auth status
```

If users need full command details, point them to:

```bash
lgtmcli --help
lgtmcli <command> --help
```

## 2) Discover datasources

List all datasources first, then filter by type as needed:

```bash
lgtmcli d list
lgtmcli d list --type loki
lgtmcli d list --type prometheus
lgtmcli d list --type tempo
lgtmcli d list --type grafana-postgresql-datasource
```

Use datasource **UIDs** from this output with all query commands (`-d <uid>`).

## 3) SQL discovery workflow (schemas/tables/columns)

For SQL datasources, discover structure before writing deep queries:

```bash
# 1) List schemas
lgtmcli sql schemas -d <sql_uid>

# 2) List tables (optionally narrow by schema)
lgtmcli sql tables -d <sql_uid>
lgtmcli sql tables -d <sql_uid> --schema public --like user%

# 3) Inspect table columns
lgtmcli sql describe users -d <sql_uid>
lgtmcli sql describe users -d <sql_uid> --schema public
```

Then run focused queries:

```bash
lgtmcli sql query 'select id, email from users order by id desc limit 20' -d <sql_uid>
```

## 4) Capability examples by command

```bash
# Logs (Loki)
lgtmcli logs query '{service="api"} |= "error"' -d <loki_uid> --since 30m --limit 200
lgtmcli logs stats 'rate({service="api"}[5m])' -d <loki_uid> --since 1h --step 1m

# Metrics (Mimir/Prometheus)
lgtmcli metrics query 'up' -d <metrics_uid>
lgtmcli metrics range 'rate(http_requests_total[5m])' -d <metrics_uid> --since 1h --step 30s

# Traces (Tempo)
lgtmcli traces search '{ status = error }' -d <tempo_uid> --since 1h --limit 20
lgtmcli traces get <trace_id> -d <tempo_uid>

# SQL
lgtmcli sql schemas -d <sql_uid>
lgtmcli sql tables -d <sql_uid> --schema public
lgtmcli sql describe <table> -d <sql_uid>
lgtmcli sql query 'select * from <table> limit 20' -d <sql_uid>
```

## 5) Best practices

- Start broad, then narrow: list datasources -> inspect schema/table shape -> run targeted queries.
- Prefer explicit time bounds (`--since` or `--from/--to`) to avoid huge scans.
- Use `--limit` during exploration to keep responses fast and readable.
- Use `--json` for agent/tool pipelines; keep table output for human triage.
- Keep datasource UIDs explicit (`-d`) to avoid querying the wrong backend.
- For SQL, enforce read-only credentials at the Grafana datasource/database layer.
- Avoid exposing secrets in shell history; prefer env vars + `auth login`.

## Installation via npx skills

This repository layout (`skills/lgtmcli/SKILL.md`) is compatible with `skills.sh` discovery.

```bash
# Install from GitHub repo (project-local)
npx skills add knifecake/lgtmcli --agent pi --skill lgtmcli

# Install from local checkout while developing
npx skills add . --agent pi --skill lgtmcli
```

Notes:
- Omit `--agent pi` to choose interactively.
- Add `-g` for global install.
- Use `npx skills list` to verify installation.
