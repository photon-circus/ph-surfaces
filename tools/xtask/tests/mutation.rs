//! Guard self-test: every ratchet is shown to fail on a mutated tree.
//!
//! A guard that has never been seen to fail is not evidence. This replaces
//! `scripts/guard-selftest.sh`, and fixes two defects it carried:
//!
//! * it re-entered the whole gate as a subprocess without clearing the
//!   environment, so a `REQUIRE_NO_SKIPS=1` parent run could make a guard look
//!   like it fired when all that happened was a SKIP being escalated;
//! * it hardcoded the `nightly` alias, so under dated-nightly release evidence
//!   half of the `alloc` case silently skipped.
//!
//! Here a check is an ordinary function called with an explicit `Ctx`, so there
//! is no ambient environment and no toolchain assumption to get wrong.
//!
//! Copies live in the system temp directory, not under `target/`. A copy nested
//! inside this repository would let `git rev-parse` walk up and find the parent
//! checkout, which is what the retired harness needed `GIT_CEILING_DIRECTORIES`
//! to prevent.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use xtask::checks::{line_endings, package, ratchets};
use xtask::runner::{Check, Ctx, Outcome, Profile};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/xtask sits two levels below the repository root")
        .to_path_buf()
}

/// A fresh copy of every tracked file, plus `Cargo.lock`.
fn tracked_copy(case: &str) -> PathBuf {
    let root = repo_root();
    let destination = std::env::temp_dir().join("ph-surfaces-mutation").join(case);
    let _ = fs::remove_dir_all(&destination);
    fs::create_dir_all(&destination).expect("could not create the mutation copy");

    let listing = Command::new("git")
        .args(["ls-files"])
        .current_dir(&root)
        .output()
        .expect("git ls-files could not run");
    assert!(listing.status.success(), "git ls-files failed");

    for relative in String::from_utf8_lossy(&listing.stdout).lines() {
        let relative = relative.trim();
        if relative.is_empty() {
            continue;
        }
        let source = root.join(relative);
        if !source.is_file() {
            continue;
        }
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("could not create a directory in the copy");
        }
        fs::copy(&source, &target).expect("could not copy a tracked file");
    }
    fs::copy(root.join("Cargo.lock"), destination.join("Cargo.lock"))
        .expect("could not copy Cargo.lock");

    destination
}

fn ctx(root: &Path, profile: Profile) -> Ctx {
    Ctx {
        root: root.to_path_buf(),
        profile,
        nightly: "nightly".to_string(),
        skip_embedded: false,
    }
}

#[track_caller]
fn assert_fires(case: &str, name: &str, outcome: Outcome) {
    match outcome {
        Outcome::Fail(reason) => {
            assert!(
                !reason.trim().is_empty(),
                "{case}: guard \"{name}\" fired without saying why"
            );
        }
        Outcome::Pass => panic!("{case}: guard \"{name}\" did NOT fire"),
        Outcome::Skip(reason) => {
            panic!("{case}: guard \"{name}\" skipped instead of firing: {reason}")
        }
    }
}

fn rewrite(path: &Path, edit: impl Fn(&str) -> String) {
    let before = fs::read_to_string(path).expect("could not read a file to mutate");
    let after = edit(&before);
    assert_ne!(before, after, "the mutation changed nothing: {}", path.display());
    fs::write(path, after).expect("could not write a mutated file");
}

