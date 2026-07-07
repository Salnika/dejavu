//! The `dejavu run` pipeline: resolve → classify → exec → reduce → emit, with
//! the absolute exit-code guard.
//!
//! Invariant: once the real command has run, its normalized exit code is
//! returned no matter what. All post-exec work runs inside `catch_unwind`; any
//! failure prints the raw output and still exits the real code.

use crate::config::Config;
use crate::env::{self, AgentEnv};
use crate::exec::classify::{classify, Classified};
use crate::exec::spawn::{CommandRunner, RealRunner, SpawnSpec};
use crate::exec::{path as exec_path, resolve, ExecMode, Family, PassthroughReason};
use crate::paths::CacheLayout;
use crate::reduce;
use crate::store::{write_logs, Db, RunRecord, StoredLogs};
use crate::{repo, state, util};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Bytes to write to our own stdout/stderr after reduction.
struct EmitPlan {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl EmitPlan {
    fn raw(outcome: &crate::exec::ExecOutcome) -> EmitPlan {
        EmitPlan {
            stdout: outcome.stdout.clone(),
            stderr: outcome.stderr.clone(),
        }
    }
    fn write(self) {
        let _ = std::io::stdout().write_all(&self.stdout);
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().write_all(&self.stderr);
        let _ = std::io::stderr().flush();
    }
}

/// Resolved per-invocation context.
struct RunCtx {
    repo_root: PathBuf,
    cwd: PathBuf,
    layout: CacheLayout,
    config: Config,
    session_id: String,
}

impl RunCtx {
    fn resolve(cwd: &Path, agent: Option<&AgentEnv>) -> anyhow::Result<RunCtx> {
        let (repo_root, layout, session_id) = if let Some(a) = agent {
            (
                a.repo_root.clone(),
                CacheLayout::from_dir(a.cache_dir.clone()),
                a.session_id.clone(),
            )
        } else {
            let root = repo::detect_repo_root(cwd);
            let layout = CacheLayout::for_repo(&root)?;
            let sid = std::env::var(env::SESSION_ID).unwrap_or_else(|_| "no-session".to_string());
            (root, layout, sid)
        };
        let config = Config::load(&repo_root)?;
        Ok(RunCtx {
            repo_root,
            cwd: cwd.to_path_buf(),
            layout,
            config,
            session_id,
        })
    }
}

pub fn run_shim(shim_name: &str, args: &[String]) -> anyhow::Result<i32> {
    let started = Instant::now();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let agent = AgentEnv::from_current();

    // Where our shim dir and own dir live (for anti-recursion resolution).
    let dejavu_dir = current_exe_dir();
    let shim_dir = agent
        .as_ref()
        .map(|a| a.shim_dir.clone())
        .or_else(|| std::env::var_os(env::SHIM_DIR).map(PathBuf::from))
        .unwrap_or_else(|| dejavu_dir.clone());

    let path_os = std::env::var_os("PATH").unwrap_or_default();

    // Resolve the real binary. Not found → behave like the shell: 127.
    let real = match resolve::resolve_real(
        shim_name,
        &resolve::ResolveEnv {
            path: &path_os,
            shim_dir: &shim_dir,
            dejavu_dir: &dejavu_dir,
        },
    ) {
        Some(p) => p,
        None => {
            eprintln!("{shim_name}: command not found");
            return Ok(127);
        }
    };

    let sanitized_path = exec_path::without_dir(&shim_dir, &path_os);

    // Fast passthrough when globally disabled — no context, no capture.
    if env::is_disabled() {
        return passthrough_exec(&real, args, &cwd, &sanitized_path);
    }

    // Build context; any failure → safe passthrough (never break the command).
    let ctx = match RunCtx::resolve(&cwd, agent.as_ref()) {
        Ok(c) => c,
        Err(_) => return passthrough_exec(&real, args, &cwd, &sanitized_path),
    };

    let repo_disabled = state::is_repo_disabled(&ctx.layout);
    let stdin_tty = crate::exec::interactive::stdin_is_tty();
    let mut classified = classify(
        shim_name,
        args,
        &ctx.config,
        false,
        repo_disabled,
        stdin_tty,
    );

    // Global-activation gate: reduce only for an agent (session, DEJAVU_FORCE,
    // or agent marker + pty). A user terminal or an output-parsing program
    // must always receive the raw output.
    if matches!(classified.mode, ExecMode::Optimize { .. }) {
        use std::io::IsTerminal;
        if !env::reduction_allowed(std::io::stdout().is_terminal()) {
            classified.mode = ExecMode::Passthrough(PassthroughReason::NoAgentContext);
        }
    }

    match &classified.mode {
        ExecMode::Passthrough(reason) => {
            let code = passthrough_exec(&real, args, &cwd, &sanitized_path)?;
            record_passthrough(&ctx, shim_name, args, &classified, *reason, code, started);
            Ok(code)
        }
        ExecMode::Optimize {
            family,
            command_key,
        } => {
            let spec = SpawnSpec {
                program: real.clone(),
                args: args.iter().map(OsString::from).collect(),
                cwd: cwd.clone(),
                env_path: sanitized_path.clone(),
                capture: true,
                inherit_stdin: !stdin_tty,
                capture_limit: Some(ctx.config.max_raw_output_bytes as usize),
            };
            let outcome = match RealRunner.run(&spec) {
                Ok(o) => o,
                // Could not spawn despite resolving — fall back to passthrough.
                Err(_) => return passthrough_exec(&real, args, &cwd, &sanitized_path),
            };

            let real_code = outcome.exit_code;
            let meta = OptimizedMeta {
                shim_name: shim_name.to_string(),
                args: args.to_vec(),
                command_original: classified.command_original.clone(),
                family: *family,
                command_key: command_key.clone(),
            };

            // Exit-code guard: reduction/recording must never change the code.
            let built = std::panic::catch_unwind(AssertUnwindSafe(|| {
                finalize_optimized(&ctx, &meta, &outcome, started)
            }));
            let plan = match built {
                Ok(Ok(plan)) => plan,
                Ok(Err(err)) => {
                    eprintln!("dejavu internal error: {err}");
                    eprintln!("falling back to raw output");
                    EmitPlan::raw(&outcome)
                }
                Err(_) => {
                    eprintln!("dejavu internal error: panic during reduction");
                    eprintln!("falling back to raw output");
                    EmitPlan::raw(&outcome)
                }
            };
            plan.write();
            Ok(real_code)
        }
    }
}

struct OptimizedMeta {
    shim_name: String,
    args: Vec<String>,
    command_original: String,
    family: Family,
    command_key: String,
}

/// M4 reduction: redact, then normalize/compare/classify, store the redacted
/// output + normalized text, record the run, and emit the compact output.
fn finalize_optimized(
    ctx: &RunCtx,
    meta: &OptimizedMeta,
    outcome: &crate::exec::ExecOutcome,
    started: Instant,
) -> anyhow::Result<EmitPlan> {
    let cfg = &ctx.config;
    let run_id = util::new_id();
    let created_at = util::now_rfc3339();

    // Redact BEFORE anything is stored, hashed, or normalized.
    let (red_stdout, red_stderr) = if cfg.redact_secrets {
        (
            reduce::redact::redact_bytes(&outcome.stdout).0,
            reduce::redact::redact_bytes(&outcome.stderr).0,
        )
    } else {
        (outcome.stdout.clone(), outcome.stderr.clone())
    };

    let git_head = repo::git_head(&ctx.repo_root);
    let git_worktree = repo::git_worktree_hash(&ctx.repo_root);
    let repo_root_s = ctx.repo_root.to_string_lossy().into_owned();
    let cwd_s = ctx.cwd.to_string_lossy().into_owned();

    let db = Db::open(&ctx.layout.db())?;
    let reduced = reduce::reduce(
        &db,
        cfg,
        &reduce::ReduceInput {
            run_id: &run_id,
            created_at: &created_at,
            repo_root: &repo_root_s,
            cwd: &cwd_s,
            shim_name: &meta.shim_name,
            command_original: &meta.command_original,
            command_family: meta.family.as_str(),
            command_key: &meta.command_key,
            exit_code: outcome.exit_code,
            git_head: git_head.as_deref(),
            git_worktree_hash: git_worktree.as_deref(),
            redacted_stdout: &red_stdout,
            redacted_stderr: &red_stderr,
        },
    )?;

    // Store logs. Normalized text is always persisted (future comparisons need
    // it) even when raw storage is disabled.
    let stored = if cfg.store_raw_outputs {
        write_logs(
            &ctx.layout,
            &run_id,
            &red_stdout,
            &red_stderr,
            Some(&reduced.normalized),
            cfg.max_raw_output_bytes as usize,
        )?
    } else {
        let mut s = StoredLogs::default();
        let _ = std::fs::create_dir_all(ctx.layout.logs_dir());
        let np = ctx.layout.normalized_log(&run_id);
        if std::fs::write(&np, &reduced.normalized).is_ok() {
            s.normalized_path = Some(np);
        }
        s
    };

    let raw_stdout = outcome.stdout.len() as i64;
    let raw_stderr = outcome.stderr.len() as i64;
    let emitted_bytes = (reduced.emit_stdout.len() + reduced.emit_stderr.len()) as i64;

    let record = RunRecord {
        id: run_id,
        session_id: ctx.session_id.clone(),
        created_at,
        repo_root: repo_root_s,
        cwd: cwd_s,
        shim_name: meta.shim_name.clone(),
        argv_json: serde_json::to_string(&meta.args).unwrap_or_else(|_| "[]".to_string()),
        command_original: meta.command_original.clone(),
        command_family: meta.family.as_str().to_string(),
        command_key: meta.command_key.clone(),
        classification: reduced.classification.as_str().to_string(),
        exit_code: outcome.exit_code as i64,
        duration_ms: outcome.duration.as_millis() as i64,
        overhead_ms: overhead_ms(started, outcome),
        stdout_path: path_str(&stored.stdout_path),
        stderr_path: path_str(&stored.stderr_path),
        normalized_path: path_str(&stored.normalized_path),
        raw_stdout_bytes: raw_stdout,
        raw_stderr_bytes: raw_stderr,
        raw_total_bytes: raw_stdout + raw_stderr,
        emitted_bytes,
        estimated_raw_tokens: reduced.estimated_raw_tokens,
        estimated_emitted_tokens: reduced.estimated_emitted_tokens,
        estimated_saved_tokens: reduced.estimated_saved_tokens,
        normalized_hash: Some(reduced.normalized_hash),
        stdout_hash: Some(util::sha256_hex(&red_stdout)),
        stderr_hash: Some(util::sha256_hex(&red_stderr)),
        git_head,
        git_worktree_hash: git_worktree,
        comparison_base_run_id: reduced.comparison_base_run_id,
        comparison_result: reduced.comparison_result,
        summary: reduced.summary,
        full_output_requested: 0,
        internal_error: None,
    };

    db.insert_run(&record)?;
    let _ = db.accumulate_session_tokens(
        &ctx.session_id,
        reduced.estimated_raw_tokens,
        reduced.estimated_emitted_tokens,
        reduced.estimated_saved_tokens,
    );

    Ok(EmitPlan {
        stdout: reduced.emit_stdout,
        stderr: reduced.emit_stderr,
    })
}

fn path_str(path: &Option<std::path::PathBuf>) -> Option<String> {
    path.as_ref().map(|p| p.to_string_lossy().into_owned())
}

fn record_passthrough(
    ctx: &RunCtx,
    shim_name: &str,
    args: &[String],
    classified: &Classified,
    _reason: PassthroughReason,
    exit_code: i32,
    started: Instant,
) {
    let record = RunRecord {
        id: util::new_id(),
        session_id: ctx.session_id.clone(),
        created_at: util::now_rfc3339(),
        repo_root: ctx.repo_root.to_string_lossy().into_owned(),
        cwd: ctx.cwd.to_string_lossy().into_owned(),
        shim_name: shim_name.to_string(),
        argv_json: serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string()),
        command_original: classified.command_original.clone(),
        command_family: "passthrough".to_string(),
        command_key: format!("passthrough:{shim_name}"),
        classification: "passthrough".to_string(),
        exit_code: exit_code as i64,
        duration_ms: 0,
        overhead_ms: started.elapsed().as_millis() as i64,
        stdout_path: None,
        stderr_path: None,
        normalized_path: None,
        raw_stdout_bytes: 0,
        raw_stderr_bytes: 0,
        raw_total_bytes: 0,
        emitted_bytes: 0,
        estimated_raw_tokens: 0,
        estimated_emitted_tokens: 0,
        estimated_saved_tokens: 0,
        normalized_hash: None,
        stdout_hash: None,
        stderr_hash: None,
        git_head: None,
        git_worktree_hash: None,
        comparison_base_run_id: None,
        comparison_result: "passthrough".to_string(),
        summary: None,
        full_output_requested: 0,
        internal_error: None,
    };
    // Best-effort: a storage failure must never affect a passthrough command.
    let _ = persist(ctx, &record, 0, 0, 0);
}

