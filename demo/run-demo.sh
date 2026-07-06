#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${DEJAVU_BIN:-"$repo_root/target/release/dejavu"}"

if [[ ! -x "$bin" ]]; then
  echo "Building Dejavu release binary..."
  (cd "$repo_root" && cargo build --release >/dev/null)
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/dejavu-demo.XXXXXX")"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

project="$tmp/project"
home="$tmp/home"
fakebin="$tmp/fake-bin"
mkdir -p "$project" "$home" "$fakebin"
base_path="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

cat >"$fakebin/pnpm" <<'SH'
#!/bin/sh
state="${DEMO_STATE:-base}"
i=1
while [ "$i" -le 140 ]; do
  if [ "$state" = "changed" ] && [ "$i" -eq 73 ]; then
    echo "FAIL packages/api/__tests__/login.test.ts expected 200 received 500"
  else
    echo "PASS packages/core/__tests__/case_$i.test.ts stable validation output line $i"
  fi
  i=$((i + 1))
done

if [ "$state" = "changed" ]; then
  echo "Tests: 1 failed, 139 passed"
  exit 1
fi

echo "Tests: 140 passed"
exit 0
SH
chmod +x "$fakebin/pnpm"

clean_env() {
  env \
    -u DEJAVU \
    -u DEJAVU_ACTIVE \
    -u DEJAVU_BIN \
    -u DEJAVU_CACHE_DIR \
    -u DEJAVU_DISABLED \
    -u DEJAVU_ORIG_ZDOTDIR \
    -u DEJAVU_REPO_ROOT \
    -u DEJAVU_SESSION_ID \
    -u DEJAVU_SHIM_DIR \
    "$@"
}

run_agent_command() {
  local label="$1"
  local state="$2"
  echo
  echo "== $label =="
  (
    cd "$project"
    clean_env \
      HOME="$home" \
      XDG_CACHE_HOME="$home/.cache" \
      PATH="$fakebin:$base_path" \
      DEMO_STATE="$state" \
      "$bin" start -- sh -c "pnpm test"
  )
}

run_agent_command "First run: large output" base
run_agent_command "Second run: repeated unchanged output" base
run_agent_command "Third run: small delta" changed || true

echo
echo "== Full output retrieval =="
(
  cd "$project"
  clean_env HOME="$home" XDG_CACHE_HOME="$home/.cache" PATH="$base_path" "$bin" show latest --stdout | sed -n '1,14p'
)

echo
echo "Demo complete."
