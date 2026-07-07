//! `dejavu bench` — a reproducible, LLM-free benchmark suite (spec §20).
//!
//! Each scenario drives the REAL `classify()` + `reduce()` chain over scripted
//! outputs in an isolated temp cache, then checks expectations (state
//! coverage, minimum reduction, passthrough safety). `--check` turns those
//! expectations into a regression gate (non-zero exit) suitable for CI. A
//! latency micro-bench additionally spawns the actual binary end-to-end and
//! reports p50/p95 per-command overhead (reported, never gated: machine-
//! dependent).

use crate::config::Config;
use crate::exec::classify::classify;
use crate::exec::ExecMode;
use crate::paths::CacheLayout;
use crate::reduce::{self, Classification, ReduceInput};
use crate::store::{write_logs, Db, RunRecord};
use crate::util;
use std::time::Instant;

use super::{fmt_int, render};

struct Step {
    shim: &'static str,
    args: Vec<String>,
    stdout: Vec<u8>,
    exit_code: i32,
}

fn step(shim: &'static str, args: &[&str], out: (Vec<u8>, i32)) -> Step {
    Step {
        shim,
        args: args.iter().map(|s| s.to_string()).collect(),
        stdout: out.0,
        exit_code: out.1,
    }
}

/// What a scenario must deliver; violated expectations fail `--check`.
struct Expect {
    /// Classifications that must all appear.
    states: &'static [&'static str],
    /// Minimum token reduction over the whole scenario (optimized runs).
    min_reduction_pct: f64,
    /// A fail -> pass transition must be observed.
    fail_to_pass: bool,
    /// Every step must be passthrough with byte-identical output (safety).
    all_passthrough: bool,
}

struct Scenario {
    name: &'static str,
    steps: Vec<Step>,
    expect: Expect,
}

// --- synthetic outputs (deterministic; volatile tokens vary per seed) -------

/// A test-runner output: 120 cases, optionally failing at `fail_at`.
fn test_output(fail_at: Option<usize>, seed: u32) -> (Vec<u8>, i32) {
    let mut s = format!("Test run started at 12:00:{:02} in {}ms\n", seed % 60, seed);
    let mut exit = 0;
    for i in 1..=120 {
        if Some(i) == fail_at {
            s.push_str(&format!(
                "FAIL tests/case_{i}.test.ts expected 200 received 500\n"
            ));
            exit = 1;
        } else {
            s.push_str(&format!("PASS tests/case_{i}.test.ts ok\n"));
        }
    }
    s.push_str("Tests: 120 total\n");
    (s.into_bytes(), exit)
}

/// A wholly different large output (drives large_delta).
fn alt_output() -> (Vec<u8>, i32) {
    let mut s = String::from("Rebuilding project graph...\n");
    for i in 1..=120 {
        s.push_str(&format!(
            "FAIL tests/integration/scenario_{i}.spec.ts timeout after wait\n"
        ));
    }
    s.push_str("Tests: 120 failed\n");
    (s.into_bytes(), 1)
}

/// A unified diff over `n` files, with `variant` altering the hunk contents.
fn diff_output(n: usize, variant: u32) -> (Vec<u8>, i32) {
    let mut s = String::new();
    for i in 1..=n {
        s.push_str(&format!(
            "diff --git a/src/mod_{i}.rs b/src/mod_{i}.rs\n--- a/src/mod_{i}.rs\n+++ b/src/mod_{i}.rs\n@@ -1,4 +1,4 @@\n"
        ));
        for l in 1..=4 {
            s.push_str(&format!("-old line {l} of module {i} rev {variant}\n"));
            s.push_str(&format!("+new line {l} of module {i} rev {variant}\n"));
        }
    }
    (s.into_bytes(), 0)
}

