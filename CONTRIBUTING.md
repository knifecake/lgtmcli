# Contributing

Thanks for contributing to `lgtmcli`.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- Access to a Grafana instance with LGTM datasources
- A Grafana service account token with datasource query permissions

## Local setup

```bash
git clone <repo-url>
cd lgtmcli
```

Set environment variables:

```bash
export GRAFANA_URL="https://<cluster>.grafana.net"
export GRAFANA_TOKEN="<grafana_service_account_token>"
```

Verify auth:

```bash
cargo run -- auth status
```

## Common development commands

```bash
make build      # debug build
make            # release build (default)
make install    # install to ~/.local/bin/lgtmcli
make lint       # fmt + check + clippy -D warnings
make test       # run unit + integration tests
```

## Testing approach

This project uses two layers of tests:

1. **Unit tests** for isolated logic (sorting, parsing, formatting, transformations)
2. **Integration tests** (`tests/cli.rs`) for command flows with minimal mocking
   using a local HTTP mock server

When adding commands:

- add unit tests for parsing/transform logic
- add at least one integration test for the command behavior and JSON output

## Output conventions

- default output: human-readable table/text
- `--json`: stable machine-readable output for scripts/agents

When adding fields, prefer additive changes and avoid breaking existing JSON keys.

## Command design conventions

- keep signal-specific commands (`logs`, `metrics`, `traces`)
- require explicit datasource selection (`--ds <uid>`) for reproducibility
- support range selection with either:
  - `--since <duration>`
  - `--from <rfc3339> --to <rfc3339>`

## Security

- never commit secrets or real tokens
- use least-privilege Grafana service account scopes
- ensure errors for 401/403 are clear and actionable
