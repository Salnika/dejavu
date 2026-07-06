# dejavucli

Stop showing coding agents the same command output twice.

This npm package is a thin launcher: on install it downloads the prebuilt
`dejavu` binary for your platform from
[GitHub Releases](https://github.com/Salnika/dejavu/releases) and runs it. The
installed command is `dejavu`.

## Use without installing

```bash
npx dejavucli start -- codex
```

## Install globally

```bash
npm install -g dejavucli
dejavu doctor
```

macOS and Linux (including WSL) on x64 or arm64 are supported. The package
version tracks the `dejavu` release it downloads.

Full documentation: <https://github.com/Salnika/dejavu>.