/// rg-style matches across files; `extra` appends additional matches.
fn search_output(extra: usize) -> (Vec<u8>, i32) {
    let mut s = String::new();
    for i in 1..=180 {
        s.push_str(&format!(
            "src/module_{}/handler.rs:{}:    session.createSession(request, opts)\n",
            i % 12,
            10 + i
        ));
    }
    for j in 0..extra {
        s.push_str(&format!(
            "src/newfeature/cache_{j}.rs:7:    token.createSession(scope)\n"
        ));
    }
    (s.into_bytes(), 0)
}

/// A very large build log (~40k lines) to exercise caps and truncation.
fn huge_output(seed: u32) -> (Vec<u8>, i32) {
    let mut s = String::with_capacity(2_500_000);
    s.push_str(&format!("build started at 09:0{}:00\n", seed % 10));
    for i in 1..=40_000 {
        s.push_str(&format!(
            "[{i:>6}] compiling unit module_{} object emitted ok\n",
            i % 900
        ));
    }
    s.push_str("build finished: 40000 units\n");
    (s.into_bytes(), 0)
}

fn porcelain_output() -> (Vec<u8>, i32) {
    let mut s = String::new();
    for i in 1..=300 {
        s.push_str(&format!(" M src/generated/file_{i}.ts\n"));
    }
    (s.into_bytes(), 0)
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "js-validation-loop",
            steps: vec![
                step("pnpm", &["test"], test_output(Some(7), 11)),
                step("pnpm", &["test"], test_output(Some(7), 22)),
                step("pnpm", &["test"], test_output(Some(8), 33)),
                step("pnpm", &["test"], alt_output()),
                step("pnpm", &["test"], test_output(None, 55)),
            ],
            expect: Expect {
                states: &["first_seen", "unchanged", "small_delta", "large_delta"],
                min_reduction_pct: 50.0,
                fail_to_pass: true,
                all_passthrough: false,
            },
        },
        Scenario {
            name: "git-workflow",
            steps: vec![
                step("git", &["diff"], diff_output(40, 1)),
                step("git", &["diff"], diff_output(40, 1)),
                step("git", &["diff"], diff_output(40, 2)),
            ],
            expect: Expect {
                states: &["first_seen", "unchanged"],
                min_reduction_pct: 60.0,
                fail_to_pass: false,
                all_passthrough: false,
            },
        },
        Scenario {
            name: "search-loop",
            steps: vec![
                step("rg", &["createSession", "src"], search_output(0)),
                step("rg", &["createSession", "src"], search_output(0)),
                step("rg", &["createSession", "src"], search_output(5)),
            ],
            expect: Expect {
                states: &["first_seen", "unchanged"],
                min_reduction_pct: 50.0,
                fail_to_pass: false,
                all_passthrough: false,
            },
        },
        Scenario {
            name: "large-output",
            steps: vec![
                step("pnpm", &["run", "build"], huge_output(1)),
                step("pnpm", &["run", "build"], huge_output(2)),
            ],
            expect: Expect {
                states: &["first_seen", "unchanged"],
                min_reduction_pct: 95.0,
                fail_to_pass: false,
                all_passthrough: false,
            },
        },
        // SAFETY: machine-readable git forms must never be reduced, even when
        // large — prompts, IDE SCM, hooks and $(git …) parse them.
        Scenario {
            name: "machine-safety",
            steps: vec![
                step("git", &["status", "--porcelain"], porcelain_output()),
                step("git", &["diff", "--name-only"], porcelain_output()),
                step(
                    "git",
                    &["log", "--oneline", "..@{upstream}"],
                    porcelain_output(),
                ),
            ],
            expect: Expect {
                states: &[],
                min_reduction_pct: 0.0,
                fail_to_pass: false,
                all_passthrough: true,
            },
        },
    ]
}

// --- execution ---------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct ScenarioResult {
    name: String,
    steps: usize,
    raw_tokens: i64,
    emitted_tokens: i64,
    saved_tokens: i64,
    reduction_pct: f64,
    states: Vec<String>,
    fail_to_pass: bool,
    all_passthrough: bool,
    max_emitted_chars: usize,
    violations: Vec<String>,
}

