# Safety

Dejavu is designed around one invariant:

```text
The real command always runs.
```

Dejavu is not an execution cache. It never decides that a command can be skipped because a previous run exists. When a command runs in an agent context, Dejavu executes the real underlying binary, captures the result, stores the full output locally, and only then decides what compact output to print back to the agent.

Outside an agent context (your own terminal, pipelines, IDE internals under global activation), shims take a fast path: they resolve the real binary and exec it directly — nothing is captured, stored, or recorded.

## Guarantees

- Dejavu always executes the real underlying command.
- Dejavu does not cache execution results to skip work.
- Dejavu only compresses, deduplicates, or summarizes what is printed back.
- Dejavu preserves the real exit code.
- Dejavu stores full command output locally, subject to the configured storage cap.
- Dejavu does not send logs to any server.
- Dejavu can be bypassed.
- Mutating or sensitive commands should be passed through unless explicitly supported.
- The user can inspect the shim path and real binary resolution.

## What Dejavu Changes

Dejavu changes stdout and stderr only after the real command has completed. The exit code returned to the caller is the real command exit code.

Examples:

```bash
pnpm test
git diff
rg "SessionStore"
docker logs api
```

If a rerun is unchanged or nearly unchanged, Dejavu may print an unchanged notice, a compact delta, or a bounded summary. The full output remains recoverable:

```bash
dejavu show latest --stdout
dejavu show latest --stderr
```

## What Dejavu Does Not Change

Dejavu does not:

- skip tests, builds, searches, diffs, or logs
- modify files in your repository
- rewrite commands before executing them
- send command output to a remote service
- require an MCP server or agent prompt instruction
- optimize commands it cannot classify with confidence

## Bypass

Bypass Dejavu for one command:

```bash
DEJAVU=off pnpm test
```

The legacy bypass variable also works:

```bash
DEJAVU_DISABLED=1 pnpm test
```

Disable Dejavu for the current repo:

```bash
dejavu disable
```

Re-enable it:

```bash
dejavu enable
```

Remove Dejavu cache and generated shims for the current repo:

```bash
dejavu uninstall
```

`dejavu uninstall` does not remove the installed binary. Remove the binary with your package manager, for example:

```bash
cargo uninstall dejavu
```

## Inspect The Setup

Run:

```bash
dejavu doctor
```

For machine-readable diagnostics:

```bash
dejavu doctor --json
```

The doctor command reports:

- Dejavu version
- OS and shell
- repo root
- cache directory
- Dejavu binary path
- storage writability
- SQLite integrity
- generated shims
- whether the shim directory is in `PATH`
- whether supported commands resolve through Dejavu in the active session
- whether underlying real binaries can be found
- bypass mode

To verify an active session, run doctor through Dejavu:

```bash
dejavu start -- dejavu doctor
```

## Command Policy

Dejavu defaults to passthrough. It optimizes only recognized read-only or validation-oriented forms.

Optimized examples:

```bash
pnpm test
pnpm test:unit
npm run lint
yarn typecheck
bun test
tsc --noEmit
eslint src
vitest run
jest
pytest
cargo test
cargo clippy
cargo check
go test ./...
rg "needle"
grep -R needle src
find src -name "*.ts"
git status
git diff
git log
git show
docker logs api
docker compose logs api
```

Passthrough examples:

```bash
git commit -m "fix"
git push
git reset --hard
git checkout main
npm publish
pnpm install
pnpm run lint:fix
eslint --fix
cargo clippy --fix
vitest            # bare vitest defaults to watch mode
jest --watch
jest -u           # snapshot update rewrites files
docker build .
docker compose up
find . -delete
find . -exec rm {} \;
```

If a command is mutating, sensitive, interactive, watch-mode, or unclear, Dejavu should run it normally and leave the output untouched.

## Local Storage

Dejavu stores data under the user cache directory:

- macOS: `~/Library/Caches/dejavu`
- Linux and WSL: `$XDG_CACHE_HOME/dejavu` or `~/.cache/dejavu`

The cache contains command metadata, compact summaries, normalized output, and redacted full stdout/stderr logs. Redaction is best effort. Treat the cache as sensitive if your command output may contain secrets.

Clean old runs:

```bash
dejavu clean --older-than 14d
```

Remove the current repo cache:

```bash
dejavu clean --all
```

## Unsafe Behavior Reports

Please report immediately if Dejavu:

- changes an exit code
- optimizes a command that should be passthrough
- hides a safety-critical warning
- stores unredacted secrets that should have matched the built-in redactor
- makes it hard to recover full output

Use the unsafe behavior issue template and include a redacted sample if possible.
