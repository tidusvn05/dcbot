#!/usr/bin/env bash
# dcbot installer — https://github.com/<owner>/dcbot
#   curl -fsSL https://raw.githubusercontent.com/<owner>/dcbot/main/install.sh | bash
# Pin a version: DCBOT_VERSION=v0.2.0 bash install.sh
set -euo pipefail

REPO="${DCBOT_REPO:-tidusvn05/dcbot}"
VERSION="${DCBOT_VERSION:-latest}"
BIN_DIR="${DCBOT_BIN_DIR:-$HOME/.local/bin}"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()   { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing required tool: $1"; }
need curl
need tar

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  os_part="linux" ;;
  Darwin) os_part="macos" ;;
  *)      die "unsupported OS: $os (dcbot requires tmux — Unix only)" ;;
esac
case "$arch" in
  x86_64|amd64)   arch_part="x86_64" ;;
  aarch64|arm64)  arch_part="aarch64" ;;
  *)              die "unsupported arch: $arch" ;;
esac
target="dcbot-${os_part}-${arch_part}"

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/${REPO}/releases/latest/download"
else
  base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "downloading ${target} (${VERSION})"
curl -fsSL "${base}/${target}.tar.gz"     -o "${tmp}/dcbot.tar.gz" \
  || die "download failed — check that release ${VERSION} has asset ${target}"
curl -fsSL "${base}/${target}.tar.gz.sha256" -o "${tmp}/dcbot.tar.gz.sha256" \
  || warn "checksum file not found — skipping verification"

if [ -s "${tmp}/dcbot.tar.gz.sha256" ]; then
  (cd "$tmp" && {
    if command -v sha256sum >/dev/null; then sha256sum -c dcbot.tar.gz.sha256;
    elif command -v shasum  >/dev/null; then shasum -a 256 -c dcbot.tar.gz.sha256;
    else warn "no sha256 tool — skipping verification"; fi
  })
fi

tar -xzf "${tmp}/dcbot.tar.gz" -C "$tmp"
[ -f "${tmp}/dcbot" ] || die "archive did not contain a dcbot binary"

mkdir -p "$BIN_DIR"
install -m 0755 "${tmp}/dcbot" "${BIN_DIR}/dcbot"
info "installed to ${BIN_DIR}/dcbot"
case ":$PATH:" in
  *":${BIN_DIR}:"*) ;;
  *) warn "${BIN_DIR} is not on your PATH — add it to your shell profile" ;;
esac

for tool in tmux claude bun; do
  command -v "$tool" >/dev/null 2>&1 || warn "recommended tool not found: ${tool}"
done

info "done — run 'dcbot new <name>' to onboard your first bot"