fn run_scenario(sc: &Scenario) -> anyhow::Result<ScenarioResult> {
    let tmp = std::env::temp_dir().join(format!("dejavu-bench-{}", util::new_id()));
    let layout = CacheLayout::from_dir(tmp.clone());
    layout.ensure_dirs()?;
    let db = Db::open(&layout.db())?;
    let cfg = Config::default();

    let repo_root = format!("/bench/{}", sc.name);
    let mut r = ScenarioResult {
        name: sc.name.to_string(),
        all_passthrough: true,
        ..Default::default()
    };
    let mut prev_exit: Option<i32> = None;

    for (i, s) in sc.steps.iter().enumerate() {
        let created_at = format!("2020-01-01T00:00:00.{:06}Z", i + 1);
        let run_id = util::new_id();

        // Real classification: family, key, and the optimize/passthrough call.
        let classified = classify(s.shim, &s.args, &cfg, false, false, false);
        let raw_tokens = util::estimate_tokens(&String::from_utf8_lossy(&s.stdout)) as i64;

        match &classified.mode {
            ExecMode::Passthrough(_) => {
                // Raw bytes go straight through.
                r.raw_tokens += raw_tokens;
                r.emitted_tokens += raw_tokens;
                r.states.push("passthrough".into());
                r.max_emitted_chars = r.max_emitted_chars.max(s.stdout.len());
            }
            ExecMode::Optimize {
                family,
                command_key,
            } => {
                r.all_passthrough = false;
                let reduced = reduce::reduce(
                    &db,
                    &cfg,
                    &ReduceInput {
                        run_id: &run_id,
                        created_at: &created_at,
                        repo_root: &repo_root,
                        cwd: &repo_root,
                        shim_name: s.shim,
                        command_original: &classified.command_original,
                        command_family: family.as_str(),
                        command_key,
                        exit_code: s.exit_code,
                        git_head: None,
                        git_worktree_hash: None,
                        redacted_stdout: &s.stdout,
                        redacted_stderr: &[],
                    },
                )?;
                let stored = write_logs(
                    &layout,
                    &run_id,
                    &s.stdout,
                    &[],
                    Some(&reduced.normalized),
                    cfg.max_raw_output_bytes as usize,
                )?;
                db.insert_run(&bench_record(
                    &run_id,
                    &created_at,
                    &repo_root,
                    s,
                    family.as_str(),
                    command_key,
                    &classified.command_original,
                    &reduced,
                    &stored,
                ))?;

                r.raw_tokens += reduced.estimated_raw_tokens;
                r.emitted_tokens += reduced.estimated_emitted_tokens;
                let state = reduced.classification.as_str().to_string();
                if !r.states.contains(&state) {
                    r.states.push(state);
                }
                r.max_emitted_chars = r.max_emitted_chars.max(reduced.emit_stdout.len());
                if reduced.classification != Classification::FirstSeen
                    && prev_exit == Some(1)
                    && s.exit_code == 0
                {
                    r.fail_to_pass = true;
                }
            }
        }
        prev_exit = Some(s.exit_code);
        r.steps += 1;
    }

    r.saved_tokens = (r.raw_tokens - r.emitted_tokens).max(0);
    r.reduction_pct = if r.raw_tokens > 0 {
        r.saved_tokens as f64 / r.raw_tokens as f64 * 100.0
    } else {
        0.0
    };

    // Expectations -> violations.
    for want in sc.expect.states {
        if !r.states.iter().any(|s| s == want) {
            r.violations.push(format!("missing state `{want}`"));
        }
    }
    if r.reduction_pct + 1e-9 < sc.expect.min_reduction_pct {
        r.violations.push(format!(
            "reduction {:.1}% below the {:.0}% floor",
            r.reduction_pct, sc.expect.min_reduction_pct
        ));
    }
    if sc.expect.fail_to_pass && !r.fail_to_pass {
        r.violations
            .push("fail->pass transition not observed".into());
    }
    if sc.expect.all_passthrough && !r.all_passthrough {
        r.violations
            .push("a machine-form step was NOT passthrough (safety)".into());
    }
    if !sc.expect.all_passthrough && r.max_emitted_chars > 15_000 {
        r.violations.push(format!(
            "an emitted output exceeded the 15K inline cap ({} chars)",
            r.max_emitted_chars
        ));
    }

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(r)
}

