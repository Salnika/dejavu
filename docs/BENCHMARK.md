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

## Built-in Local Benchmark

The repository includes a deterministic synthetic benchmark. It does not replace the real-session benchmark, but it is useful for checking that the reduction pipeline still works.

```bash
cargo run -- bench
cargo run -- bench --json
```

Expected properties:

- covers first-seen, unchanged, small-delta, and large-delta states
- has deterministic token totals
- requires no LLM
- uses an isolated temporary cache

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
