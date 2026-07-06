//! CLI dispatch and the shared application context.

pub mod dejavu_cli;

use crate::commands::render;
use crate::config::Config;
use crate::env::{self, AgentEnv};
use crate::paths::CacheLayout;
use crate::{repo, state};
use dejavu_cli::{Cli, DejavuCmd};
use std::path::PathBuf;

/// Resolved context every command needs: the repo, its cache layout, the
/// effective config, and the active session id (if any).
pub struct AppCtx {
    pub cwd: PathBuf,
    pub repo_root: PathBuf,
    pub layout: CacheLayout,
    pub config: Config,
    pub session_id: Option<String>,
}

impl AppCtx {
    pub fn resolve() -> anyhow::Result<AppCtx> {
        let cwd = std::env::current_dir()?;
        let (repo_root, layout) = if let Some(agent) = AgentEnv::from_current() {
            // Inside a session: trust the launcher's repo + cache dir.
            (
                agent.repo_root.clone(),
                CacheLayout::from_dir(agent.cache_dir.clone()),
            )
        } else {
            let root = repo::detect_repo_root(&cwd);
            let layout = CacheLayout::for_repo(&root)?;
            (root, layout)
        };
        let config = Config::load(&repo_root)?;
        let session_id = std::env::var(env::SESSION_ID).ok();
        Ok(AppCtx {
            cwd,
            repo_root,
            layout,
            config,
            session_id,
        })
    }

    pub fn repo_root_str(&self) -> String {
        self.repo_root.to_string_lossy().into_owned()
    }
}

/// Dispatch a parsed `dejavu` command to its handler. Returns the process exit
/// code. Note: the `Run` path is handled directly in `bin/dejavu.rs` (it needs
/// the exit-code guard) and never reaches here.
pub fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        DejavuCmd::Start { command } => crate::agent::launch(command),
        DejavuCmd::Init => crate::commands::init::run(),
        DejavuCmd::Shellenv => crate::commands::shellenv::run(),
        DejavuCmd::Stats { json, all, public } => crate::commands::stats::run(json, all, public),
        DejavuCmd::Repos { json, all } => crate::commands::repos::run(json, all),
        DejavuCmd::Report { redact } => crate::commands::stats::report(redact),
        DejavuCmd::Enable => set_repo_disabled(false),
        DejavuCmd::Disable => set_repo_disabled(true),
        DejavuCmd::Run { shim_name, args } => crate::runtime::run_shim(&shim_name, &args),
        DejavuCmd::Show {
            target,
            stdout,
            stderr,
            normalized,
        } => crate::commands::show::run(&target, stdout, stderr, normalized),
        DejavuCmd::Grep {
            target,
            pattern,
            normalized,
        } => crate::commands::grep::run(&target, &pattern, normalized),
        DejavuCmd::Doctor { json } => crate::commands::doctor::run(json),
        DejavuCmd::Bench { scenario, json } => crate::commands::bench::run(scenario, json),
        DejavuCmd::Clean { older_than, all } => crate::commands::clean::run(older_than, all),
        DejavuCmd::Uninstall => crate::commands::clean::uninstall(),
    }
}

fn set_repo_disabled(disabled: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    ctx.layout.ensure_dirs()?;
    let mut st = state::load(&ctx.layout);
    st.disabled = disabled;
    state::save(&ctx.layout, &st)?;
    render::title(if disabled {
        "Dejavu disabled"
    } else {
        "Dejavu enabled"
    });
    render::kv(&[("Repo", ctx.repo_root.display().to_string())]);
    Ok(0)
}
