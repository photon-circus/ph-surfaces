//! Bare-metal builds, and the no-alloc proof that only one of them is.
//!
//! `-Z build-std=core` builds a sysroot containing only `core` (and
//! `compiler_builtins`), so any `alloc` or `std` reference fails to resolve.
//! That absence is the no-alloc proof. A plain `--target` build against the
//! shipped `rust-std` sysroot proves nothing here, because bare-metal sysroots
//! still ship `alloc` -- do not describe those as a no-alloc proof.

use crate::checks::cargo::step;
use crate::proc;
use crate::runner::{Ctx, Outcome};

/// An ordinary bare-metal build on the pinned toolchain, then clippy for the
/// same target with warnings denied.
///
/// Clippy is per-target on purpose: host clippy lints host cfg resolution,
/// and a lint that only fires under a bare-metal cfg would otherwise never be
/// seen. Only the runtime library builds here (`-p ph-surfaces`): the host
/// baker is a default-member and requires `std`. The Cargo examples are host
/// assertion harnesses; their fixtures are proven on these targets through
/// the downstream consumer in `tools/consumer`.
pub fn embedded_target(ctx: &Ctx, target: &str) -> Outcome {
    if ctx.skip_embedded {
        return Outcome::skip(format!("--skip-embedded; skipping target {target}"));
    }
    match proc::target_installed(target, &ctx.root) {
        Ok(true) => {}
        Ok(false) => {
            return Outcome::skip(format!(
                "target {target} not installed; skipping (rustup target add {target})"
            ));
        }
        Err(error) => return Outcome::skip(format!("rustup is not available: {error}")),
    }
    // The host baker is a default-member and requires `std`; name the runtime
    // crate so these none-target builds cannot try to compile it.
    match step(
        ctx,
        &proc::cargo(),
        &["build", "-p", "ph-surfaces", "--target", target, "--locked"],
        &[],
    ) {
        Outcome::Pass | Outcome::PassWithNote(_) => {}
        failure => return failure,
    }
    step(
        ctx,
        &proc::cargo(),
        &[
            "clippy",
            "-p",
            "ph-surfaces",
            "--target",
            target,
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )
}

/// The core-only proof.
///
/// This deliberately ignores `--skip-embedded`: skipping the ordinary target
/// builds to save time must never quietly drop the invariant proof with them.
pub fn core_only(ctx: &Ctx, target: &str) -> Outcome {
    let toolchain = &ctx.nightly;

    match proc::capture(
        "rustup",
        &["run", toolchain, "rustc", "--version"],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => {}
        Ok(_) => {
            return Outcome::skip(format!(
                "{toolchain} toolchain not installed; skipping (rustup toolchain install {toolchain})"
            ));
        }
        Err(error) => return Outcome::skip(format!("rustup is not available: {error}")),
    }

    match proc::component_installed(toolchain, "rust-src", &ctx.root) {
        Ok(true) => {}
        Ok(false) => {
            return Outcome::skip(format!(
                "{toolchain} rust-src not installed; skipping \
                 (rustup component add rust-src --toolchain {toolchain})"
            ));
        }
        Err(error) => return Outcome::skip(format!("rustup is not available: {error}")),
    }

    // `-p ph-surfaces`: default-members also include the host baker, which
    // requires `std` and must not be compiled for these none targets.
    step(
        ctx,
        "rustup",
        &[
            "run",
            toolchain,
            "cargo",
            "build",
            "--locked",
            "-p",
            "ph-surfaces",
            "--target",
            target,
            "-Z",
            "build-std=core",
        ],
        &[],
    )
}
