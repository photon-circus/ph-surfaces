//! Guard self-test: every ratchet is shown to fail on a mutated tree.
//!
//! A guard that has never been seen to fail is not evidence. Each case calls
//! the relevant guard directly with an explicit `Ctx`, so an unrelated failure,
//! an escalated `SKIP`, or ambient configuration cannot satisfy the assertion.
//!
//! Copies live in the system temp directory, not under `target/`. A copy nested
//! inside this repository would let `git rev-parse` walk up and find the parent
//! checkout instead of observing the mutation copy's actual provenance.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use xtask::checks::{bake, history, line_endings, package, publish_lock, ratchets};
use xtask::config::{Action, CheckSpec, Config};
use xtask::runner::{Ctx, Outcome, Profile};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the repository root")
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
    fs::copy(
        root.join("xtask/config.ron"),
        destination.join("xtask/config.ron"),
    )
    .expect("could not copy xtask configuration");

    destination
}

fn ctx(root: &Path, profile: Profile) -> Ctx {
    Ctx {
        root: root.to_path_buf(),
        profile,
        nightly: "nightly".to_string(),
        skip_embedded: false,
        coverage: false,
        config: Arc::new(Config::load(root).expect("mutation configuration must load")),
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
        Outcome::Pass | Outcome::PassWithNote(_) => {
            panic!("{case}: guard \"{name}\" did NOT fire")
        }
        Outcome::Skip(reason) => {
            panic!("{case}: guard \"{name}\" skipped instead of firing: {reason}")
        }
    }
}

fn rewrite(path: &Path, edit: impl Fn(&str) -> String) {
    let before = fs::read_to_string(path).expect("could not read a file to mutate");
    let after = edit(&before);
    assert_ne!(
        before,
        after,
        "the mutation changed nothing: {}",
        path.display()
    );
    fs::write(path, after).expect("could not write a mutated file");
}

