# Comparison

Dejavu is a command-output memory layer. It is closest to tools that reduce context, but its mechanism is different: it compares command output across repeated real executions.

| Tool | Main idea | Dejavu difference |
|---|---|---|
| RTK | Compress command output | Dejavu compares across repeated runs |
| read-once | Avoid rereading unchanged files | Dejavu targets shell command outputs |
| pxpipe | Move context into images | Dejavu stays text-based and shell-native |
| Prompt instructions | Ask the agent to behave | Dejavu intercepts automatically via PATH |

## Complementary To Compressors

Output compressors reduce a single large output. Dejavu is most useful when the agent runs the same command again and the output is unchanged or nearly unchanged.

These approaches can work together:

- a compressor can shrink first-seen output
- Dejavu can avoid repeating that output on later runs
- specialized parsers can make deltas more useful

## Not A Prompt

Dejavu does not ask the agent to remember a workflow. The agent keeps running ordinary commands such as:

```bash
pnpm test
git diff
rg "needle"
docker logs api
```

The interception happens through `PATH`, so it works even when the agent does not know Dejavu exists.

## Not An MCP

MCP tools are useful when the agent chooses to call them. Dejavu targets commands the agent already calls through the shell. It is not an MCP the agent may ignore.

## Not A Test Runner

Dejavu does not replace `pnpm test`, `pytest`, `cargo test`, or `go test`. It runs the real command and preserves the real exit code.

## Not An Execution Cache

Dejavu does not say "this command already ran, skip it." It reruns the command and compares the output.

## What It Is

Dejavu is a shell-native, text-based command-output memory layer for coding agents. It is built for the repetitive loop where agents rerun tests, searches, diffs, and logs while working on code.
