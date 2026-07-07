//! `[intercept] extra` — user-added commands get shims and generic reduction.

mod common;

use common::{dejavu_cmd, path_with, write_exec};
use std::path::Path;

fn write_global_config(home: &Path, body: &str) {
    let dir = home.join(".config/dejavu");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), body).unwrap();
}

fn run(home: &Path, fake: &Path, proj: &Path, script: &str) -> String {
    let out = dejavu_cmd()
        .current_dir(proj)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", path_with(fake))
        .args(["start", "--", "sh", "-c", script])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn extra_command_is_shimmed_reduced_and_swept_on_removal() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "mytool",
        "#!/bin/sh\ni=1\nwhile [ \"$i\" -le 200 ]; do echo \"test case $i stable output padding line\"; i=$((i+1)); done\nexit 0\n",
    );
    write_global_config(home.path(), "[intercept]\nextra = [\"mytool\"]\n");

    // 1. The custom command is intercepted: 2nd identical run is deduplicated.
    run(home.path(), fake.path(), proj.path(), "mytool run");
    let second = run(home.path(), fake.path(), proj.path(), "mytool run");
    assert!(second.contains("unchanged"), "output: {second}");
    assert!(second.contains("mytool run"), "output: {second}");

    // The shim file exists in the session shim dir.
    let shim = find_shim(home.path(), "mytool").expect("mytool shim generated");
    assert!(shim.exists());

    // 2. Removing it from config sweeps the shim on the next start.
    write_global_config(home.path(), "[intercept]\nextra = []\n");
    run(home.path(), fake.path(), proj.path(), "true");
    assert!(!shim.exists(), "stale mytool shim should be removed");
    // Builtin shims are untouched by the sweep.
    assert!(find_shim(home.path(), "git").is_some());
}

#[test]
fn vitest_is_a_builtin_shim_with_run_reduced() {
    // vitest/jest are builtins now: shimmed by default (no `extra` needed),
    // `vitest run` reduced, bare `vitest` (watch default) passthrough.
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "vitest",
        "#!/bin/sh\ni=1\nwhile [ \"$i\" -le 200 ]; do echo \"test case $i stable output padding line\"; i=$((i+1)); done\nexit 0\n",
    );

    run(home.path(), fake.path(), proj.path(), "vitest run");
    let second = run(home.path(), fake.path(), proj.path(), "vitest run");
    assert!(second.contains("unchanged"), "output: {second}");
    assert!(find_shim(home.path(), "vitest").is_some());
    assert!(find_shim(home.path(), "jest").is_some());

    // Bare `vitest` would enter watch mode: never reduced, raw both times.
    run(home.path(), fake.path(), proj.path(), "vitest");
    let bare = run(home.path(), fake.path(), proj.path(), "vitest");
    assert!(!bare.contains("unchanged"), "output: {bare}");
    assert!(bare.contains("test case 200"), "output: {bare}");
}

fn find_shim(home: &Path, name: &str) -> Option<std::path::PathBuf> {
    // Cache root differs per platform (XDG vs ~/Library/Caches).
    for root in [
        home.join(".cache/dejavu"),
        home.join("Library/Caches/dejavu"),
    ] {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("shims/bin").join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}
