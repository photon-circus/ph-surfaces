//! The host contract: formatting, both test profiles, the Cargo examples,
//! lints, and rustdoc -- all with warnings denied. There is no separate
//! `cargo check` row: `clippy --all-targets -D warnings` subsumes it.

use crate::proc;
use crate::runner::{Ctx, Outcome};

/// Run a command, inheriting stdio so its output interleaves into the run log
/// exactly where the shell gate put it.
pub fn step(ctx: &Ctx, program: &str, args: &[&str], env: &[(&str, &str)]) -> Outcome {
    match proc::run(program, args, &ctx.root, env) {
        Ok(Some(0)) => Outcome::Pass,
        Ok(Some(code)) => Outcome::fail(format!("{program} {} exited {code}", args.join(" "))),
        Ok(None) => Outcome::fail(format!("{program} {} was terminated", args.join(" "))),
        Err(error) => Outcome::fail(format!("{program} could not run: {error}")),
    }
}

fn cargo_step(ctx: &Ctx, args: &[&str]) -> Outcome {
    step(ctx, &proc::cargo(), args, &[])
}

pub fn fmt(ctx: &Ctx) -> Outcome {
    cargo_step(ctx, &["fmt", "--all", "--", "--check"])
}

pub fn test(ctx: &Ctx) -> Outcome {
    cargo_step(ctx, &["test", "--locked"])
}

/// Release-profile tests. Overflow checks and codegen differ from debug, so a
/// green debug suite is not evidence for the profile firmware actually ships.
pub fn release_test(ctx: &Ctx) -> Outcome {
    cargo_step(ctx, &["test", "--locked", "--release"])
}

pub fn examples(ctx: &Ctx) -> Outcome {
    for example in &ctx.config.examples {
        match cargo_step(ctx, &["run", "--locked", "--example", example]) {
            Outcome::Pass => continue,
            failure => return failure,
        }
    }
    Outcome::Pass
}

pub fn clippy(ctx: &Ctx) -> Outcome {
    cargo_step(
        ctx,
        &[
            "clippy",
            "--locked",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

pub fn doc(ctx: &Ctx) -> Outcome {
    step(
        ctx,
        &proc::cargo(),
        &["doc", "--locked", "--no-deps"],
        &[("RUSTDOCFLAGS", "-D warnings")],
    )
}
