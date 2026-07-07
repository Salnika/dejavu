//! `DEJAVU_*` environment variable names and the ambient session view.

use std::path::PathBuf;

pub const ACTIVE: &str = "DEJAVU_ACTIVE";
pub const BIN: &str = "DEJAVU_BIN";
pub const REPO_ROOT: &str = "DEJAVU_REPO_ROOT";
pub const CACHE_DIR: &str = "DEJAVU_CACHE_DIR";
pub const SESSION_ID: &str = "DEJAVU_SESSION_ID";
pub const SHIM_DIR: &str = "DEJAVU_SHIM_DIR";
pub const MODE: &str = "DEJAVU";
pub const DISABLED: &str = "DEJAVU_DISABLED";
/// Force reduction even outside a session / agent context (`DEJAVU_FORCE=1`).
pub const FORCE: &str = "DEJAVU_FORCE";
/// The user's original ZDOTDIR (or $HOME), sourced by the wrapper zdot files.
pub const ORIG_ZDOTDIR: &str = "DEJAVU_ORIG_ZDOTDIR";

/// Agents that capture command output through a **pipe** rather than a pty:
/// Claude Code, the Codex CLI, Cursor's agent. Each of these vars is set only
/// in the shell the agent drives for its command tool, and the reader on the
/// other end of the pipe IS the agent — so reduction does not require stdout to
/// be a terminal (with a pipe reader, it never will be).
pub const PIPE_AGENT_MARKERS: &[&str] = &["CLAUDECODE", "CODEX_SANDBOX", "CURSOR_AGENT"];

/// Agents that run commands in a real **pty** (VS Code Copilot). `AI_AGENT` is
/// the cross-vendor convention; VS Code sets `AI_AGENT` + `COPILOT_AGENT` in
/// agent-tool terminals only, never in user terminals. For these markers
/// reduction additionally requires stdout to be a terminal — the pty is the
/// tell that an agent, not a pipe-reading parser (`$(git …)`, IDE SCM), is on
/// the far end.
pub const PTY_AGENT_MARKERS: &[&str] = &["AI_AGENT", "COPILOT_AGENT"];

/// True when the user asks Dejavu to force passthrough everywhere.
pub fn is_disabled() -> bool {
    let legacy_flag = std::env::var_os(DISABLED).is_some_and(|v| v == "1");
    let mode_off = std::env::var_os(MODE).is_some_and(|v| {
        matches!(
            v.to_string_lossy().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "disabled"
        )
    });
    legacy_flag || mode_off
}

/// True when running inside a `dejavu start` session.
pub fn is_active() -> bool {
    std::env::var_os(ACTIVE).is_some_and(|v| v == "1")
}

/// True when `DEJAVU_FORCE=1` overrides the global-mode reduction gates.
pub fn is_forced() -> bool {
    std::env::var_os(FORCE).is_some_and(|v| v == "1")
}

fn any_marker_set(markers: &[&str]) -> bool {
    markers
        .iter()
        .any(|m| std::env::var_os(m).is_some_and(|v| !v.is_empty()))
}

/// True when a pipe-capturing agent (Claude Code, Codex CLI, Cursor) is driving
/// this shell.
pub fn pipe_agent_marker_present() -> bool {
    any_marker_set(PIPE_AGENT_MARKERS)
}

/// True when a pty-based agent (VS Code Copilot) is driving this shell.
pub fn pty_agent_marker_present() -> bool {
    any_marker_set(PTY_AGENT_MARKERS)
}

/// Whether output reduction is allowed for this invocation.
///
/// - Inside a `dejavu start` session: always (the agent captures via pipes).
/// - `DEJAVU_FORCE=1`: always (explicit human election).
/// - Global activation (shims on PATH, no session):
///   - a **pipe-capturing** agent marker (Claude Code, Codex CLI, Cursor) →
///     always. The agent itself reads the pipe, so stdout is never a terminal;
///     the marker is set only in the shell the agent drives. This matches how
///     `dejavu start` already behaves for the same agents.
///   - a **pty-based** agent marker (Copilot) → only when stdout is a terminal.
///     Copilot runs commands in a real pty, while parsers (`$(git …)`,
///     pipelines, the IDE SCM) read through pipes — so the tty gate keeps a
///     reduced envelope from ever landing in front of a parser.
pub fn reduction_allowed(stdout_is_tty: bool) -> bool {
    is_active()
        || is_forced()
        || pipe_agent_marker_present()
        || (pty_agent_marker_present() && stdout_is_tty)
}

/// The ambient session, reconstructed from `DEJAVU_*` env vars. `None` when a
/// shim is somehow invoked outside a session.
#[derive(Debug, Clone)]
pub struct AgentEnv {
    pub bin: PathBuf,
    pub repo_root: PathBuf,
    pub cache_dir: PathBuf,
    pub session_id: String,
    pub shim_dir: PathBuf,
}

impl AgentEnv {
    pub fn from_current() -> Option<AgentEnv> {
        Some(AgentEnv {
            bin: std::env::var_os(BIN)?.into(),
            repo_root: std::env::var_os(REPO_ROOT)?.into(),
            cache_dir: std::env::var_os(CACHE_DIR)?.into(),
            session_id: std::env::var_os(SESSION_ID)?.to_string_lossy().into_owned(),
            shim_dir: std::env::var_os(SHIM_DIR)?.into(),
        })
    }
}
