//! The `dejavu` CLI command tree.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "dejavu",
    version,
    about = "Stop showing agents the same output twice."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: DejavuCmd,
}

#[derive(Subcommand, Debug)]
pub enum DejavuCmd {
    /// Launch a coding agent (or any command) with Dejavu active.
    ///
    /// Examples: `dejavu start claude`, `dejavu start codex`, `dejavu start -- bash`
    Start {
        /// The command to launch, with its arguments.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            required = true,
            value_name = "COMMAND"
        )]
        command: Vec<String>,
    },

    /// Initialize the cache for the current repo (does not modify the repo).
    Init,

    /// Print the shell line for global activation: eval "$(dejavu shellenv)".
    ///
    /// Put it at the END of ~/.zprofile (after e.g. `brew shellenv`) so the
    /// shims stay first on PATH in IDE terminals and GUI-launched agents.
    Shellenv,

    /// Diagnose the Dejavu setup for the current repo.
    Doctor {
        /// Emit the checks as JSON instead of text.
        #[arg(long)]
        json: bool,
    },

    /// Internal: invoked by a shim. Runs the real command and reduces output.
    #[command(hide = true)]
    Run {
        /// Name of the shim that was invoked (e.g. `pnpm`, `git`).
        #[arg(long)]
        shim_name: String,
        /// Everything after `--` — the real command's arguments.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Show a captured run (compact by default; raw with a stream flag).
    Show {
        /// `latest` or a run id / short prefix.
        target: String,
        /// Print the stored raw stdout of the run.
        #[arg(long)]
        stdout: bool,
        /// Print the stored raw stderr of the run.
        #[arg(long)]
        stderr: bool,
        /// Print the normalized text used for run comparison.
        #[arg(long)]
        normalized: bool,
    },

    /// Search the stored raw output of a run.
    Grep {
        /// `latest` or a run id / short prefix.
        target: String,
        /// Regex to search for (grep-style exit: 0 match, 1 none, 2 error).
        pattern: String,
        /// Search the normalized text instead of the raw output.
        #[arg(long)]
        normalized: bool,
    },

    /// Show token-savings stats for the current repo.
    Stats {
        /// Emit the stats as JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Aggregate across every repo Dejavu has ever tracked.
        #[arg(long)]
        all: bool,
        /// Omit repo paths and command details that may contain private names.
        #[arg(long)]
        public: bool,
    },

    /// List repos where Dejavu has recorded activity.
    Repos {
        /// Emit the repo list as JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Include repos disabled with `dejavu disable`.
        #[arg(long)]
        all: bool,
    },

    /// Emit a Markdown report suitable for sharing.
    Report {
        /// Omit repo paths and command details that may contain private names.
        #[arg(long)]
        redact: bool,
    },

    /// Run a reproducible local benchmark (no LLM required).
    Bench {
        /// Benchmark scenario to run (default: js-validation-loop).
        #[arg(long)]
        scenario: Option<String>,
        /// Emit the benchmark report as JSON instead of text.
        #[arg(long)]
        json: bool,
    },

    /// Remove cached runs and logs.
    Clean {
        /// Only remove runs older than this age, e.g. `14d`, `12h`, `30m`.
        #[arg(long, value_name = "AGE")]
        older_than: Option<String>,
        /// Remove every run, log, and shim for the current repo's cache.
        #[arg(long)]
        all: bool,
    },

    /// Remove Dejavu's local cache and generated shims for the current repo.
    Uninstall,

    /// Enable interception for the current repo.
    Enable,

    /// Disable interception for the current repo.
    Disable,
}
