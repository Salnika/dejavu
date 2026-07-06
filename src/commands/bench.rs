//! `dejavu bench` — a reproducible, LLM-free benchmark (spec §20). It drives the
//! real reduction pipeline over a scripted scenario of fixture outputs in an
//! isolated temp cache, and reports the token reduction.

use crate::config::Config;
use crate::paths::CacheLayout;
use crate::reduce::{self, Classification, ReduceInput};
use crate::store::{write_logs, Db, RunRecord};
use crate::util;
use std::time::Instant;

struct Step {
    family: &'static str,
    shim: &'static str,
    command_key: &'static str,
    command_original: String,
    stdout: Vec<u8>,
    exit_code: i32,
}

/// A synthetic test-runner output: 120 cases, optionally failing at `fail_at`.
/// `seed` varies only volatile tokens (time/duration) that normalization strips.
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

fn scenario() -> Vec<Step> {
    let cmd = |s: &str| s.to_string();
    let v = |shim, key, orig: &str, out: (Vec<u8>, i32)| Step {
        family: "validation",
        shim,
        command_key: key,
        command_original: cmd(orig),
        stdout: out.0,
        exit_code: out.1,
    };
    vec![
        // pnpm test: first_seen -> unchanged -> small_delta -> large_delta -> fail->pass
        v(
            "pnpm",
            "validation:pnpm:test",
            "pnpm test",
            test_output(Some(7), 11),
        ),
        v(
            "pnpm",
            "validation:pnpm:test",
            "pnpm test",
            test_output(Some(7), 22),
        ),
        v(
            "pnpm",
            "validation:pnpm:test",
            "pnpm test",
            test_output(Some(8), 33),
        ),
        v("pnpm", "validation:pnpm:test", "pnpm test", alt_output()),
        v(
            "pnpm",
            "validation:pnpm:test",
            "pnpm test",
            test_output(None, 55),
        ),
    ]
}

#[derive(Default)]
struct Totals {
    raw_bytes: i64,
    emitted_bytes: i64,
    raw_tokens: i64,
    emitted_tokens: i64,
    overhead_ms_sum: u128,
    steps: usize,
    states: Vec<String>,
    fail_to_pass: bool,
}

