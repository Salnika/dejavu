//! `dejavu stats --all` aggregates across every tracked repo cache.

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
fn stats_all_aggregates_across_repos() {
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

    // --all sees both repos, from anywhere (cwd doesn't matter).
    let out = dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["stats", "--all", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    assert_eq!(v["repos"].as_array().unwrap().len(), 2);
    assert_eq!(v["runs_captured"], 2);
    let repo_names: Vec<&str> = v["repos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["repo"].as_str().unwrap())
        .collect();
    let canon_a = std::fs::canonicalize(proj_a.path()).unwrap();
    let canon_b = std::fs::canonicalize(proj_b.path()).unwrap();
    assert!(repo_names.iter().any(|r| Path::new(r) == canon_a));
    assert!(repo_names.iter().any(|r| Path::new(r) == canon_b));

    // Per-repo stats stay scoped: each project reports only its own run.
    let solo = dejavu_cmd()
        .current_dir(proj_a.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["stats", "--json"])
        .output()
        .unwrap();
    let sv: serde_json::Value = serde_json::from_slice(&solo.stdout).unwrap();
    assert_eq!(sv["runs_captured"], 1);

    let public = dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["stats", "--all", "--public", "--json"])
        .output()
        .unwrap();
    assert!(public.status.success());
    let pv: serde_json::Value = serde_json::from_slice(&public.stdout).unwrap();
    assert_eq!(pv["public"], true);
    assert_eq!(pv["repos"].as_array().unwrap().len(), 0);
    assert!(pv["scope"].as_str().unwrap().contains("2 repos"));
    let public_text = String::from_utf8_lossy(&public.stdout);
    assert!(!public_text.contains(proj_a.path().to_string_lossy().as_ref()));
    assert!(!public_text.contains(proj_b.path().to_string_lossy().as_ref()));

    let report = dejavu_cmd()
        .current_dir(proj_a.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["report", "--redact"])
        .output()
        .unwrap();
    assert!(report.status.success());
    let report_text = String::from_utf8_lossy(&report.stdout);
    assert!(report_text.contains("# Dejavu Report"));
    assert!(report_text.contains("Redacted: yes"));
    assert!(!report_text.contains(proj_a.path().to_string_lossy().as_ref()));
}

#[test]
fn stats_all_empty_cache_reports_zeros() {
    let home = tempfile::tempdir().unwrap();
    dejavu_cmd()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .args(["stats", "--all"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Runs captured: 0"));
}
