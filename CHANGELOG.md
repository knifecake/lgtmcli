# Changelog

## Unreleased

- Add low-friction auth login flow (`auth login`) with saved profile support (XDG config path), extensible profiles file schema, and credential precedence: env vars > profile
- Add Linux arm64 release artifact and installer support
- Drop macOS x86_64 release artifacts (Apple Silicon only)
- Harden local auth profile file permissions to `0600` on Unix and hide token input during interactive `auth login` prompts
- Percent-encode datasource UID/trace ID path segments in Grafana API requests and fix UTF-8-safe HTTP error truncation
- Clarify SQL safety model in docs: `lgtmcli` does not enforce read-only SQL client-side; configure SQL datasources with read-only DB credentials
- Breaking CLI UX update: rename `--ds` to `--datasource` (`-d`), add top-level command aliases (`d`, `l`, `m`, `t`, `s`), add short flags for common options, and remove `--url`/`--token` flags to avoid leaking secrets via shell history

## v0.1.0 - 2026-03-26

- Initial version