pub fn run(scenario_name: Option<String>, json: bool) -> anyhow::Result<i32> {
    let name = scenario_name.unwrap_or_else(|| "js-validation-loop".to_string());
    if name != "js-validation-loop" {
        anyhow::bail!("unknown scenario `{name}` (available: js-validation-loop)");
    }

    // Isolated, reproducible cache.
    let tmp = std::env::temp_dir().join(format!("dejavu-bench-{}", util::new_id()));
    let layout = CacheLayout::from_dir(tmp.clone());
    layout.ensure_dirs()?;
    let db = Db::open(&layout.db())?;
    let cfg = Config::default();

    let repo_root = "/bench/js-validation-loop";
    let cwd = repo_root;
    let mut totals = Totals::default();
    let mut prev_exit: Option<i32> = None;

    for (i, step) in scenario().into_iter().enumerate() {
        let run_id = util::new_id();
        // Synthetic, monotonically-increasing timestamps → deterministic order.
        let created_at = format!("2020-01-01T00:00:00.{:06}Z", i + 1);

        let start = Instant::now();
        let reduced = reduce::reduce(
            &db,
            &cfg,
            &ReduceInput {
                run_id: &run_id,
                created_at: &created_at,
                repo_root,
                cwd,
                shim_name: step.shim,
                command_original: &step.command_original,
                command_family: step.family,
                command_key: step.command_key,
                exit_code: step.exit_code,
                git_head: None,
                git_worktree_hash: None,
                redacted_stdout: &step.stdout,
                redacted_stderr: &[],
            },
        )?;
        let overhead = start.elapsed().as_millis();

        let stored = write_logs(
            &layout,
            &run_id,
            &step.stdout,
            &[],
            Some(&reduced.normalized),
            cfg.max_raw_output_bytes as usize,
        )?;

        let raw_bytes = step.stdout.len() as i64;
        let record = RunRecord {
            id: run_id,
            session_id: "bench".to_string(),
            created_at,
            repo_root: repo_root.to_string(),
            cwd: cwd.to_string(),
            shim_name: step.shim.to_string(),
            argv_json: "[]".to_string(),
            command_original: step.command_original.clone(),
            command_family: step.family.to_string(),
            command_key: step.command_key.to_string(),
            classification: reduced.classification.as_str().to_string(),
            exit_code: step.exit_code as i64,
            duration_ms: 0,
            overhead_ms: overhead as i64,
            stdout_path: stored
                .stdout_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            stderr_path: None,
            normalized_path: stored
                .normalized_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            raw_stdout_bytes: raw_bytes,
            raw_stderr_bytes: 0,
            raw_total_bytes: raw_bytes,
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
        };
        db.insert_run(&record)?;

        totals.raw_bytes += raw_bytes;
        totals.emitted_bytes += (reduced.emit_stdout.len() + reduced.emit_stderr.len()) as i64;
        totals.raw_tokens += reduced.estimated_raw_tokens;
        totals.emitted_tokens += reduced.estimated_emitted_tokens;
        totals.overhead_ms_sum += overhead;
        totals.steps += 1;
        let state = reduced.classification.as_str().to_string();
        if !totals.states.contains(&state) {
            totals.states.push(state);
        }
        if reduced.classification != Classification::FirstSeen
            && prev_exit == Some(1)
            && step.exit_code == 0
        {
            totals.fail_to_pass = true;
        }
        prev_exit = Some(step.exit_code);
    }

    // Best-effort cleanup of the temp cache.
    let _ = std::fs::remove_dir_all(&tmp);

    report(&name, &totals, json)
}

fn report(name: &str, t: &Totals, json: bool) -> anyhow::Result<i32> {
    let saved = (t.raw_tokens - t.emitted_tokens).max(0);
    let reduction = if t.raw_tokens > 0 {
        saved as f64 / t.raw_tokens as f64 * 100.0
    } else {
        0.0
    };
    let avg_overhead = if t.steps > 0 {
        t.overhead_ms_sum as f64 / t.steps as f64
    } else {
        0.0
    };

    if json {
        let obj = serde_json::json!({
            "scenario": name,
            "without_dejavu": { "raw_bytes": t.raw_bytes, "estimated_tokens": t.raw_tokens },
            "with_dejavu": { "emitted_bytes": t.emitted_bytes, "estimated_tokens": t.emitted_tokens },
            "estimated_savings": { "tokens": saved, "reduction_pct": reduction },
            "average_overhead_ms": avg_overhead,
            "states_covered": t.states,
            "fail_to_pass": t.fail_to_pass,
            "quality": { "full_output_requested_pct": 0.0, "internal_fallback_pct": 0.0 },
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(0);
    }

    use super::fmt_int;
    println!("Dejavu benchmark\n");
    println!("Scenario: {name}\n");
    println!("Without Dejavu:");
    println!("- raw output: {} bytes", fmt_int(t.raw_bytes));
    println!("- estimated tokens: {}", fmt_int(t.raw_tokens));
    println!("\nWith Dejavu:");
    println!("- emitted output: {} bytes", fmt_int(t.emitted_bytes));
    println!("- estimated tokens: {}", fmt_int(t.emitted_tokens));
    println!("\nEstimated savings:");
    println!("- {} tokens", fmt_int(saved));
    println!("- {reduction:.1}% reduction");
    println!("\nAverage overhead:");
    println!("- {avg_overhead:.0}ms per command");
    println!("\nStates covered: {}", t.states.join(", "));
    println!("Fail -> pass observed: {}", t.fail_to_pass);
    println!("\nQuality:");
    println!("- full output requested: 0.0%");
    println!("- internal fallback: 0.0%");
    Ok(0)
}
