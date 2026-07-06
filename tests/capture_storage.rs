//! M3 acceptance: redaction on disk and in emitted output, and `show`.

mod common;

use common::{dejavu_cmd, find_ext, path_with, write_exec};
use predicates::prelude::*;

#[test]
fn secrets_redacted_on_disk_and_in_emitted_output() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "pnpm",
        "#!/bin/sh\necho AWS_SECRET_ACCESS_KEY=leakedsecret123abc\necho ok\nexit 0\n",
    );

    dejavu_cmd()
        .current_dir(proj.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("PATH", path_with(fake.path()))
        .args(["start", "--", "sh", "-c", "pnpm test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<REDACTED_SECRET>"))
        .stdout(predicate::str::contains("leakedsecret123abc").not());

    // The stored stdout log must also be redacted.
    let log = find_ext(home.path(), "stdout").expect("a .stdout log should exist");
    let contents = std::fs::read_to_string(&log).unwrap();
    assert!(contents.contains("<REDACTED_SECRET>"));
    assert!(!contents.contains("leakedsecret123abc"));
}

#[test]
fn show_latest_reports_run_metadata() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", "#!/bin/sh\necho building\nexit 0\n");

    // Record a run.
    dejavu_cmd()
        .current_dir(proj.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("PATH", path_with(fake.path()))
        .args(["start", "--", "sh", "-c", "pnpm build"])
        .assert()
        .success();

    // `show latest` from the same repo dir.
    dejavu_cmd()
        .current_dir(proj.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["show", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Command: pnpm build"))
        .stdout(predicate::str::contains("Exit code: 0"));
}
