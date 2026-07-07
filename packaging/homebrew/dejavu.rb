# typed: false
# frozen_string_literal: true

# Reference copy of the Homebrew formula. The release workflow regenerates this
# with real checksums (via scripts/render-homebrew-formula.sh) and pushes it to
# the Salnika/homebrew-dejavu tap as Formula/dejavu.rb. The <SHA256_*> markers
# below are placeholders; do not install from this copy directly.
class Dejavu < Formula
  desc "Stop showing coding agents the same command output twice"
  homepage "https://github.com/Salnika/dejavu"
  version "0.2.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Salnika/dejavu/releases/download/v0.2.1/dejavu-aarch64-apple-darwin.tar.gz"
      sha256 "<SHA256_MACOS_ARM>"
    else
      url "https://github.com/Salnika/dejavu/releases/download/v0.2.1/dejavu-x86_64-apple-darwin.tar.gz"
      sha256 "<SHA256_MACOS_X86>"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Salnika/dejavu/releases/download/v0.2.1/dejavu-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "<SHA256_LINUX_ARM>"
    else
      url "https://github.com/Salnika/dejavu/releases/download/v0.2.1/dejavu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "<SHA256_LINUX_X86>"
    end
  end

  def install
    bin.install "dejavu"
  end

  test do
    assert_match "dejavu 0.2.1", shell_output("#{bin}/dejavu --version")
  end
end
