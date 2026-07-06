//! M5 / §26.4: git read-only subcommands are optimized; mutating ones pass
//! through untouched.

mod common;

use assert_cmd::Command;
use common::{dejavu_cmd, path_with, write_exec};
use predicates::prelude::*;
use std::path::Path;

const FAKE_GIT: &str = "#!/bin/sh\n\
DIR=\"$PWD\"\n\
while [ \"$#\" -gt 0 ]; do case \"$1\" in -C) DIR=\"$2\"; shift 2;; -c) shift 2;; --) shift; break;; -*) shift;; *) break;; esac; done\n\
sub=\"${1:-}\"; [ \"$#\" -gt 0 ] && shift\n\
case \"$sub\" in\n\
  rev-parse) [ \"${1:-}\" = \"--show-toplevel\" ] && { echo \"$DIR\"; exit 0; }; exit 128;;\n\
  diff) printf 'diff --git a/x b/x\\n--- a/x\\n+++ b/x\\n@@ -1 +1 @@\\n-a\\n+b\\n'; exit 0;;\n\
  commit) echo 'MUTATING-COMMIT-RAN'; exit 0;;\n\
  *) exit 0;; esac\n";

fn agent(home: &Path, fake: &Path, proj: &Path, script: &str) -> Command {
    let mut cmd = dejavu_cmd();
    cmd.current_dir(proj)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("PATH", path_with(fake))
        .args(["start", "--", "sh", "-c", script]);
    cmd
}

#[test]
fn git_diff_optimized_commit_passthrough() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "git", FAKE_GIT);

    // git diff, run twice -> the 2nd run is reduced (unchanged summary).
    agent(home.path(), fake.path(), proj.path(), "git diff")
        .assert()
        .success();
    agent(home.path(), fake.path(), proj.path(), "git diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("git diff"))
        .stdout(predicate::str::contains("Suppressed"));

    // git commit -> raw output, never an envelope.
    agent(home.path(), fake.path(), proj.path(), "git commit -m x")
        .assert()
        .success()
        .stdout(predicate::str::contains("MUTATING-COMMIT-RAN"))
        .stdout(predicate::str::contains("dejavu:").not())
        .stdout(predicate::str::contains("Suppressed").not());
}
