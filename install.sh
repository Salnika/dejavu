#!/bin/sh
# Dejavu installer.
#
#   curl -fsSL https://raw.githubusercontent.com/Salnika/dejavu/main/install.sh | sh
#
# Environment overrides:
#   DEJAVU_VERSION       tag to install (e.g. v0.1.0). Default: latest release.
#   DEJAVU_INSTALL_DIR   install directory. Default: $HOME/.local/bin.
set -eu

OWNER="Salnika"
REPO="dejavu"
BIN="dejavu"
VERSION="${DEJAVU_VERSION:-latest}"
INSTALL_DIR="${DEJAVU_INSTALL_DIR:-$HOME/.local/bin}"

err() { printf 'error: %s\n' "$1" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- detect platform --------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  *) err "unsupported OS '$os' (macOS, Linux, or WSL required)" ;;
esac
case "$arch" in
  x86_64 | amd64) arch_part="x86_64" ;;
  arm64 | aarch64) arch_part="aarch64" ;;
  *) err "unsupported architecture '$arch'" ;;
esac
target="${arch_part}-${os_part}"
asset="dejavu-${target}.tar.gz"

# --- resolve download URL ---------------------------------------------------
if [ "$VERSION" = "latest" ]; then
  base="https://github.com/${OWNER}/${REPO}/releases/latest/download"
else
  case "$VERSION" in
    v*) tag="$VERSION" ;;
    *) tag="v$VERSION" ;;
  esac
  base="https://github.com/${OWNER}/${REPO}/releases/download/${tag}"
fi
url="${base}/${asset}"

# --- downloader -------------------------------------------------------------
if have curl; then
  dl() { curl -fsSL "$1" -o "$2"; }
elif have wget; then
  dl() { wget -qO "$2" "$1"; }
else
  err "need 'curl' or 'wget' to download"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

printf 'Installing %s for %s (%s)...\n' "$BIN" "$target" "$VERSION"
dl "$url" "$tmp/$asset" || err "download failed: $url"

# --- verify checksum (best-effort) ------------------------------------------
if dl "${url}.sha256" "$tmp/$asset.sha256" 2>/dev/null; then
  expected="$(awk '{print $1}' "$tmp/$asset.sha256")"
  actual=""
  if have sha256sum; then
    actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
  elif have shasum; then
    actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
  fi
  if [ -n "$actual" ] && [ "$actual" != "$expected" ]; then
    err "checksum mismatch (expected $expected, got $actual)"
  fi
fi

# --- extract and install ----------------------------------------------------
tar -xzf "$tmp/$asset" -C "$tmp"
[ -f "$tmp/$BIN" ] || err "archive did not contain '$BIN'"
mkdir -p "$INSTALL_DIR"
if have install; then
  install -m 0755 "$tmp/$BIN" "$INSTALL_DIR/$BIN"
else
  cp "$tmp/$BIN" "$INSTALL_DIR/$BIN"
  chmod 0755 "$INSTALL_DIR/$BIN"
fi

printf '\nInstalled %s -> %s\n' "$BIN" "$INSTALL_DIR/$BIN"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    printf '\n%s is not on your PATH. Add this to your shell profile:\n' "$INSTALL_DIR"
    printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
    ;;
esac
printf '\nNext steps:\n  %s doctor\n  cd your-project && %s start -- codex\n' "$BIN" "$BIN"
