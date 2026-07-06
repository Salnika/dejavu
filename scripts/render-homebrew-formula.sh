#!/usr/bin/env bash
# Render the Homebrew formula for a release tag, pulling each platform's
# checksum from the published `.sha256` release assets. Prints the formula to
# stdout. Used by the release workflow; also runnable by hand:
#
#   scripts/render-homebrew-formula.sh v0.1.0 > Formula/dejavu.rb
set -euo pipefail

tag="${1:?usage: render-homebrew-formula.sh <tag>}"
version="${tag#v}"
owner="Salnika"
repo="dejavu"
base="https://github.com/${owner}/${repo}/releases/download/${tag}"

sha_for() {
  # $1 = target triple
  local asset="dejavu-$1.tar.gz"
  curl -fsSL "${base}/${asset}.sha256" | awk '{print $1}'
}

sha_mac_arm="$(sha_for aarch64-apple-darwin)"
sha_mac_x86="$(sha_for x86_64-apple-darwin)"
sha_linux_arm="$(sha_for aarch64-unknown-linux-gnu)"
sha_linux_x86="$(sha_for x86_64-unknown-linux-gnu)"

cat <<RUBY
# typed: false
# frozen_string_literal: true

# Homebrew formula for Dejavu. Regenerated on each release.
class Dejavu < Formula
  desc "Stop showing coding agents the same command output twice"
  homepage "https://github.com/${owner}/${repo}"
  version "${version}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "${base}/dejavu-aarch64-apple-darwin.tar.gz"
      sha256 "${sha_mac_arm}"
    else
      url "${base}/dejavu-x86_64-apple-darwin.tar.gz"
      sha256 "${sha_mac_x86}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "${base}/dejavu-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${sha_linux_arm}"
    else
      url "${base}/dejavu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${sha_linux_x86}"
    end
  end

  def install
    bin.install "dejavu"
  end

  test do
    assert_match "dejavu ${version}", shell_output("#{bin}/dejavu --version")
  end
end
RUBY
