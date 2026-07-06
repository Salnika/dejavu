#!/usr/bin/env bash
set -euo pipefail

START_MARKER="# >>> dejavu dev cli >>>"
END_MARKER="# <<< dejavu dev cli <<<"

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
legacy_bin_dir="${XDG_DATA_HOME:-$HOME/.local/share}/dejavu/dev/bin"

if [ -n "${DEJAVU_DEV_SHELL_RC:-}" ]; then
  rc_file="$DEJAVU_DEV_SHELL_RC"
else
  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    zsh) rc_file="$HOME/.zshrc" ;;
    bash) rc_file="$HOME/.bashrc" ;;
    *) rc_file="$HOME/.profile" ;;
  esac
fi

remove_wrapper() {
  dir="$1"
  prune_empty_dir="$2"
  wrapper="$dir/dejavu"
  if [ -f "$wrapper" ] && grep -Fq "$repo_root/Cargo.toml" "$wrapper"; then
    rm -f "$wrapper"
  fi
  if [ "$prune_empty_dir" = "1" ]; then
    rmdir "$dir" 2>/dev/null || true
  fi
}

if [ -n "${DEJAVU_DEV_BIN_DIR:-}" ]; then
  remove_wrapper "$DEJAVU_DEV_BIN_DIR" 1
fi
remove_wrapper "$HOME/.local/bin" 0
remove_wrapper "$HOME/bin" 0
remove_wrapper "$legacy_bin_dir" 1

if [ -f "$rc_file" ]; then
  tmp_file="$(mktemp "${TMPDIR:-/tmp}/dejavu-rc.XXXXXX")"
  awk -v start="$START_MARKER" -v end="$END_MARKER" '
    $0 == start { skip = 1; next }
    $0 == end { skip = 0; next }
    !skip { print }
  ' "$rc_file" >"$tmp_file"
  mv "$tmp_file" "$rc_file"
fi

echo "dejavu dev CLI removed from local bin directories"
echo "PATH block removed from: $rc_file"
