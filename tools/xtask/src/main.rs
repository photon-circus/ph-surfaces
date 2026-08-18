//! The ph-surfaces verification gate.
//!
//! This replaces `scripts/ci.sh`. It runs the same named checks, reports the
//! same PASS/FAIL/SKIP verdicts in the same format, and runs natively on
//! Windows and Linux with nothing but the pinned cargo the crate already
//! requires -- no shell, no PowerShell twin, no container.
//!
//! Usage:
//!   cargo xtask ci                                  full matrix
//!   cargo xtask ci --profile dev                    fast inner loop
//!   cargo xtask ci --profile release                release evidence; no SKIPs
//!   cargo xtask ci --only 'no ph-curves'            one named check
//!   cargo xtask ci --nightly nightly-YYYY-MM-DD     exact core-only toolchain
//!   cargo xtask ci --skip-embedded                  skip target-dependent checks
//!   cargo xtask ci --fail-fast                      stop at the first failure
//!   cargo xtask list                                the registry

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use xtask::checks;
use xtask::runner::{self, Ctx, Profile};

fn main() -> ExitCode {
    match dispatch() {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::from(2)
        }
    }
}

fn dispatch() -> Result<u8, String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "ci".to_string());
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "ci" => run_ci(&rest),
        "code-size" => run_code_size(&rest),
        "list" => {
            runner::list(checks::CHECKS);
            Ok(0)
        }
        "--help" | "-h" | "help" => {
            print_usage();
            Ok(0)
        }
        other => Err(format!(
            "unknown command `{other}`; try `ci`, `code-size`, or `list`"
        )),
    }
}

fn print_usage() {
    println!("{}", include_str!("usage.txt"));
}

/// Re-measure the code-size snapshot, and optionally rewrite the committed one.
fn run_code_size(args: &[String]) -> Result<u8, String> {
    let write = args.iter().any(|argument| argument == "--write");
    for argument in args {
        if argument != "--write" {
            return Err(format!("unknown option `{argument}`"));
        }
    }

    let here = env::current_dir().map_err(|error| error.to_string())?;
    let root = runner::find_root(&here)
        .ok_or_else(|| format!("no ph-surfaces checkout at or above {}", here.display()))?;
    let ctx = Ctx {
        root,
        profile: Profile::Full,
        nightly: "nightly".to_string(),
        skip_embedded: false,
    };

    let snapshot = checks::code_size::measure(&ctx)?;
    if write {
        let path = ctx.path(checks::code_size::SNAPSHOT);
        std::fs::write(&path, &snapshot).map_err(|error| error.to_string())?;
        println!("wrote {}", path.display());
    } else {
        print!("{snapshot}");
    }
    Ok(0)
}

fn run_ci(args: &[String]) -> Result<u8, String> {
    let mut profile = Profile::Full;
    let mut only: Vec<String> = Vec::new();
    let mut nightly = "nightly".to_string();
    let mut skip_embedded = false;
    let mut fail_fast = false;
    let mut root: Option<PathBuf> = None;

    let mut cursor = args.iter();
    while let Some(argument) = cursor.next() {
        let mut value = || {
            cursor
                .next()
                .cloned()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--profile" => {
                let name = value()?;
                profile = Profile::parse(&name).ok_or_else(|| {
                    format!("unknown profile `{name}`; use dev, full, or release")
                })?;
            }
            "--only" => only.push(value()?),
            "--nightly" => nightly = value()?,
            "--root" => root = Some(PathBuf::from(value()?)),
            "--skip-embedded" => skip_embedded = true,
            "--fail-fast" => fail_fast = true,
            "--help" | "-h" => {
                print_usage();
                return Ok(0);
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }

    // Release evidence must name the reviewed dated nightly, so that the
    // recorded rustc and cargo versions describe a toolchain that cannot move
    // underneath a later re-run.
    if profile == Profile::Release && nightly == "nightly" {
        return Err(
            "the release profile requires --nightly nightly-YYYY-MM-DD, not the moving alias"
                .to_string(),
        );
    }

    let root = match root {
        Some(given) => given,
        None => {
            let here = env::current_dir().map_err(|error| error.to_string())?;
            runner::find_root(&here)
                .ok_or_else(|| format!("no ph-surfaces checkout at or above {}", here.display()))?
        }
    };

    let ctx = Ctx {
        root,
        profile,
        nightly,
        skip_embedded,
    };

    println!("profile: {}", ctx.profile);
    println!("root:    {}", ctx.root.display());

    Ok(runner::run(&ctx, checks::CHECKS, &only, fail_fast) as u8)
}