/// Insert the run and accumulate session totals. Storage failures propagate to
/// the caller, which treats them as a reason to fall back to raw output.
fn persist(
    ctx: &RunCtx,
    record: &RunRecord,
    raw_tokens: i64,
    emitted_tokens: i64,
    saved_tokens: i64,
) -> anyhow::Result<()> {
    let db = Db::open(&ctx.layout.db())?;
    db.insert_run(record)?;
    let _ = db.accumulate_session_tokens(&ctx.session_id, raw_tokens, emitted_tokens, saved_tokens);
    Ok(())
}

fn passthrough_exec(
    program: &Path,
    args: &[String],
    cwd: &Path,
    env_path: &OsStr,
) -> anyhow::Result<i32> {
    let spec = SpawnSpec {
        program: program.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
        cwd: cwd.to_path_buf(),
        env_path: env_path.to_os_string(),
        capture: false,
        inherit_stdin: true,
        capture_limit: None,
    };
    Ok(RealRunner.run(&spec)?.exit_code)
}

fn overhead_ms(started: Instant, outcome: &crate::exec::ExecOutcome) -> i64 {
    let total = started.elapsed().as_millis() as i64;
    let cmd = outcome.duration.as_millis() as i64;
    (total - cmd).max(0)
}

fn current_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}
