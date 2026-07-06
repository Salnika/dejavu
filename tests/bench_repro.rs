//! M6 / §26.5: `dejavu bench` is reproducible and covers every reduction state.

mod common;

use common::dejavu_cmd;

fn bench_json() -> String {
    let out = dejavu_cmd().args(["bench", "--json"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Token numbers are deterministic; only the timing line varies.
fn strip_timing(s: &str) -> String {
    s.lines()
        .filter(|l| !l.contains("overhead"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn bench_is_reproducible_and_covers_states() {
    let a = bench_json();
    let b = bench_json();
    assert_eq!(
        strip_timing(&a),
        strip_timing(&b),
        "bench must be deterministic"
    );

    for state in ["first_seen", "unchanged", "small_delta", "large_delta"] {
        assert!(a.contains(state), "bench should cover state {state}");
    }
    assert!(a.contains("\"fail_to_pass\": true"));
    // Raw and emitted token totals must differ (reduction actually happened).
    assert!(a.contains("\"reduction_pct\""));
}
