//! Output reduction. The `reduce()` facade normalizes, hashes, finds a
//! comparable prior run, classifies, and builds the compact output the agent
//! sees — without writing anything (the runtime persists logs + the record).

pub mod classify;
pub mod compare;
pub mod envelope;
pub mod generic;
pub mod normalize;
pub mod parsers;
pub mod redact;

pub use classify::Classification;

use crate::config::Config;
use crate::store::{Db, RunRecord};
use crate::util::{estimate_tokens, sha256_hex, short_id};
use envelope::Envelope;

pub struct ReduceInput<'a> {
    pub run_id: &'a str,
    pub created_at: &'a str,
    pub repo_root: &'a str,
    pub cwd: &'a str,
    pub shim_name: &'a str,
    pub command_original: &'a str,
    pub command_family: &'a str,
    pub command_key: &'a str,
    pub exit_code: i32,
    pub git_head: Option<&'a str>,
    pub git_worktree_hash: Option<&'a str>,
    pub redacted_stdout: &'a [u8],
    pub redacted_stderr: &'a [u8],
}

pub struct ReduceOutput {
    pub classification: Classification,
    pub normalized: String,
    pub normalized_hash: String,
    pub comparison_base_run_id: Option<String>,
    pub comparison_result: String,
    pub summary: Option<String>,
    pub emit_stdout: Vec<u8>,
    pub emit_stderr: Vec<u8>,
    pub estimated_raw_tokens: i64,
    pub estimated_emitted_tokens: i64,
    pub estimated_saved_tokens: i64,
}

/// Reduce captured (already-redacted) output into the compact form + metadata.
pub fn reduce(db: &Db, cfg: &Config, input: &ReduceInput) -> anyhow::Result<ReduceOutput> {
    let combined_raw = combined(input.redacted_stdout, input.redacted_stderr);
    let raw_tokens = estimate_tokens(&combined_raw) as i64;

    let normalized = normalize::normalize(&combined_raw);
    let normalized_hash = sha256_hex(normalized.as_bytes());

    // Hybrid comparability (decision #3): match on repo/cwd/family/key only.
    let prior = db.find_comparable_prior(
        input.repo_root,
        input.cwd,
        input.command_family,
        input.command_key,
        input.created_at,
    )?;
    let prior_normalized = prior
        .as_ref()
        .and_then(|p| p.normalized_path.as_ref())
        .and_then(|path| std::fs::read_to_string(path).ok());

    let classification = classify_state(
        cfg,
        prior.as_ref(),
        &normalized,
        &normalized_hash,
        &prior_normalized,
    );

    let emit = build_emit(
        cfg,
        input,
        &normalized,
        prior.as_ref(),
        prior_normalized.as_deref(),
        classification,
        raw_tokens,
    );

    let comparison_base_run_id = match classification {
        Classification::FirstSeen => None,
        _ => prior.as_ref().map(|p| p.id.clone()),
    };

    Ok(ReduceOutput {
        classification,
        normalized,
        normalized_hash,
        comparison_base_run_id,
        comparison_result: classification.as_str().to_string(),
        summary: emit.summary,
        emit_stdout: emit.stdout,
        emit_stderr: emit.stderr,
        estimated_raw_tokens: raw_tokens,
        estimated_emitted_tokens: emit.emitted_tokens,
        estimated_saved_tokens: emit.saved_tokens,
    })
}

fn classify_state(
    cfg: &Config,
    prior: Option<&RunRecord>,
    normalized: &str,
    normalized_hash: &str,
    prior_normalized: &Option<String>,
) -> Classification {
    let (Some(prior), Some(prior_norm)) = (prior, prior_normalized) else {
        return Classification::FirstSeen;
    };
    if prior.normalized_hash.as_deref() == Some(normalized_hash) {
        return Classification::Unchanged;
    }
    let stats = compare::diff_stats(prior_norm, normalized);
    if stats.changed_lines <= cfg.small_delta_max_changed_lines
        && stats.ratio <= cfg.small_delta_max_changed_ratio
    {
        Classification::SmallDelta
    } else {
        Classification::LargeDelta
    }
}

