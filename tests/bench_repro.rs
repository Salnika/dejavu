//! M6 / §26.5: `dejavu bench` is reproducible, covers every reduction state,
//! and `--check` acts as a regression gate.

mod common;

use common::dejavu_cmd;

fn bench_json(extra: &[&str]) -> (String, bool) {
    let out = dejavu_cmd()
        .args(["bench", "--json"])
        .args(extra)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.success(),
    )
}

/// Token numbers are deterministic; only timing varies.
fn strip_timing(s: &str) -> String {
    s.lines()
        .filter(|l| !l.contains("latency") && !l.contains("p50") && !l.contains("p95"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn bench_is_reproducible_and_covers_states() {
    // --check also skips the latency micro-bench, keeping this test fast.
    let (a, ok_a) = bench_json(&["--check"]);
    let (b, _) = bench_json(&["--check"]);
    assert!(ok_a, "bench --check must pass: {a}");
    assert_eq!(
        strip_timing(&a),
        strip_timing(&b),
        "bench must be deterministic"
    );

    let v: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert!(v["check"]["passed"].as_bool().unwrap());
    assert_eq!(v["check"]["violations"].as_array().unwrap().len(), 0);

    // The js loop covers every reduction state + fail->pass.
    let scenarios = v["scenarios"].as_array().unwrap();
    let js = scenarios
        .iter()
        .find(|s| s["name"] == "js-validation-loop")
        .unwrap();
    let states: Vec<&str> = js["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    for state in ["first_seen", "unchanged", "small_delta", "large_delta"] {
        assert!(states.contains(&state), "js loop must cover {state}");
    }
    assert!(js["fail_to_pass"].as_bool().unwrap());

    // Machine-readable git forms are never reduced (safety scenario).
    let safety = scenarios
        .iter()
        .find(|s| s["name"] == "machine-safety")
        .unwrap();
    assert!(safety["all_passthrough"].as_bool().unwrap());
    assert_eq!(safety["saved_tokens"], 0);

    // Real reduction happened overall.
    assert!(v["totals"]["saved_tokens"].as_i64().unwrap() > 0);
}

#[test]
fn bench_scenario_filter_and_unknown_name() {
    let out = dejavu_cmd()
        .args(["bench", "--json", "--scenario", "git-workflow"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["scenarios"].as_array().unwrap().len(), 1);

    let out = dejavu_cmd()
        .args(["bench", "--scenario", "nope"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
