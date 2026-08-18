//! Supply-chain policy, against a recorded tool version.
//!
//! Issue #27 asks for cargo-deny to be pinned or recorded. A policy verdict is
//! only evidence if the tool that produced it is identified, so the version is
//! always printed, and in the release profile it must be the reviewed one.

use crate::checks::cargo::step;
use crate::proc;
use crate::runner::{Ctx, Outcome};

/// The reviewed cargo-deny. Bump this deliberately, with the policy re-read.
pub const REVIEWED_VERSION: &str = "0.20.2";

pub fn deny(ctx: &Ctx) -> Outcome {
    let version = match proc::capture("cargo-deny", &["--version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        Ok(_) | Err(_) => {
            return Outcome::skip("cargo-deny not installed; skipping (cargo install cargo-deny)");
        }
    };

    println!("tool: {version}");
    let reviewed = format!("cargo-deny {REVIEWED_VERSION}");
    if version != reviewed && ctx.strict() {
        return Outcome::fail(format!(
            "release evidence requires {reviewed}, found {version}.\n\
             Install it, or review the policy against the new version and update \
             REVIEWED_VERSION."
        ));
    }
    if version != reviewed {
        println!("note: reviewed version is {reviewed}");
    }

    step(ctx, &proc::cargo(), &["deny", "check"], &[])
}
