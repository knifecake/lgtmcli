#!/usr/bin/env bash
set -euo pipefail

# Smoke test for an installed lgtmcli binary against a Grafana stack.
#
# This script intentionally does not read or print credentials.
# It relies on normal lgtmcli auth resolution (env/profile).

BINARY="${BINARY:-lgtmcli}"
METRICS_DS="${LGTMCLI_METRICS_DS:-internal_mimir}"
LOGS_DS="${LGTMCLI_LOGS_DS:-internal_loki}"
TRACES_DS="${LGTMCLI_TRACES_DS:-internal_tempo}"
SQL_DS="${LGTMCLI_SQL_DS:-isfg_app_db}"

METRICS_QUERY="${LGTMCLI_METRICS_QUERY:-up}"
# Loki requires at least one non-empty matcher.
LOGS_QUERY="${LGTMCLI_LOGS_QUERY:-{job=~\".+\"}}"
TRACES_QUERY="${LGTMCLI_TRACES_QUERY:-{}}"
SQL_QUERY="${LGTMCLI_SQL_QUERY:-select 1 as ok}"

SINCE="${LGTMCLI_SINCE:-1h}"
LOGS_LIMIT="${LGTMCLI_LOGS_LIMIT:-5}"
TRACES_LIMIT="${LGTMCLI_TRACES_LIMIT:-5}"
RUN_SQL="${LGTMCLI_RUN_SQL:-true}"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/smoke-test.sh [options]

Options:
  --binary <path>        lgtmcli binary to execute (default: lgtmcli from PATH)
  --metrics-ds <uid>     metrics datasource UID (default: internal_mimir)
  --logs-ds <uid>        logs datasource UID (default: internal_loki)
  --traces-ds <uid>      traces datasource UID (default: internal_tempo)
  --sql-ds <uid>         SQL datasource UID (default: isfg_app_db)
  --since <duration>     range for logs/traces checks (default: 1h)
  --logs-limit <n>       log lines limit (default: 5)
  --traces-limit <n>     trace limit (default: 5)
  --skip-sql             skip SQL checks
  -h, --help             show this help

Environment overrides:
  BINARY, LGTMCLI_METRICS_DS, LGTMCLI_LOGS_DS, LGTMCLI_TRACES_DS, LGTMCLI_SQL_DS,
  LGTMCLI_METRICS_QUERY, LGTMCLI_LOGS_QUERY, LGTMCLI_TRACES_QUERY, LGTMCLI_SQL_QUERY,
  LGTMCLI_SINCE, LGTMCLI_LOGS_LIMIT, LGTMCLI_TRACES_LIMIT, LGTMCLI_RUN_SQL

Examples:
  ./scripts/smoke-test.sh
  ./scripts/smoke-test.sh --binary ~/.local/bin/lgtmcli --skip-sql
  LGTMCLI_LOGS_DS=loki-prod LGTMCLI_SQL_DS=pg-ro ./scripts/smoke-test.sh
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      BINARY="$2"
      shift 2
      ;;
    --metrics-ds)
      METRICS_DS="$2"
      shift 2
      ;;
    --logs-ds)
      LOGS_DS="$2"
      shift 2
      ;;
    --traces-ds)
      TRACES_DS="$2"
      shift 2
      ;;
    --sql-ds)
      SQL_DS="$2"
      shift 2
      ;;
    --since)
      SINCE="$2"
      shift 2
      ;;
    --logs-limit)
      LOGS_LIMIT="$2"
      shift 2
      ;;
    --traces-limit)
      TRACES_LIMIT="$2"
      shift 2
      ;;
    --skip-sql)
      RUN_SQL="false"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v "$BINARY" >/dev/null 2>&1; then
  echo "❌ lgtmcli binary not found: $BINARY" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "❌ python3 is required for JSON validation" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

json_count() {
  local file="$1"
  python3 - "$file" <<'PY'
import json,sys
payload=json.load(open(sys.argv[1], encoding='utf-8'))
print(payload.get('count', payload.get('row_count', -1)))
PY
}

echo "== lgtmcli smoke test =="
"$BINARY" --version
"$BINARY" auth status

# Verify expected datasource UIDs exist.
datasources_json="$tmp_dir/datasources.json"
"$BINARY" d list --json > "$datasources_json"

if [[ "$RUN_SQL" == "true" ]]; then
  python3 - "$datasources_json" "$METRICS_DS" "$LOGS_DS" "$TRACES_DS" "$SQL_DS" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1], encoding='utf-8'))
uids = {d.get('uid'): d.get('type') for d in payload.get('datasources', [])}
required = sys.argv[2:]
missing = [u for u in required if u and u not in uids]
if missing:
    print("❌ Missing datasource UID(s): " + ", ".join(missing), file=sys.stderr)
    sys.exit(1)
print("✅ Datasources found: " + ", ".join(f"{u}({uids[u]})" for u in required if u))
PY
else
  python3 - "$datasources_json" "$METRICS_DS" "$LOGS_DS" "$TRACES_DS" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1], encoding='utf-8'))
uids = {d.get('uid'): d.get('type') for d in payload.get('datasources', [])}
required = sys.argv[2:]
missing = [u for u in required if u and u not in uids]
if missing:
    print("❌ Missing datasource UID(s): " + ", ".join(missing), file=sys.stderr)
    sys.exit(1)
print("✅ Datasources found: " + ", ".join(f"{u}({uids[u]})" for u in required if u))
PY
fi

echo "== Metrics query =="
metrics_json="$tmp_dir/metrics.json"
"$BINARY" metrics query "$METRICS_QUERY" -d "$METRICS_DS" --json > "$metrics_json"
metrics_count="$(json_count "$metrics_json")"
echo "✅ metrics ok (count=${metrics_count})"

echo "== Logs query =="
logs_json="$tmp_dir/logs.json"
"$BINARY" logs query "$LOGS_QUERY" -d "$LOGS_DS" --since "$SINCE" --limit "$LOGS_LIMIT" --json > "$logs_json"
logs_count="$(json_count "$logs_json")"
echo "✅ logs ok (count=${logs_count})"

echo "== Traces search =="
traces_json="$tmp_dir/traces.json"
"$BINARY" traces search "$TRACES_QUERY" -d "$TRACES_DS" --since "$SINCE" --limit "$TRACES_LIMIT" --json > "$traces_json"
traces_count="$(json_count "$traces_json")"
echo "✅ traces ok (count=${traces_count})"

if [[ "$RUN_SQL" == "true" ]]; then
  echo "== SQL query =="
  sql_json="$tmp_dir/sql.json"
  "$BINARY" sql query "$SQL_QUERY" -d "$SQL_DS" --json > "$sql_json"
  sql_count="$(json_count "$sql_json")"
  echo "✅ sql ok (row_count=${sql_count})"
fi

echo "✅ Smoke test complete"
