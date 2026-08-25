//! Supply-chain policy, with the tool version recorded.
//!
//! Issue #27 asks for cargo-deny to be pinned or recorded. A policy verdict is
//! only evidence if the tool that produced it is identified, so the version is
//! always printed here and logged by the release runbook's tool-version
//! capture. It is recorded rather than pinned: the shipped `ph-surfaces`
//! package has an empty dependency graph, so the policy has almost nothing to
//! evaluate, and an exact-version requirement would fail release evidence on
//! every routine cargo-deny update for zero added signal. Host `xtask`
//! dependencies live in `[workspace.dependencies]` and are excluded from this
//! graph. Revisit pinning when the shipped `[dependencies]` gains its first
//! entry.

use crate::checks::cargo::step;
use crate::proc;
use crate::runner::{Ctx, Outcome};

pub fn deny(ctx: &Ctx) -> Outcome {
    let version = match proc::capture("cargo-deny", &["--version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        Ok(_) | Err(_) => {
            return Outcome::skip("cargo-deny not installed; skipping (cargo install cargo-deny)");
        }
    };
    println!("tool: {version}");

    step(ctx, &proc::cargo(), &["deny", "check"], &[])
}
