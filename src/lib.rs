//! Dejavu — a local output-reduction proxy for coding agents.
//!
//! Dejavu intercepts common shell commands via `PATH` shims, runs the real
//! command, and returns a compact/diffed view to the agent while preserving the
//! exact exit code. It reduces what the agent *reads*, never what the shell
//! *executes*.

pub mod agent;
pub mod cli;
pub mod commands;
pub mod config;
pub mod env;
pub mod error;
pub mod exec;
pub mod paths;
pub mod reduce;
pub mod repo;
pub mod runtime;
pub mod state;
pub mod store;
pub mod util;
