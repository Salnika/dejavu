//! M4 acceptance: unchanged / small_delta reduction states (spec §26.3).

mod common;

use assert_cmd::Command;
use common::{dejavu_cmd, path_with, write_exec};
use predicates::prelude::*;
use std::path::Path;

// ~120 lines so raw output exceeds `min_raw_tokens_to_reduce` and the envelope
// (not raw passthrough) is used. `VARIANT=changed` flips one line.
const BIG_PNPM: &str = "#!/bin/sh\n\
i=1\n\
while [ $i -le 120 ]; do\n\
  line=\"PASS tests/unit/module_$i/case check ok result stable padding\"\n\
  [ \"$VARIANT\" = changed ] && [ $i -eq 60 ] && line=\"FAIL tests/unit/module_$i/case boom mismatch here now\"\n\
  echo \"$line\"\n\
  i=$((i+1))\n\
done\n\
exit 0\n";

fn agent(home: &Path, fake: &Path, proj: &Path, variant: &str) -> Command {
    let mut cmd = dejavu_cmd();
    cmd.current_dir(proj)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("PATH", path_with(fake))
        .env("VARIANT", variant)
        .args(["start", "--", "sh", "-c", "pnpm test"]);
    cmd
}

#[test]
fn identical_output_is_unchanged_then_change_is_small_delta() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", BIG_PNPM);

    // Run 1: first_seen.
    agent(home.path(), fake.path(), proj.path(), "base")
        .assert()
        .success();

    // Run 2: identical normalized output -> unchanged.
    agent(home.path(), fake.path(), proj.path(), "base")
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged since run"))
        .stdout(predicate::str::contains("Suppressed"));

    // Run 3: one line now fails -> small_delta; the jest reducer surfaces the
    // newly failing file.
    agent(home.path(), fake.path(), proj.path(), "changed")
        .assert()
        .success()
        .stdout(predicate::str::contains("changed since run"))
        .stdout(predicate::str::contains("module_60"));
}