#[test]
fn conditional_no_std_is_rejected() {
    let root = tracked_copy("no_std-conditional");
    rewrite(&root.join("crates/surfaces/src/lib.rs"), |text| {
        text.replace("#![no_std]", "#![cfg_attr(not(feature = \"std\"), no_std)]")
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
    rewrite(&root.join("crates/surfaces/Cargo.toml"), |text| {
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
    rewrite(&root.join("crates/surfaces/src/lib.rs"), |text| {
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
    rewrite(
        &root.join("crates/surfaces/examples/firmware_quickstart.rs"),
        |text| text.replacen("fn main() {", "fn main() {\n    let _probe = 0.5f32;", 1),
    );
    assert_fires(
        "example-float",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn example_host_paths_are_rejected() {
    let root = tracked_copy("example-host-path");
    rewrite(
        &root.join("crates/surfaces/examples/firmware_quickstart.rs"),
        |text| {
            text.replacen(
                "fn main() {",
                "fn main() {\n    let _probe: std::vec::Vec<u8> = std::vec::Vec::new();",
                1,
            )
        },
    );
    assert_fires(
        "example-host-path",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn wide_arithmetic_outside_the_kernel_is_rejected() {
    let root = tracked_copy("wide-int");
    // Inject into the runtime region of `evaluate`, not the file tail: the
    // scanner deliberately exempts each file's `#[cfg(test)]` tail, so an
    // appended line would not prove the guard.
    rewrite(&root.join("crates/surfaces/src/evaluate.rs"), |text| {
        text.replacen(
            "let x_cell = self.locate_x(x)?;",
            "let _wide: i64 = 0;\n        let x_cell = self.locate_x(x)?;",
            1,
        )
    });
    assert_fires(
        "wide-int",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_128_bit_integer_in_runtime_code_is_rejected() {
    let root = tracked_copy("wide-int-128");
    // The kernel itself may widen to 64 bits, never to 128; prove the ban
    // holds even inside `crates/surfaces/src/interp.rs`.
    rewrite(&root.join("crates/surfaces/src/interp.rs"), |text| {
        text.replacen(
            "let span = i64::from(x1) - i64::from(x0);",
            "let _widest: i128 = 0;\n    let span = i64::from(x1) - i64::from(x0);",
            1,
        )
    });
    assert_fires(
        "wide-int-128",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn runtime_code_after_a_test_item_is_rejected() {
    let root = tracked_copy("runtime-after-test");
    rewrite(&root.join("crates/surfaces/src/evaluate.rs"), |text| {
        format!("{text}\npub fn hidden_wide_integer() -> i64 {{ 0 }}\n")
    });
    assert_fires(
        "runtime-after-test",
        "integer only",
        ratchets::integer_only(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_ph_curves_dependency_is_rejected() {
    let root = tracked_copy("ph-curves");
    rewrite(&root.join("crates/surfaces/Cargo.toml"), |text| {
        format!("{text}\n[dependencies.ph-curves]\npath = \"../ph-curves\"\n")
    });
    assert_fires(
        "ph-curves",
        "no ph-curves",
        ratchets::no_ph_curves(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_runtime_dependency_on_the_baker_is_rejected() {
    let root = tracked_copy("runtime-bake-dep");
    rewrite(&root.join("crates/surfaces/Cargo.toml"), |text| {
        format!("{text}\n[dependencies.ph-surfaces-bake]\npath = \"../surfaces-bake\"\n")
    });
    assert_fires(
        "runtime-bake-dep",
        "no ph-curves",
        ratchets::no_ph_curves(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn exceeding_the_baker_line_budget_is_rejected() {
    let root = tracked_copy("baker-line-budget");
    rewrite(&root.join("xtask/config.ron"), |text| {
        text.replace(
            "max_implementation_lines: 1500",
            "max_implementation_lines: 1",
        )
    });
    assert_fires(
        "baker-line-budget",
        "baker line budget",
        bake::baker_line_budget(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_manifest_floor_change_is_rejected() {
    let root = tracked_copy("manifest-version");
    let configuration = Config::load(&root).unwrap();
    let current = format!("version = \"{}\"", configuration.package.version);
    rewrite(&root.join("crates/surfaces/Cargo.toml"), |text| {
        text.replace(&current, "version = \"0.0.0-mutated\"")
    });
    assert_fires(
        "manifest-version",
        "manifest floor",
        ratchets::manifest_floor(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn an_unclassified_workspace_member_is_rejected() {
    let root = tracked_copy("unclassified-member");
    fs::create_dir_all(root.join("crates/unclassified"))
        .expect("could not create the unclassified member");
    fs::write(
        root.join("crates/unclassified/Cargo.toml"),
        "[package]\n\
         name = \"unclassified\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\
         publish = false\n",
    )
    .expect("could not write the unclassified member manifest");
    rewrite(&root.join("Cargo.toml"), |text| {
        text.replace(
            "members = [\"crates/surfaces\", \"crates/surfaces-bake\", \"xtask\"]",
            "members = [\"crates/surfaces\", \"crates/surfaces-bake\", \"crates/unclassified\", \"xtask\"]",
        )
    });
    assert_fires(
        "unclassified-member",
        "publish lock",
        publish_lock::publish_lock(&ctx(&root, Profile::Full)),
    );
}

/// Turn a tracked-file copy into its own repository with one commit. Being
/// outside `target/` means it cannot see this checkout's repository.
fn init_repository(root: &Path) {
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
            .current_dir(root)
            .status()
            .expect("git could not run");
        assert!(status.success(), "git {args:?} failed");
    }
}

#[test]
fn crlf_in_a_tracked_file_is_rejected() {
    let root = tracked_copy("crlf");
    // The check reads the tracked list from git, so the copy needs its own
    // repository.
    init_repository(&root);

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
fn a_planted_secret_is_rejected() {
    // Without gitleaks the guard itself reports SKIP, and this case can prove
    // nothing either way; degrade the same way rather than failing the suite
    // on machines that do not carry the tool. The strict release run installs
    // gitleaks, so release evidence still sees this case execute.
    let have_gitleaks = Command::new("gitleaks")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !have_gitleaks {
        eprintln!("gitleaks not installed; the secret-scan mutation case cannot run here");
        return;
    }

    let root = tracked_copy("secret");
    init_repository(&root);
    let initial_branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&root)
        .output()
        .expect("git could not identify the initial branch");
    assert!(initial_branch.status.success());
    let initial_branch = String::from_utf8(initial_branch.stdout)
        .expect("branch name was not UTF-8")
        .trim()
        .to_string();
    let status = Command::new("git")
        .args(["switch", "--quiet", "--create", "secret-ref"])
        .current_dir(&root)
        .status()
        .expect("git could not create the secret-only ref");
    assert!(status.success());
    // A shaped-and-random token, so both the pattern rule and its entropy
    // requirement are met. It is not a real credential, and it is assembled
    // at runtime so the token never appears in this repository's own history,
    // where the real secret scan would find it.
    let token = format!("ghp_{}{}", "wWPw5k4aXcZcnwHq1FqF", "q7BdkS9AqPqm2eKv");
    fs::write(root.join("leaked.env"), format!("GITHUB_TOKEN={token}\n"))
        .expect("could not plant the secret");
    for args in [
        vec!["add", "--all"],
        vec![
            "-c",
            "user.email=selftest@example.invalid",
            "-c",
            "user.name=selftest",
            "commit",
            "--quiet",
            "--message=secret-only-ref",
        ],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()
            .expect("git could not commit the secret-only ref");
        assert!(status.success(), "git {args:?} failed");
    }
    let status = Command::new("git")
        .args(["switch", "--quiet", &initial_branch])
        .current_dir(&root)
        .status()
        .expect("git could not return to the clean branch");
    assert!(status.success());
    assert!(
        !root.join("leaked.env").exists(),
        "the secret must be reachable only from the non-HEAD ref"
    );

    assert_fires(
        "secret",
        "secret scan",
        history::secret_scan(&ctx(&root, Profile::Full)),
    );
}

#[test]
fn a_shallow_repository_cannot_claim_a_full_history_scan() {
    let source = tracked_copy("shallow-source");
    init_repository(&source);
    fs::write(source.join("second-commit.txt"), "second commit\n")
        .expect("could not add the second commit fixture");
    let status = Command::new("git")
        .args(["add", "--all"])
        .current_dir(&source)
        .status()
        .expect("git add could not run");
    assert!(status.success(), "git add failed");
    let status = Command::new("git")
        .args([
            "-c",
            "user.email=selftest@example.invalid",
            "-c",
            "user.name=selftest",
            "commit",
            "--quiet",
            "--message=second",
        ])
        .current_dir(&source)
        .status()
        .expect("git commit could not run");
    assert!(status.success(), "git commit failed");

    let shallow = std::env::temp_dir()
        .join("ph-surfaces-mutation")
        .join("shallow-clone");
    let _ = fs::remove_dir_all(&shallow);
    let source_url = format!(
        "file:///{}",
        source.display().to_string().replace('\\', "/")
    );
    let status = Command::new("git")
        .args(["clone", "--quiet", "--depth=1", &source_url])
        .arg(&shallow)
        .status()
        .expect("git clone could not run");
    assert!(status.success(), "shallow git clone failed");

    match history::secret_scan(&ctx(&shallow, Profile::Full)) {
        Outcome::Skip(reason) => assert!(
            reason.contains("shallow"),
            "shallow repository skipped for the wrong reason: {reason}"
        ),
        Outcome::Pass | Outcome::PassWithNote(_) => {
            panic!("shallow repository passed the full-history secret scan")
        }
        Outcome::Fail(reason) => panic!("ordinary profile failed instead of skipping: {reason}"),
    }
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
    let registry = [CheckSpec {
        name: "synthetic".to_string(),
        profiles: vec![Profile::Full, Profile::Release],
        opt_in: None,
        action: Action::EmbeddedTarget {
            target: "thumb".to_string(),
        },
    }];

    let root = repo_root();
    let mut full = ctx(&root, Profile::Full);
    full.skip_embedded = true;
    assert_eq!(
        xtask::runner::run(&full, &registry, &[], false),
        0,
        "an ordinary run tolerates a SKIP"
    );
    let mut release = ctx(&root, Profile::Release);
    release.skip_embedded = true;
    assert_eq!(
        xtask::runner::run(&release, &registry, &[], false),
        1,
        "release evidence must record a would-be SKIP as a FAIL"
    );
}
