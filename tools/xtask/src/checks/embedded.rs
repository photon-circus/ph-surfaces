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

/// The two bare-metal targets, named once. The shell gate spelled them in three
/// separate places.
pub const TARGETS: [&str; 2] = ["thumbv7em-none-eabi", "riscv32imac-unknown-none-elf"];

/// An ordinary bare-metal build on the pinned toolchain.
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
    step(
        ctx,
        &proc::cargo(),
        &["build", "--target", target, "--locked"],
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

    // `rustup run` rather than `cargo +toolchain`, because CARGO may already
    // point at a concrete toolchain binary that does not understand `+`.
    step(
        ctx,
        "rustup",
        &[
            "run",
            toolchain,
            "cargo",
            "build",
            "--locked",
            "--target",
            target,
            "-Z",
            "build-std=core",
        ],
        &[],
    )
}

pub fn thumbv7em(ctx: &Ctx) -> Outcome {
    embedded_target(ctx, TARGETS[0])
}

pub fn riscv32imac(ctx: &Ctx) -> Outcome {
    embedded_target(ctx, TARGETS[1])
}

pub fn core_only_thumbv7em(ctx: &Ctx) -> Outcome {
    core_only(ctx, TARGETS[0])
}

pub fn core_only_riscv32imac(ctx: &Ctx) -> Outcome {
    core_only(ctx, TARGETS[1])
}