struct Emit {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    emitted_tokens: i64,
    saved_tokens: i64,
    summary: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn build_emit(
    cfg: &Config,
    input: &ReduceInput,
    normalized: &str,
    prior: Option<&RunRecord>,
    prior_normalized: Option<&str>,
    classification: Classification,
    raw_tokens: i64,
) -> Emit {
    use Classification::*;

    // Below the reduction threshold, emit the raw output with no envelope — for
    // ANY classification. Wrapping a tiny output in a "dejavu: unchanged since…"
    // envelope would emit more tokens than it saves and, worse, would hand
    // corrupted output to any program that parses it (shell prompts, IDE SCM).
    if raw_tokens < cfg.min_raw_tokens_to_reduce as i64 {
        return Emit {
            emitted_tokens: raw_tokens,
            saved_tokens: 0,
            stdout: input.redacted_stdout.to_vec(),
            stderr: input.redacted_stderr.to_vec(),
            summary: generic::summarize(normalized, input.exit_code),
        };
    }

    let prev_short = prior.map(|p| short_id(&p.id).to_string());
    let run_short = short_id(input.run_id).to_string();
    let max_lines = match classification {
        SmallDelta => cfg.max_emitted_lines_small_delta,
        LargeDelta => cfg.max_emitted_lines_large_delta,
        _ => cfg.max_emitted_lines_first_seen,
    };

    // Try a specialized reducer; fall back to the generic one.
    let special = parsers::dispatch(
        input.command_family,
        input.shim_name,
        input.command_key,
        normalized,
        prior_normalized,
        classification,
        input.exit_code,
        prev_short.as_deref(),
        max_lines,
    );

    let (mut status, mut body, full_label, summary) = match special {
        Some(sp) => (sp.status, sp.body, sp.full_label, Some(sp.summary)),
        None => {
            let body = match classification {
                SmallDelta => compare::unified_diff(
                    prior_normalized.unwrap_or(""),
                    normalized,
                    3,
                    cfg.max_emitted_lines_small_delta,
                ),
                Unchanged => prior
                    .and_then(|p| p.summary.clone())
                    .unwrap_or_else(|| generic::extract(normalized, max_lines, input.exit_code)),
                FirstSeen | LargeDelta => generic::extract(normalized, max_lines, input.exit_code),
            };
            let status = generic_status(classification, input.exit_code, prev_short.as_deref());
            (
                status,
                body,
                "Full output",
                generic::summarize(normalized, input.exit_code),
            )
        }
    };

    // An `unchanged` run replays the stored summary: cap it — the agent has
    // already seen the full version, a reminder needs only the head.
    const UNCHANGED_REPLAY_MAX_LINES: usize = 10;
    if classification == Unchanged {
        let n = body.lines().count();
        if n > UNCHANGED_REPLAY_MAX_LINES {
            let kept: Vec<&str> = body.lines().take(UNCHANGED_REPLAY_MAX_LINES).collect();
            body = format!(
                "{}\n... (+{} more identical lines)",
                kept.join("\n"),
                n - UNCHANGED_REPLAY_MAX_LINES
            );
        }
    }

    // Exit-code change emphasis (spec §12.6); fail -> pass overrides the status
    // and shrinks the body to what matters: what USED to fail, plus the tail.
    let exit_changed = prior
        .map(|p| p.exit_code as i32 != input.exit_code)
        .unwrap_or(false);
    let headline = exit_changed.then(|| {
        let p = prior.unwrap();
        format!("Exit code changed: {} -> {}", p.exit_code, input.exit_code)
    });
    if let Some(p) = prior {
        if p.exit_code != 0 && input.exit_code == 0 {
            status = "command now passes.".to_string();
            if let Some(prev_sum) = p.summary.as_deref() {
                let capped: Vec<&str> = prev_sum.lines().take(8).collect();
                if !capped.is_empty() {
                    body = format!(
                        "Previously failing summary:\n{}\n\n{}",
                        capped.join("\n"),
                        body
                    );
                }
            }
        }
    }

    let note = git_note(
        prior,
        input.git_head,
        input.git_worktree_hash,
        classification,
    );

    let body_tokens = estimate_tokens(&body) as i64;
    let suppressed = (raw_tokens - body_tokens).max(0);

    let rendered = envelope::render(&Envelope {
        status: &status,
        command: input.command_original,
        exit_code: input.exit_code,
        headline: headline.as_deref(),
        note: note.as_deref(),
        body: &body,
        suppressed_tokens: suppressed,
        run_id_short: &run_short,
        prev_id_short: prev_short.as_deref(),
        full_label,
    });

    let emitted_tokens = estimate_tokens(&rendered) as i64;
    let saved_tokens = (raw_tokens - emitted_tokens).max(0);
    Emit {
        stdout: rendered.into_bytes(),
        stderr: Vec::new(),
        emitted_tokens,
        saved_tokens,
        summary,
    }
}

fn generic_status(classification: Classification, exit_code: i32, prev: Option<&str>) -> String {
    use Classification::*;
    let prev = prev.unwrap_or("?");
    match classification {
        FirstSeen => {
            if exit_code != 0 {
                "command failed.".to_string()
            } else {
                "command output.".to_string()
            }
        }
        Unchanged => format!("output unchanged since run {prev}."),
        SmallDelta => format!("output changed slightly since run {prev}."),
        LargeDelta => format!("output changed significantly since run {prev}."),
    }
}

/// Git-state annotation (decision #3).
fn git_note(
    prior: Option<&RunRecord>,
    curr_head: Option<&str>,
    curr_worktree: Option<&str>,
    classification: Classification,
) -> Option<String> {
    let prior = prior?;
    let git_differs = prior.git_head.as_deref() != curr_head
        || prior.git_worktree_hash.as_deref() != curr_worktree;
    match classification {
        Classification::SmallDelta | Classification::LargeDelta => Some(if git_differs {
            "Note: output changed across code changes (git state differs).".to_string()
        } else {
            "Note: output changed with no code change — possibly flaky/nondeterministic."
                .to_string()
        }),
        Classification::Unchanged if git_differs => {
            Some("Note: output unchanged across code changes.".to_string())
        }
        _ => None,
    }
}

fn combined(stdout: &[u8], stderr: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(stdout).into_owned();
    if !stderr.is_empty() {
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(&String::from_utf8_lossy(stderr));
    }
    s
}
