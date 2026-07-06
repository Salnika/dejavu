"use strict";

// Resolve, download, and cache the platform-specific `dejavu` binary from the
// matching GitHub Release. Pure Node (no npm deps): https for download with
// redirect following, the system `tar` for extraction, crypto for checksum.

const fs = require("fs");
const os = require("os");
const path = require("path");
const https = require("https");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

const REPO = "Salnika/dejavu";
const pkg = require("../package.json");

// process.platform + process.arch -> Rust release target triple.
const TARGETS = {
  "darwin arm64": "aarch64-apple-darwin",
  "darwin x64": "x86_64-apple-darwin",
  "linux x64": "x86_64-unknown-linux-gnu",
  "linux arm64": "aarch64-unknown-linux-gnu",
};

function target() {
  const key = process.platform + " " + process.arch;
  const t = TARGETS[key];
  if (!t) {
    throw new Error(
      "unsupported platform '" +
        key +
        "'. Dejavu supports macOS and Linux (incl. WSL) on x64/arm64."
    );
  }
  return t;
}

function binaryPath() {
  return path.join(__dirname, "..", "vendor", "dejavu");
}

function tarballUrl() {
  const tag = "v" + pkg.version;
  return (
    "https://github.com/" +
    REPO +
    "/releases/download/" +
    tag +
    "/dejavu-" +
    target() +
    ".tar.gz"
  );
}

// GET with redirect following (GitHub -> object storage), streamed to `dest`.
function download(url, dest, redirects) {
  redirects = redirects || 0;
  return new Promise((resolve, reject) => {
    if (redirects > 10) return reject(new Error("too many redirects"));
    https
      .get(url, { headers: { "User-Agent": "dejavu-npm" } }, (res) => {
        const code = res.statusCode || 0;
        if ([301, 302, 303, 307, 308].indexOf(code) !== -1 && res.headers.location) {
          res.resume();
          return resolve(download(res.headers.location, dest, redirects + 1));
        }
        if (code !== 200) {
          res.resume();
          return reject(new Error("HTTP " + code + " for " + url));
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on("finish", () => file.close(() => resolve()));
        file.on("error", reject);
      })
      .on("error", reject);
  });
}

async function downloadText(url) {
  const tmp = path.join(os.tmpdir(), "dejavu-sum-" + process.pid);
  await download(url, tmp);
  const text = fs.readFileSync(tmp, "utf8");
  fs.rmSync(tmp, { force: true });
  return text;
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

// Download + verify + extract the binary to vendor/. Idempotent.
async function ensureBinary(opts) {
  opts = opts || {};
  const dest = binaryPath();
  if (fs.existsSync(dest) && !opts.force) return dest;

  const url = tarballUrl();
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "dejavu-"));
  try {
    const tarball = path.join(tmpDir, "dejavu.tar.gz");
    await download(url, tarball);

    // Best-effort checksum verification; a real mismatch aborts.
    try {
      const line = await downloadText(url + ".sha256");
      const expected = line.trim().split(/\s+/)[0];
      const actual = sha256(tarball);
      if (expected && actual !== expected) {
        throw new Error(
          "checksum mismatch (expected " + expected + ", got " + actual + ")"
        );
      }
    } catch (e) {
      if (/checksum mismatch/.test(e.message)) throw e;
      // .sha256 unreachable: proceed without verification.
    }

    execFileSync("tar", ["-xzf", tarball, "-C", tmpDir]);
    const extracted = path.join(tmpDir, "dejavu");
    if (!fs.existsSync(extracted)) {
      throw new Error("release archive did not contain 'dejavu'");
    }
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.copyFileSync(extracted, dest);
    fs.chmodSync(dest, 0o755);
    return dest;
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

module.exports = { ensureBinary, binaryPath, target, tarballUrl };
