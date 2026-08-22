//! Process invocation and tool discovery.
//!
//! Every external command goes through here so that argument quoting, working
//! directory, and CR stripping are handled in one place rather than at each
//! call site. There is no shell in the path, so nothing here depends on MSYS
//! argument or path translation.

use std::env;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// The cargo that launched us, so a check cannot silently use a different one
/// than the pinned toolchain resolved.
pub fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

pub struct Captured {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    pub fn ok(&self) -> bool {
        self.status == Some(0)
    }
}

fn build(program: &str, args: &[impl AsRef<OsStr>], directory: &Path) -> Command {
    let mut command = Command::new(program);
    command.args(args).current_dir(directory);
    // Incremental artifacts are noise for a gate and cost disk on every run.
    command.env("CARGO_INCREMENTAL", "0");
    command
}

/// Run a command with inherited stdio, so its output lands in the run log in
/// order. Returns the exit code, or `None` if it was killed by a signal.
pub fn run(
    program: &str,
    args: &[impl AsRef<OsStr>],
    directory: &Path,
    env: &[(&str, &str)],
) -> io::Result<Option<i32>> {
    let mut command = build(program, args, directory);
    for (key, value) in env {
        command.env(key, value);
    }
    Ok(command.status()?.code())
}

/// Run a command and capture its output, with CR stripped from both streams.
///
/// The shell gate applied `tr -d '\r'` to some tool output and not other tool
/// output; that inconsistency is what let a CRLF line turn a target check into
/// a false SKIP. Here it is unconditional.
pub fn capture(
    program: &str,
    args: &[impl AsRef<OsStr>],
    directory: &Path,
) -> io::Result<Captured> {
    let output = build(program, args, directory)
        .stdin(Stdio::null())
        .output()?;
    Ok(Captured {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        stderr: String::from_utf8_lossy(&output.stderr).replace('\r', ""),
    })
}

/// Whether `rustup` reports a target as installed.
pub fn target_installed(target: &str, directory: &Path) -> io::Result<bool> {
    let output = capture("rustup", &["target", "list", "--installed"], directory)?;
    Ok(output.ok() && output.stdout.lines().any(|line| line.trim() == target))
}

/// Whether a toolchain exists and carries a component.
pub fn component_installed(toolchain: &str, component: &str, directory: &Path) -> io::Result<bool> {
    let output = capture(
        "rustup",
        &["component", "list", "--toolchain", toolchain, "--installed"],
        directory,
    )?;
    Ok(output.ok()
        && output
            .stdout
            .lines()
            .any(|line| line.trim() == component || line.trim().starts_with(component)))
}
