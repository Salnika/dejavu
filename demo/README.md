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

## Recording the README GIF

The README embeds `demo/dejavu.gif`, generated with
[VHS](https://github.com/charmbracelet/vhs) from `demo/dejavu.tape`:

```bash
brew install vhs
cargo build --release
vhs demo/dejavu.tape
```

This drives `run-demo.sh` and writes `demo/dejavu.gif`. Commit the regenerated
GIF alongside any change to the demo.