#[allow(clippy::too_many_arguments)]
fn bench_record(
    run_id: &str,
    created_at: &str,
    repo_root: &str,
    s: &Step,
    family: &str,
    command_key: &str,
    command_original: &str,
    reduced: &reduce::ReduceOutput,
    stored: &crate::store::StoredLogs,
) -> RunRecord {
    RunRecord {
        id: run_id.to_string(),
        session_id: "bench".to_string(),
        created_at: created_at.to_string(),
        repo_root: repo_root.to_string(),
        cwd: repo_root.to_string(),
        shim_name: s.shim.to_string(),
        argv_json: serde_json::to_string(&s.args).unwrap_or_else(|_| "[]".into()),
        command_original: command_original.to_string(),
        command_family: family.to_string(),
        command_key: command_key.to_string(),
        classification: reduced.classification.as_str().to_string(),
        exit_code: s.exit_code as i64,
        duration_ms: 0,
        overhead_ms: 0,
        stdout_path: stored
            .stdout_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        stderr_path: None,
        normalized_path: stored
            .normalized_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        raw_stdout_bytes: s.stdout.len() as i64,
        raw_stderr_bytes: 0,
        raw_total_bytes: s.stdout.len() as i64,
        emitted_bytes: (reduced.emit_stdout.len() + reduced.emit_stderr.len()) as i64,
        estimated_raw_tokens: reduced.estimated_raw_tokens,
        estimated_emitted_tokens: reduced.estimated_emitted_tokens,
        estimated_saved_tokens: reduced.estimated_saved_tokens,
        normalized_hash: Some(reduced.normalized_hash.clone()),
        stdout_hash: None,
        stderr_hash: None,
        git_head: None,
        git_worktree_hash: None,
        comparison_base_run_id: reduced.comparison_base_run_id.clone(),
        comparison_result: reduced.comparison_result.clone(),
        summary: reduced.summary.clone(),
        full_output_requested: 0,
        internal_error: None,
    }
}

// --- latency micro-bench ------------------------------------------------------

