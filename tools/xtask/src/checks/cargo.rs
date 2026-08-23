//! The host contract: formatting, both test profiles, the Cargo examples,
//! lints, and rustdoc -- all with warnings denied. There is no separate
//! `cargo check` row: `clippy --all-targets -D warnings` subsumes it.

use crate::proc;
use crate::runner::{Ctx, Outcome};
use serde::Deserialize;

#[derive(Deserialize)]
struct CoverageReport {
    data: Vec<CoverageData>,
}

#[derive(Deserialize)]
struct CoverageData {
    totals: CoverageTotals,
}

#[derive(Deserialize)]
struct CoverageTotals {
    lines: CoverageMetric,
    regions: CoverageMetric,
    functions: CoverageMetric,
}

#[derive(Deserialize)]
struct CoverageMetric {
    percent: f64,
}

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
            Outcome::Pass | Outcome::PassWithNote(_) => continue,
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

/// Measure host test coverage without imposing a percentage ratchet.
pub fn coverage(ctx: &Ctx) -> Outcome {
    let cargo = proc::cargo();
    let version = match proc::capture(&cargo, &["llvm-cov", "--version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        Ok(_) | Err(_) => {
            return Outcome::skip(
                "cargo-llvm-cov not installed; install it with `cargo install cargo-llvm-cov --locked`",
            );
        }
    };
    println!("tool: {version}");

    match cargo_step(
        ctx,
        &["llvm-cov", "--locked", "--all-targets", "--summary-only"],
    ) {
        Outcome::Pass => coverage_totals(ctx, &cargo),
        outcome => outcome,
    }
}

fn coverage_totals(ctx: &Ctx, cargo: &str) -> Outcome {
    let output = match proc::capture(
        cargo,
        &["llvm-cov", "report", "--json", "--summary-only"],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => output,
        Ok(output) => {
            return Outcome::fail(format!(
                "coverage tests passed but the summary report failed: {}",
                output.stderr.trim()
            ));
        }
        Err(error) => {
            return Outcome::fail(format!(
                "coverage tests passed but the summary report could not run: {error}"
            ));
        }
    };
    let report: CoverageReport = match serde_json::from_str(&output.stdout) {
        Ok(report) => report,
        Err(error) => {
            return Outcome::fail(format!(
                "coverage tests passed but the summary report was invalid: {error}"
            ));
        }
    };
    let Some(totals) = report.data.first().map(|data| &data.totals) else {
        return Outcome::fail("coverage tests passed but the summary report had no totals");
    };

    Outcome::pass_with_note(format!(
        "lines {:.2}%, regions {:.2}%, functions {:.2}%",
        totals.lines.percent, totals.regions.percent, totals.functions.percent
    ))
}
