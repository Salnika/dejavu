//! `dejavu shellenv` — global activation without a `dejavu start` session.

mod common;

use common::{dejavu_cmd, write_exec};
use std::process::Command as StdCommand;

#[test]
fn shellenv_generates_global_shims_and_activates_interception() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    // Large, deterministic output so reduction actually engages (a tiny output
    // passes through raw and would never show the "unchanged" envelope).
    write_exec(
        fake.path(),
        "pnpm",
        "#!/bin/sh\ni=1\nwhile [ \"$i\" -le 200 ]; do echo \"validation line $i stable output padding content\"; i=$((i+1)); done\nexit 0\n",
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
    // Self-shim: the envelope's `dejavu show …` follow-up must work everywhere.
    assert!(shim_dir.join("dejavu").is_file());
    let v = StdCommand::new(shim_dir.join("dejavu"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&v.stdout).starts_with("dejavu "));

    // 2. Eval the line in a shell with NO DEJAVU_* session env: interception
    // works purely from PATH (repo context rebuilt from cwd, no recursion).
    // Output capture here is a PIPE, so global-mode reduction requires an
    // explicit DEJAVU_FORCE=1 (agents in pty terminals need no override).
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
        .env("DEJAVU_FORCE", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    // The second identical run is deduplicated into an "unchanged" envelope.
    assert!(text.contains("pnpm test"), "output: {text}");
    assert!(text.contains("unchanged"), "output: {text}");
}

#[test]
fn global_mode_without_agent_context_never_reduces() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "pnpm",
        "#!/bin/sh\ni=1\nwhile [ \"$i\" -le 200 ]; do echo \"validation line $i stable output padding content\"; i=$((i+1)); done\nexit 0\n",
    );

    let line = String::from_utf8_lossy(
        &dejavu_cmd()
            .env("HOME", home.path())
            .env("XDG_CACHE_HOME", home.path().join(".cache"))
            .arg("shellenv")
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();

    let dejavu_bin_dir = assert_cmd::cargo::cargo_bin("dejavu");
    let base_path = format!(
        "{}:{}:/usr/bin:/bin",
        fake.path().display(),
        dejavu_bin_dir.parent().unwrap().display(),
    );
    // No DEJAVU_* session, no agent marker, stdout is a pipe: a user terminal
    // or an output-parsing program. Both runs must be RAW — never an envelope.
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
    assert!(!text.contains("dejavu:"), "output was reduced: {text}");
    assert!(!text.contains("unchanged"), "output was reduced: {text}");
    // Both raw runs present: 2 × 200 lines.
    assert_eq!(text.matches("validation line 200").count(), 2);

    // Fast path: nothing is recorded either — the user's own terminal history
    // stays out of the cache (no per-repo runs.sqlite was even created).
    for root in [
        home.path().join(".cache/dejavu"),
        home.path().join("Library/Caches/dejavu"),
    ] {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let db = entry.path().join("runs.sqlite");
            assert!(
                !db.exists(),
                "no-agent passthrough must not record runs: {}",
                db.display()
            );
        }
    }

    // Exit codes are preserved on the fast path too: intercept a failing
    // command (regenerate shims after adding it to the config) and check the
    // code survives the shim -> dejavu -> real chain untouched.
    write_exec(fake.path(), "failing", "#!/bin/sh\necho boom\nexit 42\n");
    let mut cfg_home = home.path().join(".config/dejavu");
    std::fs::create_dir_all(&cfg_home).unwrap();
    cfg_home.push("config.toml");
    std::fs::write(&cfg_home, "[intercept]\nextra = [\"failing\"]\n").unwrap();
    dejavu_cmd()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .arg("shellenv")
        .assert()
        .success();
    let out = StdCommand::new("/bin/sh")
        .arg("-c")
        .arg(format!("{line}\ncd {} && failing", proj.path().display()))
        .env_clear()
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .env("PATH", &base_path)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(42),
        "fast path must preserve exit codes"
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("boom"));
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
