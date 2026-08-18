//! The registry.
//!
//! Adding a check is one module, one row here, and one mutation case in
//! `tests/mutation.rs`. Nothing else grows -- which is the whole point of
//! replacing a single 1048-line script.
//!
//! Check names are the ones `scripts/ci.sh` printed. They appear in archived
//! release evidence and in `--only`, so they are part of the interface.
//!
//! Order matters: the cheap ratchets run before anything that compiles, so an
//! obvious violation is reported in seconds rather than after a full matrix.

pub mod cargo;
pub mod code_size;
pub mod deny;
pub mod embedded;
pub mod line_endings;
pub mod metadata;
pub mod package;
pub mod ratchets;

use crate::runner::{Check, Profile};

const ALL: &[Profile] = &[Profile::Dev, Profile::Full, Profile::Release];
const DEEP: &[Profile] = &[Profile::Full, Profile::Release];

pub const CHECKS: &[Check] = &[
    Check {
        name: "line endings",
        profiles: ALL,
        run: line_endings::line_endings,
    },
    Check {
        name: "no_std unconditional",
        profiles: ALL,
        run: ratchets::no_std_unconditional,
    },
    Check {
        name: "integer only",
        profiles: ALL,
        run: ratchets::integer_only,
    },
    Check {
        name: "no ph-curves",
        profiles: ALL,
        run: ratchets::no_ph_curves,
    },
    Check {
        name: "manifest floor",
        profiles: ALL,
        run: ratchets::manifest_floor,
    },
    Check {
        name: "fmt",
        profiles: ALL,
        run: cargo::fmt,
    },
    Check {
        name: "check",
        profiles: ALL,
        run: cargo::check,
    },
    Check {
        name: "test",
        profiles: ALL,
        run: cargo::test,
    },
    Check {
        name: "release test",
        profiles: DEEP,
        run: cargo::release_test,
    },
    Check {
        name: "examples",
        profiles: DEEP,
        run: cargo::examples,
    },
    Check {
        name: "clippy",
        profiles: ALL,
        run: cargo::clippy,
    },
    Check {
        name: "doc",
        profiles: DEEP,
        run: cargo::doc,
    },
    Check {
        name: "package list",
        profiles: DEEP,
        run: package::package_list,
    },
    Check {
        name: "package build",
        profiles: DEEP,
        run: package::package_build,
    },
    // Release-only on purpose. An ordinary run packages with `--allow-dirty`,
    // so there is no provenance to verify; the shell gate reported PASS there,
    // which claimed a check it had not performed.
    Check {
        name: "package provenance",
        profiles: &[Profile::Release],
        run: package::package_provenance,
    },
    Check {
        name: "package digest",
        profiles: DEEP,
        run: package::package_digest,
    },
    Check {
        name: "package consumer",
        profiles: DEEP,
        run: package::package_consumer,
    },
    Check {
        name: "code size snapshot",
        profiles: DEEP,
        run: code_size::code_size_snapshot,
    },
    Check {
        name: "guards fire on mutation",
        profiles: DEEP,
        run: guard_selftest,
    },
    Check {
        name: "github metadata",
        profiles: DEEP,
        run: metadata::github_metadata,
    },
    Check {
        name: "actionlint",
        profiles: DEEP,
        run: metadata::actionlint,
    },
    Check {
        name: "deny",
        profiles: DEEP,
        run: deny::deny,
    },
    Check {
        name: "core-only thumbv7em-none-eabi",
        profiles: DEEP,
        run: embedded::core_only_thumbv7em,
    },
    Check {
        name: "core-only riscv32imac-unknown-none-elf",
        profiles: DEEP,
        run: embedded::core_only_riscv32imac,
    },
    Check {
        name: "thumbv7em-none-eabi",
        profiles: DEEP,
        run: embedded::thumbv7em,
    },
    Check {
        name: "riscv32imac-unknown-none-elf",
        profiles: DEEP,
        run: embedded::riscv32imac,
    },
];

/// Every guard is shown to fail on a mutated copy of the tree.
///
/// A guard that has never been seen to fail is not evidence. These are ordinary
/// `#[test]`s in `tools/xtask/tests/mutation.rs`, not a bespoke shell harness.
/// The separate target directory keeps this from relinking the `xtask` binary
/// that is currently running it.
fn guard_selftest(ctx: &crate::runner::Ctx) -> crate::runner::Outcome {
    let target_dir = ctx.path("target/xt/selftest");
    let target_dir = target_dir.display().to_string();
    cargo::step(
        ctx,
        &crate::proc::cargo(),
        &[
            "test",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
        ],
        &[("CARGO_TARGET_DIR", target_dir.as_str())],
    )
}
