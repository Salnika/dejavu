//! `dejavu repos` lists tracked active repos and can include disabled ones.

mod common;

use common::{dejavu_cmd, path_with, write_exec};
use std::path::Path;

fn run_in_project(home: &Path, fake: &Path, proj: &Path) {
    dejavu_cmd()
        .current_dir(proj)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("PATH", path_with(fake))
        .args(["start", "--", "sh", "-c", "pnpm test"])
        .assert()
        .success();
}

#[test]
fn repos_lists_active_repos_and_can_include_disabled_ones() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let proj_a = tempfile::tempdir().unwrap();
    let proj_b = tempfile::tempdir().unwrap();
    write_exec(
        fake.path(),
        "pnpm",
        "#!/bin/sh\necho test output ok\nexit 0\n",
    );

    run_in_project(home.path(), fake.path(), proj_a.path());
    run_in_project(home.path(), fake.path(), proj_b.path());

    dejavu_cmd()
        .current_dir(proj_b.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["disable"])
        .assert()
        .success();

    let active = dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["repos"])
        .output()
        .unwrap();
    assert!(active.status.success());
    let active_text = String::from_utf8_lossy(&active.stdout);
    assert!(active_text.contains("active repos"));
    assert!(active_text.contains(proj_a.path().to_string_lossy().as_ref()));
    assert!(!active_text.contains(proj_b.path().to_string_lossy().as_ref()));

    let all = dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["repos", "--all", "--json"])
        .output()
        .unwrap();
    assert!(all.status.success());
    let v: serde_json::Value = serde_json::from_slice(&all.stdout).unwrap();
    assert_eq!(v["repos_tracked"], 2);
    let repos = v["repos"].as_array().unwrap();
    assert_eq!(repos.len(), 2);
    let canon_a = std::fs::canonicalize(proj_a.path()).unwrap();
    let canon_b = std::fs::canonicalize(proj_b.path()).unwrap();
    assert!(repos.iter().any(|r| {
        r["repo"]
            .as_str()
            .is_some_and(|repo| Path::new(repo) == canon_a)
            && r["status"].as_str() == Some("active")
    }));
    assert!(repos.iter().any(|r| {
        r["repo"]
            .as_str()
            .is_some_and(|repo| Path::new(repo) == canon_b)
            && r["status"].as_str() == Some("disabled")
    }));
}

#[test]
fn repos_empty_cache_reports_no_repos() {
    let home = tempfile::tempdir().unwrap();
    dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["repos"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No repos found."));
}
