#!/usr/bin/env bash
set -euo pipefail

START_MARKER="# >>> dejavu dev cli >>>"
END_MARKER="# <<< dejavu dev cli <<<"

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
legacy_bin_dir="${XDG_DATA_HOME:-$HOME/.local/share}/dejavu/dev/bin"

path_contains() {
  needle="$1"
  old_ifs="$IFS"
  IFS=:
  for entry in $PATH; do
    if [ "$entry" = "$needle" ]; then
      IFS="$old_ifs"
      return 0
    fi
  done
  IFS="$old_ifs"
  return 1
}

choose_bin_dir() {
  if [ -n "${DEJAVU_DEV_BIN_DIR:-}" ]; then
    printf "%s" "$DEJAVU_DEV_BIN_DIR"
    return
  fi

  for dir in "$HOME/.local/bin" "$HOME/bin"; do
    if path_contains "$dir"; then
      printf "%s" "$dir"
      return
    fi
  done

  old_ifs="$IFS"
  IFS=:
  for dir in $PATH; do
    case "$dir" in
      "$HOME"/*)
        if [ -d "$dir" ] && [ -w "$dir" ]; then
          IFS="$old_ifs"
          printf "%s" "$dir"
          return
        fi
        ;;
    esac
  done
  IFS="$old_ifs"

  printf "%s" "$HOME/.local/bin"
}

bin_dir="$(choose_bin_dir)"
wrapper="$bin_dir/dejavu"
legacy_wrapper="$legacy_bin_dir/dejavu"

without_path_entry() {
  remove="$1"
  input="$2"
  output=""
  old_ifs="$IFS"
  IFS=:
  for entry in $input; do
    [ "$entry" = "$remove" ] && continue
    if [ -z "$output" ]; then
      output="$entry"
    else
      output="$output:$entry"
    fi
  done
  IFS="$old_ifs"
  printf "%s" "$output"
}

if [ -n "${DEJAVU_SHIM_DIR:-}" ]; then
  PATH="$(without_path_entry "$DEJAVU_SHIM_DIR" "$PATH")"
  export PATH
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "bootstrap:cli: cargo is required but was not found on PATH" >&2
  exit 1
fi

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

shell_quote() {
  printf "'%s'" "$(printf "%s" "$1" | sed "s/'/'\\\\''/g")"
}

mkdir -p "$bin_dir"
cat >"$wrapper" <<EOF
#!/usr/bin/env bash
set -euo pipefail

without_path_entry() {
  remove="\$1"
  input="\$2"
  output=""
  old_ifs="\$IFS"
  IFS=:
  for entry in \$input; do
    [ "\$entry" = "\$remove" ] && continue
    if [ -z "\$output" ]; then
      output="\$entry"
    else
      output="\$output:\$entry"
    fi
  done
  IFS="\$old_ifs"
  printf "%s" "\$output"
}

if [ -n "\${DEJAVU_SHIM_DIR:-}" ]; then
  PATH="\$(without_path_entry "\$DEJAVU_SHIM_DIR" "\$PATH")"
  export PATH
fi

exec cargo run --quiet --manifest-path $(shell_quote "$repo_root/Cargo.toml") --bin dejavu -- "\$@"
EOF
chmod +x "$wrapper"

cargo build --quiet --manifest-path "$repo_root/Cargo.toml" --bin dejavu

if [ "$legacy_wrapper" != "$wrapper" ]; then
  rm -f "$legacy_wrapper"
  rmdir "$legacy_bin_dir" 2>/dev/null || true
fi

if path_contains "$bin_dir"; then
  if [ -f "$rc_file" ] && grep -Fqx "$START_MARKER" "$rc_file"; then
    tmp_file="$(mktemp "${TMPDIR:-/tmp}/dejavu-rc.XXXXXX")"
    awk -v start="$START_MARKER" -v end="$END_MARKER" '
      $0 == start { skip = 1; next }
      $0 == end { skip = 0; next }
      !skip { print }
    ' "$rc_file" >"$tmp_file"
    mv "$tmp_file" "$rc_file"
  fi
else
  mkdir -p "$(dirname "$rc_file")"
  tmp_file="$(mktemp "${TMPDIR:-/tmp}/dejavu-rc.XXXXXX")"
  if [ -f "$rc_file" ]; then
    awk -v start="$START_MARKER" -v end="$END_MARKER" '
      $0 == start { skip = 1; next }
      $0 == end { skip = 0; next }
      !skip { print }
    ' "$rc_file" >"$tmp_file"
  else
    : >"$tmp_file"
  fi

  {
    printf "\n%s\n" "$START_MARKER"
    printf "export PATH=%s:\"\$PATH\"\n" "$(shell_quote "$bin_dir")"
    printf "%s\n" "$END_MARKER"
  } >>"$tmp_file"
  mv "$tmp_file" "$rc_file"
fi

echo "dejavu dev CLI installed: $wrapper"
if path_contains "$bin_dir"; then
  echo "dejavu is available in the current shell."
else
  echo "PATH updated in: $rc_file"
  echo "Open a new shell or run: export PATH=$(shell_quote "$bin_dir"):\"\$PATH\""
fi
