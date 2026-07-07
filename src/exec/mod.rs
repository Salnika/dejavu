//! Execution core: shim generation, PATH handling, anti-recursion real-binary
//! resolution, process spawning, and classification/passthrough policy.

pub mod classify;
pub mod command_key;
pub mod interactive;
pub mod path;
pub mod resolve;
pub mod shim;
pub mod spawn;

use std::time::Duration;

/// Command families that Dejavu can optimize (spec §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Validation,
    Search,
    Tree,
    GitReadonly,
    Logs,
}

impl Family {
    pub fn as_str(&self) -> &'static str {
        match self {
            Family::Validation => "validation",
            Family::Search => "search",
            Family::Tree => "tree",
            Family::GitReadonly => "git_readonly",
            Family::Logs => "logs",
        }
    }
}

/// The decision the classifier hands to the runtime.
#[derive(Debug, Clone)]
pub enum ExecMode {
    /// Capture output for reduction.
    Optimize { family: Family, command_key: String },
    /// Run with inherited stdio; no capture, no reduction.
    Passthrough(PassthroughReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassthroughReason {
    Disabled,
    RepoDisabled,
    ConfigExcluded,
    UnknownShim,
    UnsupportedSubcommand,
    ArgsNotWhitelisted,
    Interactive,
    MutatingGit,
    DangerousDocker,
    SideEffecting,
    /// A machine-readable git form (`--porcelain`, `-z`, `@{upstream}`, …) that a
    /// program parses — reducing it would corrupt shell prompts / IDE SCM.
    /// (Outside an agent context entirely, `run_shim` short-circuits to a pure
    /// exec before classification — no reason is ever recorded there.)
    MachineReadable,
}

impl PassthroughReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            PassthroughReason::Disabled => "disabled",
            PassthroughReason::RepoDisabled => "repo_disabled",
            PassthroughReason::ConfigExcluded => "config_excluded",
            PassthroughReason::UnknownShim => "unknown_shim",
            PassthroughReason::UnsupportedSubcommand => "unsupported_subcommand",
            PassthroughReason::ArgsNotWhitelisted => "args_not_whitelisted",
            PassthroughReason::Interactive => "interactive",
            PassthroughReason::MutatingGit => "mutating_git",
            PassthroughReason::DangerousDocker => "dangerous_docker",
            PassthroughReason::SideEffecting => "side_effecting",
            PassthroughReason::MachineReadable => "machine_readable",
        }
    }
}

/// The result of running the real command.
#[derive(Debug, Clone)]
pub struct ExecOutcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Already normalized: `code()` if exited, else `128 + signal`.
    pub exit_code: i32,
    pub duration: Duration,
    /// Whether we captured (Optimize) or passed stdio through (Passthrough).
    pub captured: bool,
    /// Whether the raw output was truncated to the storage cap (spec §21.4).
    pub truncated_raw: bool,
}
