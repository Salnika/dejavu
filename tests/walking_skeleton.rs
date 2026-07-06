//! M2 acceptance: activation, shim resolution, exit-code preservation,
//! recursion guard, and the disabled passthrough (spec §26.1/§26.2).

mod common;

use common::{dejavu_cmd, path_with, write_exec};
use predicates::prelude::*;

const FAKE_PNPM: &str = "#!/bin/sh\n\
echo hello-from-real-pnpm\n\
git status >/dev/null 2>&1\n\
exit ${FAKE_EXIT:-0}\n";

const FAKE_GIT: &str = "#!/bin/sh\n\
DIR=\"$PWD\"\n\
while [ \"$#\" -gt 0 ]; do case \"$1\" in -C) DIR=\"$2\"; shift 2;; -c) shift 2;; --) shift; break;; -*) shift;; *) break;; esac; done\n\
sub=\"${1:-}\"; [ \"$#\" -gt 0 ] && shift\n\
case \"$sub\" in\n\
  rev-parse) [ \"${1:-}\" = \"--show-toplevel\" ] && { echo \"$DIR\"; exit 0; }; exit 128;;\n\
  status) echo clean; exit 0;;\n\
  commit) echo committed; exit 0;;\n\
  *) exit 0;; esac\n";

#[test]
fn activation_shim_resolution_and_exit_one() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", FAKE_PNPM);
    write_exec(fake.path(), "git", FAKE_GIT);

    dejavu_cmd()
        .current_dir(proj.path())
        .env("HOME", home.path())
        .env("PATH", path_with(fake.path()))
        .env("FAKE_EXIT", "1")
        .args([
            "start",
            "--",
            "sh",
            "-c",
            "echo active=$DEJAVU_ACTIVE; command -v pnpm; pnpm test",
        ])
        .assert()
        .failure()
        .code(1) // sh's exit == pnpm test's exit == 1
        .stdout(predicate::str::contains("active=1"))
        .stdout(predicate::str::contains("/shims/bin/pnpm"))
        .stdout(predicate::str::contains("hello-from-real-pnpm"));
}

#[test]
fn exit_zero_preserved() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", FAKE_PNPM);
    write_exec(fake.path(), "git", FAKE_GIT);

    dejavu_cmd()
        .current_dir(proj.path())
        .env("HOME", home.path())
        .env("PATH", path_with(fake.path()))
        .env("FAKE_EXIT", "0")
        .args(["start", "--", "sh", "-c", "pnpm test"])
        .assert()
        .success();
}

#[test]
fn missing_real_binary_is_127() {
    let home = tempfile::tempdir().unwrap();
    let shimdir = tempfile::tempdir().unwrap();

    dejavu_cmd()
        .env("HOME", home.path())
        .env("PATH", shimdir.path()) // nothing real behind the shim
        .env("DEJAVU_SHIM_DIR", shimdir.path())
        .args(["run", "--shim-name", "pnpm", "--", "test"])
        .assert()
        .code(127)
        .stderr(predicate::str::contains("command not found"));
}

#[test]
fn disabled_passes_through_without_capture() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let shimdir = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", FAKE_PNPM);
    write_exec(fake.path(), "git", FAKE_GIT);

    dejavu_cmd()
        .env("HOME", home.path())
        .env("PATH", path_with(fake.path()))
        .env("DEJAVU_SHIM_DIR", shimdir.path())
        .env("DEJAVU_DISABLED", "1")
        .env("FAKE_EXIT", "0")
        .args(["run", "--shim-name", "pnpm", "--", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello-from-real-pnpm"));
}

#[test]
fn dejavu_off_passes_through_without_capture() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let shimdir = tempfile::tempdir().unwrap();
    write_exec(fake.path(), "pnpm", FAKE_PNPM);
    write_exec(fake.path(), "git", FAKE_GIT);

    dejavu_cmd()
        .env("HOME", home.path())
        .env("PATH", path_with(fake.path()))
        .env("DEJAVU_SHIM_DIR", shimdir.path())
        .env("DEJAVU", "off")
        .env("FAKE_EXIT", "0")
        .args(["run", "--shim-name", "pnpm", "--", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello-from-real-pnpm"));
}