#[test]
fn conditional_no_std_is_rejected() {
    let root = tracked_copy("no_std-conditional");
    rewrite(&root.join("src/lib.rs"), |text| {
        text.replace(
            "#![no_std]",
            "#![cfg_attr(not(feature = \"std\"), no_std)]",
        )
    });
    assert_fires(
        "no_std-conditional",
        "no_std unconditional",
        ratchets::no_std_unconditional(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_features_table_is_rejected() {
    let root = tracked_copy("features-table");
    rewrite(&root.join("Cargo.toml"), |text| {
        format!("{text}\n[features]\ndefault = []\n")
    });
    assert_fires(
        "features-table",
        "no_std unconditional",
        ratchets::no_std_unconditional(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn runtime_allocator_paths_are_rejected() {
    let root = tracked_copy("alloc");
    rewrite(&root.join("src/lib.rs"), |text| {
        format!("{text}\npub fn leak() -> alloc::vec::Vec<u8> {{ alloc::vec::Vec::new() }}\n")
    });
    assert_fires(
        "alloc",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn example_floating_point_is_rejected() {
    let root = tracked_copy("example-float");
    rewrite(&root.join("examples/firmware_quickstart.rs"), |text| {
        text.replacen("fn main() {", "fn main() {\n    let _probe = 0.5f32;", 1)
    });
    assert_fires(
        "example-float",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn example_host_paths_are_rejected() {
    let root = tracked_copy("example-host-path");
    rewrite(&root.join("examples/firmware_quickstart.rs"), |text| {
        text.replacen(
            "fn main() {",
            "fn main() {\n    let _probe: std::vec::Vec<u8> = std::vec::Vec::new();",
            1,
        )
    });
    assert_fires(
        "example-host-path",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_ph_curves_dependency_is_rejected() {
    let root = tracked_copy("ph-curves");
    rewrite(&root.join("Cargo.toml"), |text| {
        format!("{text}\n[dependencies.ph-curves]\npath = \"../ph-curves\"\n")
    });
    assert_fires(
        "ph-curves",
        "no ph-curves",
        ratchets::no_ph_curves(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_manifest_floor_change_is_rejected() {
    let root = tracked_copy("manifest-version");
    rewrite(&root.join("Cargo.toml"), |text| {
        text.replace("version = \"0.1.0-incubating.1\"", "version = \"0.2.0\"")
    });
    assert_fires(
        "manifest-version",
        "manifest floor",
        ratchets::manifest_floor(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn crlf_in_a_tracked_file_is_rejected() {
    let root = tracked_copy("crlf");
    // The check reads the tracked list from git, so the copy needs its own
    // repository -- and being outside `target/` means it cannot see this one.
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "--all"],
        vec![
            "-c",
            "user.email=selftest@example.invalid",
            "-c",
            "user.name=selftest",
            "commit",
            "--quiet",
            "--message=mutation",
        ],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()
            .expect("git could not run");
        assert!(status.success(), "git {args:?} failed");
    }

    let path = root.join("README.md");
    let bytes = fs::read(&path).expect("could not read README.md");
    let mut crlf = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        if byte == b'\n' {
            crlf.push(b'\r');
        }
        crlf.push(byte);
    }
    fs::write(&path, crlf).expect("could not write CRLF README.md");

    assert_fires(
        "crlf",
        "line endings",
        line_endings::line_endings(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_tree_without_provenance_fails_strict_packaging() {
    // A tracked-file-only copy has no Git provenance at all, so release
    // evidence must refuse to package it.
    let root = tracked_copy("release-provenance");
    assert_fires(
        "release-provenance",
        "package list",
        package::package_list(&ctx(&root, Profile::Release)),
    );
}

#[test]
fn a_would_be_skip_fails_the_release_profile() {
    fn always_skips(_: &Ctx) -> Outcome {
        Outcome::skip("a tool this machine does not have")
    }
    const REGISTRY: &[Check] = &[Check {
        name: "synthetic",
        profiles: &[Profile::Full, Profile::Release],
        run: always_skips,
    }];

    let root = repo_root();
    assert_eq!(
        xtask::runner::run(&ctx(&root, Profile::Full), REGISTRY, &[], false),
        0,
        "an ordinary run tolerates a SKIP"
    );
    assert_eq!(
        xtask::runner::run(&ctx(&root, Profile::Release), REGISTRY, &[], false),
        1,
        "release evidence must record a would-be SKIP as a FAIL"
    );
}
