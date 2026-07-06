# Contributing

Thanks for helping make Dejavu safer and more useful for real coding-agent loops.

## Install Locally

```bash
cargo install --path .
dejavu doctor
```

For development:

```bash
npm run bootstrap:cli
```

## Run Tests

```bash
cargo test
```

Run one test:

```bash
cargo test stats_all
```

## Lint And Format

```bash
cargo fmt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Add A Command Wrapper

1. Add the command name to `SHIM_NAMES` in `src/config/mod.rs`.
2. Add a boolean to `InterceptConfig`.
3. Enable it in `src/config/defaults.rs`.
4. Add classification logic in `src/exec/classify.rs`.
5. Prefer passthrough for ambiguous, mutating, interactive, or watch-mode command shapes.
6. Add tests showing safe forms are optimized and unsafe forms pass through.

## Add A Parser Or Normalizer

1. Add parser code under `src/reduce/parsers/`.
2. Wire it through `src/reduce/parsers/mod.rs`.
3. Keep raw output recoverable with `dejavu show`.
4. Prefer stable facts over prose.
5. Strip timestamps, durations, colors, progress noise, and other volatile text only when safe.
6. Add fixtures or integration tests for unchanged and small-delta behavior.

## Add Benchmark Cases

1. Extend `src/commands/bench.rs` for deterministic local scenarios.
2. Keep scenarios LLM-free and private-data-free.
3. Cover first-seen, unchanged, small-delta, and large-delta states.
4. Add or update tests in `tests/bench_repro.rs`.
5. Document any real-session benchmark changes in `docs/BENCHMARK.md`.

## Report Unsafe Behavior

Open an unsafe behavior issue if Dejavu:

- changes an exit code
- optimizes a command that should be passthrough
- hides important safety output
- fails to recover full output
- stores unredacted secrets that should have matched built-in patterns

Include the command, OS, shell, agent, Dejavu version, and a redacted sample when possible.

## Code Style

- Keep the safety invariant obvious: always run the real command.
- Default to passthrough when uncertain.
- Avoid new dependencies unless they clearly reduce risk or complexity.
- Keep docs honest about what is implemented.
- Do not add telemetry or network calls.
- Do not put generated files in the repository.
