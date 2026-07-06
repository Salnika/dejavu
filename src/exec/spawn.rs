//! Process spawning behind a `CommandRunner` seam (so bench/tests can inject
//! fixture output), plus the single exit-code normalization point.

use super::ExecOutcome;
use crate::env;
use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

/// What to run and how.
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    /// Sanitized PATH (shim dir removed) for the child.
    pub env_path: OsString,
    /// Capture stdout/stderr (Optimize) vs inherit stdio (Passthrough).
    pub capture: bool,
    /// When capturing, whether to inherit stdin (pipe) or use `/dev/null` (tty).
    pub inherit_stdin: bool,
    /// Reserved for M3 capped capture; ignored in the full-capture path.
    pub capture_limit: Option<usize>,
}

/// Abstraction over "run this command and give me its output + exit code".
pub trait CommandRunner {
    fn run(&self, spec: &SpawnSpec) -> std::io::Result<ExecOutcome>;
}

/// Production runner: actually spawns the process.
pub struct RealRunner;

impl CommandRunner for RealRunner {
    fn run(&self, spec: &SpawnSpec) -> std::io::Result<ExecOutcome> {
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .env("PATH", &spec.env_path)
            // Belt-and-suspenders anti-recursion: even if a shim dir lingers on
            // the child's PATH, the nested `dejavu run` will fast-passthrough.
            .env(env::DISABLED, "1");

        if spec.capture {
            cmd.stdin(if spec.inherit_stdin {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

            let start = Instant::now();
            let output = cmd.spawn()?.wait_with_output()?;
            let duration = start.elapsed();
            Ok(ExecOutcome {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: normalized_exit_code(output.status),
                duration,
                captured: true,
                truncated_raw: false,
            })
        } else {
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            let start = Instant::now();
            let status = cmd.spawn()?.wait()?;
            Ok(ExecOutcome {
                stdout: Vec::new(),
                stderr: Vec::new(),
                exit_code: normalized_exit_code(status),
                duration: start.elapsed(),
                captured: false,
                truncated_raw: false,
            })
        }
    }
}

/// Normalize an `ExitStatus` into an `i32` the way a shell would: a normal exit
/// yields its code; a signal death yields `128 + signum`.
pub fn normalized_exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        code
    } else if let Some(signal) = status.signal() {
        128 + signal
    } else {
        1
    }
}
