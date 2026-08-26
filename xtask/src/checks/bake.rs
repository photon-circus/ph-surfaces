//! Implementation-line budget and packaged-artifact checks for the host baker.
//!
//! `crates/surfaces-bake/src` is capped at a declared number of
//! implementation lines (`max_implementation_lines` in `xtask/config.ron`).
//! Tests (`#[cfg(test)]` tails, matching the integer-only scanner), fixtures,
//! and generated output directories are excluded. The declared number may
//! move when the baker needs it; the check exists to prevent unbounded
//! growth. Exceeding the current declared cap without bumping it is a FAIL.
//!
//! The baker is a second crates.io package; `baker_package` proves its archive
//! independently of the runtime `package *` checks.

use std::path::Path;

use super::package;
use crate::runner::{Ctx, Outcome};
use crate::text;

const EXCLUDED_DIR_NAMES: &[&str] = &["fixtures", "generated", "golden", "goldens", "out"];

const BAKER_PACKAGE: &str = "ph-surfaces-bake";

/// Count implementation lines under the configured baker `src` and compare
/// them to the cap.
pub fn baker_line_budget(ctx: &Ctx) -> Outcome {
    let baker = &ctx.config.baker;
    let src = ctx.path(&baker.src);
    if !src.is_dir() {
        return Outcome::fail(format!(
            "{} is missing; the baker crate floor must exist.",
            baker.src
        ));
    }

    let sources = match text::rust_sources(&src) {
        Ok(sources) => sources,
        Err(error) => return Outcome::fail(format!("{} is unreadable: {error}", baker.src)),
    };

    let mut total = 0usize;
    let mut counted_files = 0usize;
    for path in sources {
        if path_is_excluded(&src, &path) {
            continue;
        }
        let source = match text::read_text(&path) {
            Ok(source) => source,
            Err(error) => {
                return Outcome::fail(format!("{} is unreadable: {error}", path.display()));
            }
        };
        let relative = path
            .strip_prefix(&ctx.root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let count = match text::implementation_line_count(&relative, &source) {
            Ok(count) => count,
            Err(error) => {
                return Outcome::fail(error);
            }
        };
        counted_files += 1;
        total = total.saturating_add(count);
    }

    if counted_files == 0 {
        return Outcome::fail(format!(
            "{} contains no implementation sources to budget.",
            baker.src
        ));
    }

    println!(
        "baker line budget: {total} / {} implementation lines in {}",
        baker.max_implementation_lines, baker.src
    );

    if total > baker.max_implementation_lines {
        return Outcome::fail(format!(
            "{}: {total} implementation lines; the declared cap is {}.\n\
             Exceeding it without bumping max_implementation_lines in xtask/config.ron is a FAIL.",
            baker.src, baker.max_implementation_lines
        ));
    }
    Outcome::Pass
}

fn path_is_excluded(src: &Path, path: &Path) -> bool {
    path.strip_prefix(src)
        .ok()
        .and_then(|relative| relative.parent())
        .is_some_and(|parent| {
            parent.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|name| EXCLUDED_DIR_NAMES.contains(&name))
            })
        })
}

/// Packaged baker artifact: file set, Cargo verification unpack, digest, and
/// (in the release profile) VCS provenance.
///
/// The runtime `package *` checks stay on `ph-surfaces`. This is the matching
/// evidence for the second shipped crate; it does not run the firmware consumer.
pub fn baker_package(ctx: &Ctx) -> Outcome {
    if let Err(message) = package::clean_release_tree(ctx) {
        return Outcome::Fail(message);
    }

    let mut expected = ctx.config.baker.files.clone();
    expected.sort();

    let mut listed = match package::listed_files(ctx, BAKER_PACKAGE) {
        Ok(files) => files,
        Err(message) => return Outcome::Fail(message),
    };
    listed.sort();
    if listed != expected {
        return Outcome::fail(package::file_set_diff(
            &expected,
            &listed,
            "baker `cargo package --list` file set differs from the expected list.",
        ));
    }

    // Package the runtime crate in the same invocation: the baker's pinned
    // `ph-surfaces` dev-dependency must resolve while cargo prepares the
    // archive, and on a version bump the new runtime version exists only in
    // this workspace, not yet on crates.io. Cargo satisfies the requirement
    // from the sibling packaged alongside it.
    let artifact = match package::package_artifact(
        ctx,
        &[&ctx.config.package.name, BAKER_PACKAGE],
        BAKER_PACKAGE,
        &ctx.config.baker.version,
    ) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    let mut actual = match package::packaged_tree(&artifact.unpacked) {
        Ok(files) => files,
        Err(message) => return Outcome::Fail(message),
    };
    actual.sort();
    if actual != expected {
        return Outcome::fail(package::file_set_diff(
            &expected,
            &actual,
            "baker packaged file set differs from the expected list.",
        ));
    }
    println!("baker packaged files:");
    for file in &actual {
        println!("{file}");
    }

    let digest = match package::archive_digest(&artifact.archive, "baker package") {
        Ok(digest) => digest,
        Err(message) => return Outcome::fail(message),
    };
    println!("baker package SHA-256: {digest}");

    if ctx.strict() {
        match package::verify_provenance(ctx, &artifact.unpacked, "baker packaged") {
            Ok(packaged) => println!("baker package VCS provenance: {packaged} (clean)"),
            Err(message) => return Outcome::fail(message),
        }
    }
    Outcome::Pass
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fixture_and_generated_directories_are_excluded() {
        let src = PathBuf::from("crates/surfaces-bake/src");
        assert!(path_is_excluded(&src, &src.join("fixtures/sample.rs")));
        assert!(path_is_excluded(&src, &src.join("generated/out.rs")));
        assert!(!path_is_excluded(&src, &src.join("lib.rs")));
        assert!(!path_is_excluded(&src, &src.join("ingest.rs")));
    }
}
