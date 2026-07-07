#!/usr/bin/env bash
# Prepare a realistic throwaway project (failing test suite + fake pnpm) and
# drop into a `dejavu start` shell inside it. Used by demo/dejavu.tape to
# record the README GIF; also runnable by hand for a live tour.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${DEJAVU_BIN:-"$repo_root/target/release/dejavu"}"
if [[ ! -x "$bin" ]]; then
  echo "Building Dejavu release binary..."
  (cd "$repo_root" && cargo build --release >/dev/null)
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/dejavu-demo.XXXXXX")"
project="$tmp/acme-api"
home="$tmp/home"
fakebin="$tmp/fake-bin"
mkdir -p "$project/src" "$home" "$fakebin"

# The "bug" the demo fixes live.
cat >"$project/src/billing.ts" <<'TS'
export function applyDiscount(subtotal: number, discount: number) {
  return subtotal + discount; // BUG: should subtract
}
TS

# Fake pnpm: a vitest-looking suite whose outcome depends on the source file.
cat >"$fakebin/pnpm" <<'SH'
#!/bin/sh
echo "> acme-api@2.1.0 test"
echo "> vitest run"
echo ""
if grep -q '+ discount' src/billing.ts 2>/dev/null; then
  i=1
  while [ "$i" -le 130 ]; do
    echo " ✓ src/tests/unit_$i.test.ts (12 tests) 34ms"
    i=$((i + 1))
  done
  echo " ❯ src/billing.test.ts (8 tests | 3 failed) 102ms"
  echo "   × applyDiscount subtracts the discount"
  echo "     → expected 90 to be 110 // Object.is equality"
  echo "   × invoice total applies discount before tax"
  echo "     → expected 108 to be 132"
  echo "   × checkout grand total"
  echo "     → expected 540 to be 660"
  echo ""
  echo " Test Files  1 failed | 129 passed (130)"
  echo "      Tests  3 failed | 1037 passed (1040)"
  exit 1
fi
i=1
while [ "$i" -le 130 ]; do
  echo " ✓ src/tests/unit_$i.test.ts (12 tests) 33ms"
  i=$((i + 1))
done
echo " ✓ src/billing.test.ts (8 tests) 98ms"
echo ""
echo " Test Files  130 passed (130)"
echo "      Tests  1040 passed (1040)"
exit 0
SH
chmod +x "$fakebin/pnpm"

(
  cd "$project"
  git init -q
  git config user.email demo@example.com
  git config user.name demo
  git add -A
  git commit -qm "initial commit"
)

cd "$project"
export HOME="$home"
export XDG_CACHE_HOME="$home/.cache"
export XDG_CONFIG_HOME="$home/.config"
export PATH="$fakebin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export PS1='➜ acme-api '
exec "$bin" start -- bash --noprofile --norc -i
