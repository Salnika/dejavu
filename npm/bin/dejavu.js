#!/usr/bin/env node
"use strict";

// Transparent launcher for the real `dejavu` binary. It writes NOTHING to
// stdout — Dejavu's whole job is clean command output for the agent, so the
// wrapper must never add noise. Diagnostics go to stderr only.

const fs = require("fs");
const { spawnSync } = require("child_process");
const { binaryPath, ensureBinary } = require("../lib/download");

async function main() {
  let bin = binaryPath();
  if (!fs.existsSync(bin)) {
    // postinstall was skipped (e.g. `npm install --ignore-scripts`): fetch now.
    process.stderr.write("[dejavu] downloading binary...\n");
    bin = await ensureBinary();
  }

  const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    process.stderr.write("[dejavu] " + result.error.message + "\n");
    process.exit(1);
  }
  // Preserve the real exit code; signal death -> non-zero.
  process.exit(typeof result.status === "number" ? result.status : 1);
}

main().catch((err) => {
  process.stderr.write("[dejavu] " + err.message + "\n");
  process.exit(1);
});
