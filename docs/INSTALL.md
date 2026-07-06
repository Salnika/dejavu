# Installation

Dejavu installs from source as a single binary.

## Requirements

- Rust toolchain with Cargo
- macOS, Linux, or WSL
- `bash`, `zsh`, or `sh`

Native Windows support is not implemented.

## Source Install

From the repository checkout:

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
terminals, GUI-launched agents — add this at the end of `~/.zprofile`:

```bash
eval "$(dejavu shellenv)"
```

Deactivate by removing the line.

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
