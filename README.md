# lgtmcli

A fast CLI for querying Grafana-backed **logs, metrics, traces, and SQL datasources** from your terminal.

`lgtmcli` is optimized for both humans and automation:

- readable table output by default
- machine-readable `--json` output for scripts/agents
- explicit datasource routing with `--ds <uid>`

### Why?

- Incident response without dashboard click-through
- One CLI surface for Loki, Prometheus/Mimir, Tempo, and SQL datasources
- Script-friendly output contracts (`--json`)

## Install

### Option 1: Download a release binary

Download the archive for your OS/architecture from:

- https://github.com/knifecake/lgtmcli/releases/latest

Extract it and place `lgtmcli` somewhere on your `PATH` (for example `~/.local/bin`).

### Option 2: Use the install script (same release artifacts, automated)

```bash
curl -fsSL https://raw.githubusercontent.com/knifecake/lgtmcli/master/scripts/install.sh | sh
```

By default, this installs to:

- `~/.local/bin/lgtmcli`

Override the install location if needed:

```bash
curl -fsSL https://raw.githubusercontent.com/knifecake/lgtmcli/master/scripts/install.sh | sh -s -- --install-dir /usr/local/bin
```

### Option 3: Build and install locally

```bash
make            # release build (default target)
make install    # installs to ~/.local/bin/lgtmcli
```

## Authenticate

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

## Usage

A typical flow looks like this:

```bash
# 1) Find the datasource UIDs you want to query
lgtmcli ds list

# 2) Pull recent logs from Loki
lgtmcli logs query '{service="api"} |= "error"' --ds loki-prod --since 30m

# 3) Turn logs into a time series (LogQL stats)
lgtmcli logs stats 'rate({service="api"}[5m])' --ds loki-prod --since 1h --step 1m

# 4) Check metrics from Prometheus/Mimir
lgtmcli metrics range 'rate(http_requests_total[5m])' --ds mimir-prod --since 1h --step 30s

# 5) Inspect traces from Tempo
lgtmcli traces search '{ status = error }' --ds tempo-prod --since 1h --limit 20

# 6) Query SQL datasources
lgtmcli sql tables --ds pg-read-replica
lgtmcli sql query 'select id, email from users order by id desc limit 20' --ds pg-read-replica
```
Use `--json` on any command when scripting:

```bash
lgtmcli ds list --json
lgtmcli traces search '{}' --ds tempo-prod --since 1h --json
```

For time ranges, use either `--since <duration>` (for example `15m`, `1h`, `24h`) or explicit bounds with `--from <RFC3339> --to <RFC3339>`.

For the complete command reference, run:

```bash
lgtmcli --help
lgtmcli <command> --help
```


## Contributing

For development setup and workflow, see [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

This project is licensed under the **MIT License**. See [LICENSE](./LICENSE).
