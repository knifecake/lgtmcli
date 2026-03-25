#!/usr/bin/env sh
set -eu

REPO="${LGTMCLI_REPO:-knifecake/lgtmcli}"
VERSION=""
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BINARY="lgtmcli"

usage() {
  cat <<EOF
Install lgtmcli from GitHub Releases.

Usage:
  install.sh [--version <tag>] [--install-dir <path>] [--repo <owner/repo>]

Examples:
  install.sh
  install.sh --version v0.1.0
  install.sh --install-dir /usr/local/bin
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --repo)
      REPO="$2"
      shift 2
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

for cmd in curl tar python3; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
done

api_get() {
  endpoint="$1"
  curl -fsSL -H "Accept: application/vnd.github+json" "https://api.github.com/repos/${REPO}/${endpoint}"
}

if [ -z "$VERSION" ]; then
  VERSION="$(api_get "releases/latest" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
fi

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)
    echo "Unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

case "$os" in
  linux)
    if [ "$arch" != "x86_64" ]; then
      echo "Unsupported Linux architecture: $arch (supported: x86_64)" >&2
      exit 1
    fi
    target="x86_64-unknown-linux-musl"
    ext="tar.gz"
    ;;
  darwin)
    if [ "$arch" = "x86_64" ]; then
      target="x86_64-apple-darwin"
    else
      target="aarch64-apple-darwin"
    fi
    ext="tar.gz"
    ;;
  *)
    echo "Unsupported OS: $os" >&2
    exit 1
    ;;
esac

archive="lgtmcli-${VERSION}-${target}.${ext}"
base_url="https://github.com/${REPO}/releases/download/${VERSION}"
archive_url="${base_url}/${archive}"
checksums_url="${base_url}/checksums.txt"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

echo "Installing ${BINARY} ${VERSION} for ${target}..."
curl -fsSL -o "${tmp_dir}/${archive}" "${archive_url}"

if curl -fsSL -o "${tmp_dir}/checksums.txt" "${checksums_url}"; then
  expected="$(grep "  ${archive}$" "${tmp_dir}/checksums.txt" | awk '{print $1}')"
  if [ -n "$expected" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      actual="$(sha256sum "${tmp_dir}/${archive}" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
      actual="$(shasum -a 256 "${tmp_dir}/${archive}" | awk '{print $1}')"
    else
      actual=""
      echo "Warning: no sha256 tool found, skipping checksum verification" >&2
    fi

    if [ -n "$actual" ] && [ "$actual" != "$expected" ]; then
      echo "Checksum verification failed for ${archive}" >&2
      exit 1
    fi
  fi
fi

tar -xzf "${tmp_dir}/${archive}" -C "$tmp_dir"
mkdir -p "$INSTALL_DIR"
cp "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

echo "✅ Installed to ${INSTALL_DIR}/${BINARY}"
echo "Run '${BINARY} --help' to get started."
