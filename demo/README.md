# Dejavu Demo

This demo creates a temporary fake project and fake `pnpm` binary. It is deterministic and does not touch your source tree.

Run:

```bash
demo/run-demo.sh
```

The demo shows:

- a command with large output
- a repeated identical output
- a small delta
- full output retrieval with `dejavu show latest --stdout`

If `target/release/dejavu` does not exist, the script builds it first.
