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

- keep signal-specific commands (`logs`, `metrics`, `traces`, `sql`)
- require explicit datasource selection (`--ds <uid>`) for reproducibility
- support range selection with either:
  - `--since <duration>`
  - `--from <rfc3339> --to <rfc3339>`

## Security

- never commit secrets or real tokens
- use least-privilege Grafana service account scopes
- ensure errors for 401/403 are clear and actionable

## Versioning and releases

`lgtmcli` currently ships a single release channel:

- **Stable**: semantic versions (`vX.Y.Z`)

Current phase (pre-1.0):

- breaking CLI/JSON changes: bump `MINOR` (`v0.4.0` -> `v0.5.0`)
- additive features/fixes: bump `PATCH` (`v0.5.0` -> `v0.5.1`)

After 1.0:

- follow SemVer strictly (`MAJOR.MINOR.PATCH`)

Before 1.0, breaking changes are allowed but must be called out clearly in release notes.

### What counts as a breaking change

- changing/removing command names, flags, or behavior relied on by scripts
- changing/removing existing keys in `--json` output

Human-readable table/text output may evolve more freely.

### Release automation

- Pushing a `vX.Y.Z` tag creates a stable release via
  `.github/workflows/release-stable.yml`.

Stable release command:

```bash
# make sure Cargo.toml has version = "0.2.0"
git tag v0.2.0
git push origin v0.2.0
```

See [docs/releases.md](./docs/releases.md) for artifact names and installer usage.
