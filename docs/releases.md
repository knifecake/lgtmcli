# Release process

This project ships a single release channel:

- **stable**: SemVer tags (`vX.Y.Z`)

## Stable releases

Stable releases are triggered by pushing a SemVer tag.

The workflow enforces that `Cargo.toml`'s `[package].version` matches the tag (for example tag `v0.2.0` requires `version = "0.2.0"`).

```bash
# choose next version
git tag v0.2.0
git push origin v0.2.0
```

That runs `.github/workflows/release-stable.yml` and publishes:

- `lgtmcli-vX.Y.Z-x86_64-unknown-linux-musl.tar.gz`
- `lgtmcli-vX.Y.Z-aarch64-unknown-linux-musl.tar.gz`
- `lgtmcli-vX.Y.Z-x86_64-apple-darwin.tar.gz`
- `lgtmcli-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- `lgtmcli-vX.Y.Z-x86_64-pc-windows-msvc.zip`
- `checksums.txt`

## Installer script

Users can install the latest stable release with:

```bash
curl -fsSL https://raw.githubusercontent.com/knifecake/lgtmcli/main/scripts/install.sh | sh
```

Pin to a specific tag:

```bash
curl -fsSL https://raw.githubusercontent.com/knifecake/lgtmcli/main/scripts/install.sh | sh -s -- --version v0.2.0
```
