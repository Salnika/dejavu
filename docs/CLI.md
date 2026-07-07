# CLI Reference

```text
dejavu <COMMAND> [OPTIONS]
```

Global options: `-h, --help` on every command, `-V, --version` at the top level.

All state lives outside your repos, in the user cache
(`~/Library/Caches/dejavu` on macOS, `$XDG_CACHE_HOME/dejavu` or
`~/.cache/dejavu` on Linux/WSL), one directory per repo. Global config:
`~/.config/dejavu/config.toml`; optional per-project `.dejavu.toml` at the repo
root (never created automatically).

---

## Session & activation

### `dejavu start <COMMAND>...`

Launch a coding agent (or any command) with Dejavu active.

| Argument | Meaning |
|---|---|
| `<COMMAND>...` | The command to launch, with its arguments, passed verbatim |

- Detects the repo root (`git rev-parse --show-toplevel`, falling back to the
  working directory), prepares the cache, generates the shims, prepends the
  shim directory to `PATH`, and sets the `DEJAVU_*` session variables.
- Also sets a wrapper `ZDOTDIR` so login shells (`zsh -lc`, used by agents
  like Codex to run commands) keep the shims first on `PATH` even after
  macOS `path_helper` and `brew shellenv` rebuild it.
- stdin/stdout/stderr are inherited; the agent's exit code is propagated.

```bash
dejavu start claude
dejavu start codex
dejavu start claude --continue     # flags after the command go to the agent
dejavu start -- bash               # any command works
```

### `dejavu shellenv [OPTIONS]`

Global activation — for shells Dejavu did not launch (IDE integrated terminals,
GUI-launched agents).

| Option | Meaning |
|---|---|
| _(none)_ | Print the activation line for `eval "$(dejavu shellenv)"` |
| `--install` | Write a managed block into your shell profile(s) |
| `--uninstall` | Remove that managed block |
| `--shell <zsh\|bash\|sh>` | Target one shell; default for install/uninstall is all three |

```bash
dejavu shellenv --install     # ~/.zshrc, ~/.bashrc, ~/.profile
dejavu shellenv --uninstall   # undo
eval "$(dejavu shellenv)"      # or wire it up yourself
```

- Generates a repo-independent shim directory (`<cache>/shims/bin`) honoring
  the global `[intercept]` config, then emits/writes an idempotent POSIX guard
  that prepends it to `PATH`.
- `--install` is idempotent: re-running updates the single managed block (never
  duplicates it), and `--uninstall` restores the file.
- No `DEJAVU_*` variable is needed: the repo context is rebuilt from the
  working directory of each command, and shims self-identify to prevent
  recursion.

### `dejavu init`

Initialize the cache for the current repo (does not modify the repo). Optional
— `dejavu start` does it too.

### `dejavu enable` / `dejavu disable`

Per-repo toggle, stored in the repo's cache (never in the repo). While
disabled, every shim is a pure passthrough for this repo.

---

## Inspecting captured runs

Run targets: `latest`, a full run id, or a unique short prefix (git-style —
errors if the prefix is ambiguous).

### `dejavu show [OPTIONS] <TARGET>`

Show a captured run. Default: the compact output the agent saw.

| Option | Meaning |
|---|---|
| `--stdout` | Print the stored raw stdout of the run |
| `--stderr` | Print the stored raw stderr of the run |
| `--normalized` | Print the normalized text used for run comparison |

Inside an active session, `show` marks the run as `full_output_requested`
(feeds the quality metric in `stats`); outside a session it does not.

```bash
dejavu show latest
dejavu show latest --stdout
dejavu show 8c51f73 --stderr
```

### `dejavu grep [OPTIONS] <TARGET> <PATTERN>`

Search the stored raw output of a run with a regex.

| Option | Meaning |
|---|---|
| `--normalized` | Search the normalized text instead of the raw output |

Grep-style exit code: `0` at least one match, `1` no match, `2` bad pattern or
missing run.

```bash
dejavu grep latest "AssertionError"
dejavu grep 8c51f73 "TS2322"
```

---

## Measuring savings

### `dejavu stats [OPTIONS]`

Token-savings report for the current repo.

| Option | Meaning |
|---|---|
| `--json` | Emit the stats as JSON instead of text |
| `--all` | Aggregate across every repo Dejavu has ever tracked |
| `--public` | Omit repo paths and command details that may contain private names |

- Reports run counts by state (first seen / unchanged / small delta / large
  delta / passthrough), raw vs emitted vs saved token estimates, and quality
  metrics (full-output requests, internal fallbacks, average overhead).
- `--all` scans every repo cache **read-only** (it never migrates or touches
  a database a live session is writing), lists a per-repo breakdown, and
  reports `repos_tracked` / `repos_unreadable` in the JSON.

```bash
dejavu stats
dejavu stats --all
dejavu stats --all --public --json
```

### `dejavu repos [OPTIONS]`

List repos where Dejavu has recorded activity. By default, only repos that are
not disabled with `dejavu disable` are shown.

| Option | Meaning |
|---|---|
| `--json` | Emit the repo list as JSON instead of text |
| `--all` | Include repos disabled with `dejavu disable` |

Each row includes the repo path, status (`active` or `disabled`), run count,
session count, estimated saved tokens, latest activity timestamp, and cache
directory.

```bash
dejavu repos
dejavu repos --all
dejavu repos --all --json
```

### `dejavu report [OPTIONS]`

Emit a Markdown report for the current repo, suitable for sharing.

| Option | Meaning |
|---|---|
| `--redact` | Omit repo paths and command details that may contain private names |

### `dejavu bench [OPTIONS]`