/// End-to-end per-command overhead: spawn the real binary through the full
/// shim pipeline (classify, capture, reduce, record) against a trivial fake
/// tool, and report p50/p95. Machine-dependent — reported, never gated.
fn latency_bench(iterations: usize) -> Option<(f64, f64)> {
    use std::os::unix::fs::PermissionsExt;

    let exe = std::env::current_exe().ok()?;
    let tmp = std::env::temp_dir().join(format!("dejavu-bench-lat-{}", util::new_id()));
    let home = tmp.join("home");
    let fixture = tmp.join("bin");
    let proj = tmp.join("proj");
    for d in [&home, &fixture, &proj] {
        std::fs::create_dir_all(d).ok()?;
    }
    let fake = fixture.join("tree");
    std::fs::write(&fake, "#!/bin/sh\necho .\necho ./src\necho ./tests\n").ok()?;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).ok()?;

    let path = format!("{}:/usr/bin:/bin", fixture.display());
    let mut times: Vec<f64> = Vec::with_capacity(iterations);
    // One warmup iteration first: it pays the one-time cache + DB creation,
    // which would otherwise dominate p95.
    for i in 0..=iterations {
        let start = Instant::now();
        let out = std::process::Command::new(&exe)
            .args(["run", "--shim-name", "tree", "--", "."])
            .current_dir(&proj)
            .env_clear()
            .env("HOME", &home)
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("PATH", &path)
            .env("DEJAVU_FORCE", "1")
            .output();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        match out {
            Ok(o) if o.status.success() => {
                if i > 0 {
                    times.push(elapsed);
                }
            }
            _ => break,
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if times.len() < iterations {
        return None;
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = times[times.len() / 2];
    let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
    Some((p50, p95))
}

// --- entry point ----------------------------------------------------------------

pub fn run(scenario_name: Option<String>, json: bool, check: bool) -> anyhow::Result<i32> {
    let all = scenarios();
    let selected: Vec<&Scenario> = match scenario_name.as_deref() {
        None | Some("all") => all.iter().collect(),
        Some(name) => {
            let found: Vec<&Scenario> = all.iter().filter(|s| s.name == name).collect();
            if found.is_empty() {
                let names: Vec<&str> = all.iter().map(|s| s.name).collect();
                anyhow::bail!(
                    "unknown scenario `{name}` (available: {})",
                    names.join(", ")
                );
            }
            found
        }
    };
    let run_latency = scenario_name.is_none() && !check;

    let mut results = Vec::new();
    for sc in &selected {
        results.push(run_scenario(sc)?);
    }
    let latency = if run_latency { latency_bench(15) } else { None };

    let total_raw: i64 = results.iter().map(|r| r.raw_tokens).sum();
    let total_emitted: i64 = results.iter().map(|r| r.emitted_tokens).sum();
    let total_saved = (total_raw - total_emitted).max(0);
    let total_reduction = if total_raw > 0 {
        total_saved as f64 / total_raw as f64 * 100.0
    } else {
        0.0
    };
    let violations: Vec<String> = results
        .iter()
        .flat_map(|r| r.violations.iter().map(move |v| format!("{}: {v}", r.name)))
        .collect();
    let passed = violations.is_empty();

    if json {
        let obj = serde_json::json!({
            "scenarios": results,
            "totals": {
                "raw_tokens": total_raw,
                "emitted_tokens": total_emitted,
                "saved_tokens": total_saved,
                "reduction_pct": total_reduction,
            },
            "latency_ms": latency.map(|(p50, p95)| serde_json::json!({"p50": p50, "p95": p95})),
            "check": { "passed": passed, "violations": violations },
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(if check && !passed { 2 } else { 0 });
    }

    render::title("Dejavu benchmark");
    render::section("Scenarios");
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|r| {
            vec![
                if r.violations.is_empty() {
                    "ok".to_string()
                } else {
                    "fail".to_string()
                },
                r.name.clone(),
                fmt_int(r.raw_tokens),
                fmt_int(r.emitted_tokens),
                format!("{:.1}%", r.reduction_pct),
                if r.all_passthrough {
                    "passthrough (safety)".to_string()
                } else {
                    r.states.join(", ")
                },
            ]
        })
        .collect();
    render::table_styled(
        &["", "Scenario", "Raw tok", "Emitted", "Reduction", "States"],
        &rows,
        &[render::Style::Status, render::Style::Cyan],
    );

    render::section("Totals");
    render::kv(&[
        ("Estimated tokens saved", fmt_int(total_saved)),
        ("Scenarios", results.len().to_string()),
    ]);
    println!();
    println!("{}", render::meter("Overall reduction", total_reduction));

    if let Some((p50, p95)) = latency {
        render::section("Latency (end-to-end per command, this machine)");
        render::kv(&[
            ("p50", format!("{p50:.0}ms")),
            ("p95", format!("{p95:.0}ms")),
        ]);
    }

    if !violations.is_empty() {
        render::section("Check failures");
        for v in &violations {
            println!("  {}", render::paint(render::Style::Red, v));
        }
    } else if check {
        println!();
        println!(
            "{}",
            render::paint(render::Style::Green, "check: all expectations met")
        );
    }

    Ok(if check && !passed { 2 } else { 0 })
}
