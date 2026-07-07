# Benchmark

The current benchmark evidence is early. Treat it as a small measurement, not a broad token-cost claim.

Correct wording:

```text
In this early benchmark, Dejavu reduced intercepted command output by 52-55% in campaign 2.
```

Avoid wording that claims a 55% reduction in total token costs.

The strongest effect appears in repeated rerun loops.

## Methodology

Two early campaigns were run against real Codex sessions:

- Campaign 1: 12 real sessions
- Campaign 2: 12 real sessions

The benchmark measured command output that Dejavu intercepted. It compared estimated output tokens before and after Dejavu's compact output. Token estimates use Dejavu's configured approximation, not a model-specific tokenizer.

Repeated local rerun loops were measured separately with four runs of the same local workflow to estimate the best-case effect when an agent repeatedly asks for nearly identical output.

## What Was Measured

- Intercepted command output.
- Compact output returned by Dejavu.
- Estimated raw output tokens.
- Estimated emitted output tokens.
- Estimated reduction on intercepted outputs.
- Success count for the sessions in each campaign.
- Approximate overhead added by Dejavu.
- Full-output requests during the benchmark.

## What Was Not Measured

- Total prompt or completion cost for the whole agent session.
- Tokens from non-shell tools.
- The agent's internal reasoning tokens.
- Human time saved.
- Long-term behavior across many repositories.
- Accuracy effects from compact output.
- Every possible command shape.

## Raw-ish Table

| Metric | Campaign 1 | Campaign 2 |
|---|---:|---:|
| Benchmark type | early real Codex sessions | early real Codex sessions |
| Sessions | 12 | 12 |
| Reduction on intercepted outputs | 38.8-40.2% | 52.3-54.8% |
| Repeated local loop, 4 runs | approximately 72% | 87.0% |
| Success | 12/12 | 12/12 |
| Average overhead | approximately 62 ms | approximately 60 ms |
| Full-output requests | 0 | 0 |

Campaign 2 non-cached input reduction by effort:

| Effort | Without Dejavu | With Dejavu | Reduction |
|---|---:|---:|---:|
| low | 47,150 | 21,748 | -54% |
| medium | 33,124 | 22,940 | -31% |
| xhigh | 45,027 | 22,808 | -49% |

## Built-in Benchmark Suite

The repository includes a deterministic synthetic suite. It does not replace
the real-session benchmark, but every step runs through the REAL `classify()`
and `reduce()` pipeline (nothing is mocked), so it measures — and guards —
what the pipeline actually does.

```bash
cargo run -- bench            # full suite + latency micro-bench
cargo run -- bench --json     # machine-readable report
cargo run -- bench --check    # regression gate: exit 2 on any violation (runs in CI)
cargo run -- bench --scenario git-workflow
```

The README chart is generated from this suite by
`scripts/render-benchmark-chart.py`. In the chart, the gray bar is the raw
output an agent would read without Dejavu; the green bar is what it reads
with Dejavu. Bars are scaled per scenario (each pair relative to its own
gray bar) so the 1M-token scenario doesn't flatten the others; the labeled
token counts are the real absolute values.

### What each scenario simulates

**`js-validation-loop`** — the classic agent debugging loop: five runs of
`pnpm test` over a 120-case suite that walk through every reduction state.
Run 1 fails one test (bounded first output: the failure, not the 119 PASS
lines); run 2 is identical (`unchanged` — the core dedup); run 3 fails a
*different* test (`small_delta`: just the diff); run 4 is a completely
different output (`large_delta`: bounded summary); run 5 passes
(**fail → pass**: "command now passes." plus what used to fail). Reduction is
"only" ~56% because the first run must still show the useful content — by
design. `--check` requires all four states plus the fail→pass transition.

**`git-workflow`** — `git diff` over 40 files, three times: first run is
summarized by the git-diff parser (files + per-file `+/-` counts instead of
the full patch), an identical re-run dedups to `unchanged`, a modified diff
produces a delta.

**`search-loop`** — `rg createSession src` with 180 matches, three times:
first run gives count + files + a sample; identical re-run dedups; a re-run
with 5 new matches shows only the added matches.

**`large-output`** — a 40,000-line build log (`pnpm run build`), twice. This
exercises the volume guards: passing-run output reduced to its tail summary,
the 14K-char hard cap (so IDE agents like Copilot always capture the envelope
inline), and dedup on the second run. It dominates the suite total — read the
"overall" number with that in mind.

**`machine-safety`** — `git status --porcelain`, `git diff --name-only`, and
`git log --oneline ..@{upstream}`: machine-readable forms that shell prompts,
IDE source control, git hooks, and `$(git …)` substitutions PARSE. The
expected result is **zero reduction**: byte-identical passthrough. Equal bars
in the chart are the safety guarantee made visible, and `--check` fails if a
single one of these steps is ever reduced.

### Expectations and the CI gate

Each scenario declares expectations: required state coverage, a minimum
reduction floor, the 15K inline cap on every emitted output, and (for
machine-safety) strict passthrough. `dejavu bench --check` exits non-zero on
any violation and runs in CI, so a change that regresses reduction or —
worse — starts reducing parsed output fails the build.

### Latency micro-bench

The full run also spawns the actual binary end-to-end (shim pipeline,
capture, reduce, record) against a trivial fake tool, ~15 iterations after a
warmup, and reports p50/p95 per-command overhead. It is machine-dependent, so
it is reported but never gated.

Other properties: deterministic token totals, no LLM, no network, isolated
temporary cache per scenario.

## Stats And Reports

After using Dejavu in a project:

```bash
dejavu stats
dejavu stats --json
dejavu stats --public
dejavu stats --all
```

Share a redacted Markdown report:

```bash
dejavu report --redact > dejavu-report.md
```

Public and redacted modes omit repo paths and top command details that may contain private names. They do not include full command output logs.

## How To Reproduce Locally

1. Install Dejavu from the checkout:

   ```bash
   cargo install --path .
   ```

2. Run the synthetic benchmark:

   ```bash
   dejavu bench --json
   ```

3. Run a local demo:

   ```bash
   demo/run-demo.sh
   ```

4. Use Dejavu with a real coding agent:

   ```bash
   cd your-project
   dejavu start -- codex
   ```

5. Work normally until the agent has rerun tests, searches, diffs, or logs.

6. Export a redacted report:

   ```bash
   dejavu report --redact > dejavu-report.md
   ```

## How To Submit An Anonymized Report

Open an issue or discussion with:

- Dejavu version
- OS and shell
- agent used
- rough project type
- `dejavu report --redact`
- any commands that compacted badly
- whether you needed `dejavu show latest --stdout`

Do not include raw command output unless you have reviewed and redacted it.

## Known Weaknesses

- The benchmark is small.
- It currently focuses on Codex sessions.
- It measures intercepted command output, not all tokens.
- It uses estimated token counts.
- The local repeated loop is a favorable scenario for Dejavu.
- Real projects with mostly unique output will see less reduction.
- Full-output request behavior needs more long-running data.
- Parser quality varies by tool family.

## Next Benchmark Plan

- Run larger campaigns across Claude Code, Codex, Cursor agent, opencode, Aider, and Gemini CLI.
- Track whole-session input where it can be measured without private data.
- Separate results by command family.
- Include more monorepo and test-runner cases.
- Publish anonymized benchmark fixtures.
- Track full-output requests after compact output.
- Track false compacting and unsafe passthrough misses.
