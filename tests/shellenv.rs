//! `dejavu shellenv` — global activation without a `dejavu start` session.

mod common;

use common::{dejavu_cmd, write_exec};
use std::process::Command as StdCommand;

#[test]
fn shellenv_generates_global_shims_and_activates_interception() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "pnpm",
        "#!/bin/sh\necho global shim ok\nexit 0\n",
    );

    // 1. shellenv prints the idempotent PATH guard and creates the shims.
    let out = dejavu_cmd()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .arg("shellenv")
        .output()
        .unwrap();
    assert!(out.status.success());
    let line = String::from_utf8_lossy(&out.stdout);
    assert!(line.contains("export PATH="));
    assert!(line.contains("shims/bin"));

    let shim_dir = home
        .path()
        .join(".cache/dejavu/shims/bin")
        .canonicalize()
        .or_else(|_| {
            // macOS cache layout.
            home.path()
                .join("Library/Caches/dejavu/shims/bin")
                .canonicalize()
        })
        .unwrap();
    assert!(shim_dir.join("pnpm").is_file());
    assert!(shim_dir.join("git").is_file());

    // 2. Eval the line in a shell with NO DEJAVU_* variable: interception must
    // work purely from PATH (repo context rebuilt from cwd, no recursion).
    let dejavu_bin_dir = assert_cmd::cargo::cargo_bin("dejavu");
    let base_path = format!(
        "{}:{}:/usr/bin:/bin",
        fake.path().display(),
        dejavu_bin_dir.parent().unwrap().display(),
    );
    let script = format!(
        "{line}\ncd {} && pnpm test && pnpm test",
        proj.path().display()
    );
    let out = StdCommand::new("/bin/sh")
        .arg("-c")
        .arg(&script)
        .env_clear()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("PATH", &base_path)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    // First run passes through raw (small output); second is deduplicated.
    assert!(text.contains("global shim ok"));
    assert!(text.contains("unchanged"));
}

#[test]
fn shellenv_is_idempotent_when_evaled_twice() {
    let home = tempfile::tempdir().unwrap();
    let out = dejavu_cmd()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .arg("shellenv")
        .output()
        .unwrap();
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // Eval twice, count occurrences of the shim dir in PATH: must be 1.
    let script = format!("{line}\n{line}\nprintf %s \"$PATH\"");
    let out = StdCommand::new("/bin/sh")
        .arg("-c")
        .arg(&script)
        .env_clear()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    let path = String::from_utf8_lossy(&out.stdout);
    assert_eq!(path.matches("dejavu/shims/bin").count(), 1, "PATH: {path}");
}
