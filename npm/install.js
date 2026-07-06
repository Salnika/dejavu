#!/usr/bin/env node
"use strict";

// postinstall: fetch the platform binary eagerly so the first real run is
// instant. Best-effort — if it fails (offline, no matching asset yet), the
// launcher self-heals on first run, so we never fail `npm install`.

const { ensureBinary } = require("./lib/download");

ensureBinary()
  .then((p) => process.stderr.write("[dejavu] ready: " + p + "\n"))
  .catch((err) => {
    process.stderr.write("[dejavu] could not download the binary now: " + err.message + "\n");
    process.stderr.write("[dejavu] it will be fetched automatically on first run.\n");
    process.exit(0);
  });
