//! Full-history secret scan.
//!
//! `RELEASING.md` requires a completed secret review before the repository
//! goes public. This check automates the scanning half so it runs on every
//! full gate instead of being remembered at release time: gitleaks exits
//! nonzero when it finds a leak, which is exactly the FAIL condition, and the
//! release profile turns a missing tool from SKIP into FAIL. git-sizer stays
//! a release-runbook step -- it reports repository shape but renders no
//! verdict a gate could act on.

use crate::checks::cargo::step;
use crate::proc;
use crate::runner::{Ctx, Outcome};

pub fn secret_scan(ctx: &Ctx) -> Outcome {
    // The scan covers history, so it needs a repository, not just files. A
    // tracked-file copy without one skips rather than passing vacuously.
    match proc::capture("git", &["rev-parse", "--git-dir"], &ctx.root) {
        Ok(output) if output.ok() => {}
        _ => return Outcome::skip("no Git repository at this root; skipping the secret scan"),
    }

    // A shallow repository contains only a suffix of the history. Scanning it
    // cannot support a full-history claim, even when gitleaks reports no
    // findings. The release profile promotes this SKIP to FAIL.
    match proc::capture("git", &["rev-parse", "--is-shallow-repository"], &ctx.root) {
        Ok(output) if output.ok() && output.stdout.trim() == "false" => {}
        Ok(output) if output.ok() && output.stdout.trim() == "true" => {
            return Outcome::skip(
                "Git repository is shallow; fetch full history before running the secret scan",
            );
        }
        Ok(output) => {
            return Outcome::skip(format!(
                "could not determine whether the Git repository is shallow: {}",
                output.stderr.trim()
            ));
        }
        Err(error) => {
            return Outcome::skip(format!(
                "could not determine whether the Git repository is shallow: {error}"
            ));
        }
    }

    let version = match proc::capture("gitleaks", &["version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        Ok(_) | Err(_) => {
            return Outcome::skip(
                "gitleaks not installed; skipping (https://github.com/gitleaks/gitleaks)",
            );
        }
    };
    println!("tool: gitleaks {version}");

    step(ctx, "gitleaks", &["git", ".", "--redact"], &[])
}
