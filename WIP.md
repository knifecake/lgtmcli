# WIP: metrics + traces implementation

## Goals

- [x] Add `metrics` command family with ergonomic subcommands for humans and agents
- [x] Add `traces` command family with search and trace-by-id flows
- [x] Keep output behavior consistent (`table` default, `--json` optional)
- [x] Reuse shared time-range patterns where it makes sense
- [x] Add unit + integration tests for new command flows
- [x] Run full lint/test loop
- [x] Polish README and help text

## UX decisions

- Keep signal-specific commands (`logs`, `metrics`, `traces`) instead of generic `query`
- Require explicit datasource via `--ds` for deterministic scripting
- Time ranges:
  - `--since` for quick relative windows
  - `--from/--to` for explicit ranges
- Consistent JSON payloads with metadata + flattened result rows

## Implemented commands

- [x] `metrics query <promql> --ds <uid> [--time <rfc3339>]`
- [x] `metrics range <promql> --ds <uid> [--since|--from --to] [--step <duration>]`
- [x] `traces search <traceql> --ds <uid> [--since|--from --to] [--limit <n>]`
- [x] `traces get <trace_id> --ds <uid>`

## Notes

- Tempo search uses `/api/search` with `q`, `start`, `end`, and `limit`.
- Tempo trace-by-id uses `/api/v2/traces/<trace_id>`.
- Shared time parsing/range validation moved into `src/time.rs`.

## Future ideas

- Add datasource auto-resolution helpers (explicit default + friendly aliases)
- Add `logs tail` and label discovery (`logs labels`, `logs values`)
- Add optional `--raw` mode for traces get table output
- Add pagination/streaming strategy for very large result sets
