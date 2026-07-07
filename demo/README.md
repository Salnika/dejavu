# Dejavu Demo

This demo creates a temporary fake project and fake `pnpm` binary. It is deterministic and does not touch your source tree.

Run the scripted tour:

```bash
demo/run-demo.sh
```

Or drop into a live `dejavu start` shell inside the prepared project (what the
GIF records) and type the commands yourself:

```bash
demo/session.sh
pnpm test                      # big failing output -> bounded summary
pnpm test                      # unchanged -> deduplicated
sed -i.bak 's/subtotal + discount/subtotal - discount/' src/billing.ts
pnpm test                      # "command now passes."
dejavu stats
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
