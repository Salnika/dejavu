# Releasing

Dejavu ships as prebuilt binaries (GitHub Releases), a crates.io crate, a
Homebrew tap, and a `curl | sh` installer. Everything is driven by pushing a
version tag; the [release workflow](../.github/workflows/release.yml) does the
rest.

## Cut a release

1. Bump `version` in `Cargo.toml`, commit, and push to `master`.
2. Tag and push:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

The `Release` workflow then:

- builds `dejavu` for macOS (arm64 + x86_64) and Linux (arm64 + x86_64) on
  native runners,
- packages each as `dejavu-<target>.tar.gz` + `.tar.gz.sha256`,
- creates the GitHub Release with those assets and `install.sh`,
- publishes to crates.io (if `CARGO_REGISTRY_TOKEN` is set),
- publishes the npm launcher package (if `NPM_TOKEN` is set),
- regenerates the Homebrew formula and pushes it to the tap
  (if `HOMEBREW_TAP_TOKEN` is set).

To re-run against an existing tag, use **Actions → Release → Run workflow** and
enter the tag.

## Required repository secrets

Both are optional — the corresponding job skips cleanly when the secret is
absent, so you can enable each channel when you are ready.

| Secret | Enables | How to create |
|---|---|---|
| `CARGO_REGISTRY_TOKEN` | crates.io publish | crates.io → Account Settings → API Tokens (scope: publish-update) |
| `NPM_TOKEN` | npm publish (`npx dejavucli`) | npmjs.com → Access Tokens → Generate (Automation / publish) |
| `HOMEBREW_TAP_TOKEN` | Homebrew tap update | A fine-grained PAT with **Contents: read/write** on `Salnika/homebrew-dejavu` |

The npm launcher package lives in [`npm/`](../npm); its version is set from the
tag at publish time and it downloads the matching release binary on install.

Add them under **Settings → Secrets and variables → Actions**.

## One-time setup for Homebrew

Create a public repo `Salnika/homebrew-dejavu` (the `homebrew-` prefix is
required by Homebrew tap conventions). The workflow writes `Formula/dejavu.rb`
into it on each release. Users then:

```bash
brew tap Salnika/dejavu
brew install dejavu
```

## Verifying a release locally

```bash
# The installer, pinned to a tag:
DEJAVU_VERSION=v0.1.0 sh install.sh

# Render the formula by hand (needs the release assets to exist):
scripts/render-homebrew-formula.sh v0.1.0
```