Run a reproducible local benchmark suite through the REAL classify + reduce
pipeline — no LLM, no network, no toolchain needed (deterministic synthetic
outputs).

| Option | Meaning |
|---|---|
| `--scenario <NAME>` | Run one scenario (default: all) |
| `--json` | Emit the benchmark report as JSON instead of text |
| `--check` | Fail (exit 2) if any scenario misses its expectations — used as a CI regression gate |

Scenarios: `js-validation-loop` (every state + fail→pass), `git-workflow`,
`search-loop`, `large-output` (caps/truncation), and `machine-safety` (asserts
machine-readable git forms are NEVER reduced). The full run also reports an
end-to-end latency micro-bench (p50/p95 per command, spawning the real binary
— reported, never gated).

---

## Maintenance

### `dejavu doctor [OPTIONS]`

Diagnose the setup: binary reachable, cache writable, SQLite integrity, shims
generated, real binary resolvable behind each shim, `PATH` active in-session,
config valid.

| Option | Meaning |
|---|---|
| `--json` | Emit the checks as JSON instead of text |

Exit code: `0` when healthy, non-zero when a check fails.

### `dejavu clean [OPTIONS]`

Remove cached runs and logs for the current repo.

| Option | Meaning |
|---|---|
| `--older-than <AGE>` | Only remove runs older than this age, e.g. `14d`, `12h`, `30m` |
| `--all` | Remove every run, log, and shim for this repo's cache |

With no option, applies the configured retention (`retention_days`,
default 14 days).

### `dejavu uninstall`

Remove Dejavu's local cache and generated shims for the current repo. The
repo itself is never touched. To remove the binary afterwards:
`cargo uninstall dejavu`.

---

## Internal

### `dejavu run --shim-name <NAME> -- <ARGS>...`

The shim↔runtime protocol (hidden from `--help`). Every generated shim is:

```sh
#!/bin/sh
exec "${DEJAVU_BIN:-/abs/path/to/dejavu}" run --shim-name pnpm -- "$@"
```

It resolves the real binary further down `PATH`, always executes it, captures
and reduces the output, and exits with the **exact** real exit code (including
`127` for not-found and `128+signal`). On any internal error it prints the raw
output and the real exit code. Not meant to be invoked by hand.

---

## Environment variables

| Variable | Direction | Meaning |
|---|---|---|
| `DEJAVU=off` (or `0`, `false`, `disabled`) | you → dejavu | Bypass Dejavu for a command: `DEJAVU=off pnpm test` |
| `DEJAVU_DISABLED=1` | you → dejavu | Same as above (legacy form) |
| `DEJAVU_FORCE=1` | you → dejavu | Force reduction under global activation even without an agent marker / TTY |
| `DEJAVU_ACTIVE=1` | dejavu → session | Set inside a `dejavu start` session |
| `DEJAVU_BIN` | dejavu → session | Absolute path to the `dejavu` binary used by shims |
| `DEJAVU_REPO_ROOT` | dejavu → session | Detected repo root |
| `DEJAVU_CACHE_DIR` | dejavu → session | This repo's cache directory |
| `DEJAVU_SESSION_ID` | dejavu → session | Current session id (groups runs in `stats`) |
| `DEJAVU_SHIM_DIR` | dejavu → session | The shim directory on `PATH` |
| `DEJAVU_ORIG_ZDOTDIR` | dejavu → session | The user's original `ZDOTDIR`, sourced by the wrapper zdot files |

None of the session variables are required: shims work from `PATH` alone
(see `shellenv`).

Under **global activation** (no session), reduction additionally requires an
agent context — one of:

- a **pipe-capturing** agent marker (`CLAUDECODE`, `CODEX_SANDBOX`,
  `CURSOR_AGENT`): Claude Code, the Codex CLI and Cursor read command output
  through a pipe, so no terminal is required; or
- a **pty-based** agent marker (`AI_AGENT`, `COPILOT_AGENT`, set by VS Code
  Copilot) **and** stdout being a terminal; or
- `DEJAVU_FORCE=1`.

Anything else (your own terminals, pipelines, `$(…)` substitutions, IDE SCM)
takes a fast path: the real binary is resolved and exec'd directly — raw
output, native speed, and nothing is recorded in the cache.

---

## Configuration keys

`~/.config/dejavu/config.toml`, overridable per project by `.dejavu.toml`.
Defaults shown; the resolved config is written to `config.effective.json` in
the cache.

```toml
enabled = true
store_raw_outputs = true
redact_secrets = true
max_raw_output_bytes = 5242880        # 5 MB; beyond, head+tail is stored
min_raw_tokens_to_reduce = 800        # below, output passes through untouched
max_emitted_lines_first_seen = 160
max_emitted_lines_large_delta = 160
max_emitted_lines_small_delta = 120
small_delta_max_changed_lines = 80
small_delta_max_changed_ratio = 0.20
estimate_tokens_method = "chars_div_4"
retention_days = 14

[intercept]                           # one switch per shimmed command
npm = true
pnpm = true
yarn = true
bun = true
tsc = true
eslint = true
pytest = true
cargo = true
go = true
git = true
rg = true
grep = true
find = true
ls = true
tree = true
docker = true
extra = []                            # your own commands, e.g. ["vitest", "make"]
```

`extra` entries each get a shim and generic validation-style reduction (dedup,
deltas, bounded summaries, test-runner output sniffing). Watch modes pass
through, the min-token floor and agent gating apply, and builtins listed in
`extra` keep their specialized policy (e.g. `git` mutating subcommands still
pass through). Removing a name sweeps its shim on the next `dejavu start` /
`dejavu shellenv --install`.
