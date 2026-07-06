# Installation

Dejavu is a single binary. Pick whichever channel you prefer.

## Requirements

- macOS, Linux, or WSL
- `bash`, `zsh`, or `sh`
- Cargo/Rust only for the source and `cargo install` methods

Native Windows support is not implemented.

## Install Script

```bash
curl -fsSL https://raw.githubusercontent.com/Salnika/dejavu/master/install.sh | sh
```

Detects your OS and architecture, downloads the matching release binary,
verifies its checksum, and installs it to `~/.local/bin` (override with
`DEJAVU_INSTALL_DIR`). Pin a version with `DEJAVU_VERSION=v0.1.0`.

## Homebrew

```bash
brew tap Salnika/dejavu
brew install dejavu
```

## npm / npx

```bash
npx @salnika/dejavu start -- codex   # run without installing
npm install -g @salnika/dejavu       # or install globally
```

The npm package is a thin launcher that downloads the matching release binary
on install; the installed command is still `dejavu`. macOS and Linux (incl.
WSL) on x64/arm64.

## Cargo

```bash
cargo install dejavu-cli
```

## Prebuilt Binaries

Download the archive for your platform from the
[Releases page](https://github.com/Salnika/dejavu/releases), extract it, and
put the `dejavu` binary on your `PATH`. Each archive ships with a `.sha256`.

## From Source

```bash
cargo install --path .
dejavu doctor
```

This installs the `dejavu` binary into Cargo's bin directory.

## Local Development Wrapper

For development inside this checkout:

```bash
npm run bootstrap:cli
dejavu doctor
```

Remove the local wrapper:

```bash
npm run bootstrap:cli:clean
```

## Global Activation (optional)

To intercept commands in shells Dejavu did not launch — IDE integrated
terminals, GUI-launched agents — install the activation into your shell
profiles:

```bash
dejavu shellenv --install      # ~/.zshrc, ~/.bashrc, ~/.profile (idempotent)
dejavu shellenv --uninstall    # remove it
```

Use `--shell zsh|bash|sh` to target one shell. `dejavu shellenv` with no flag
just prints the line for a manual `eval "$(dejavu shellenv)"`.

## Smoke Test

After installing:

```bash
cd your-project
dejavu start -- sh -c 'pnpm test || true'
dejavu stats
dejavu doctor
```

Bypass for one command:

```bash
DEJAVU=off pnpm test
```

Remove the current repo cache and shims:

```bash
dejavu uninstall
```
