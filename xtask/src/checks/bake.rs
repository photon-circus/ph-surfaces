//! Implementation-line budget and packaged-artifact checks for the host baker.
//!
//! `crates/surfaces-bake/src` is capped at a declared number of
//! implementation lines. Tests (`#[cfg(test)]` tails, matching the
//! integer-only scanner), fixtures, and generated output directories are
//! excluded. Exceeding the cap is a FAIL, not a quiet raise.
//!
//! The baker is a second crates.io package; `baker_package` proves its archive
//! independently of the runtime `package *` checks.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use similar::TextDiff;

use super::package;
use crate::runner::{Ctx, Outcome};
use crate::text;

const EXCLUDED_DIR_NAMES: &[&str] = &["fixtures", "generated", "golden", "goldens", "out"];

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
            "{}: {total} implementation lines; the cap is {}.\n\
             Exceeding the baker line budget is a FAIL, not a quiet raise.",
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

    let listed = match baker_listed_files(ctx) {
        Ok(files) => files,
        Err(message) => return Outcome::Fail(message),
    };
    if listed != expected {
        return Outcome::fail(file_set_diff(
            &expected,
            &listed,
            "baker `cargo package --list` file set differs from the expected list.",
        ));
    }

    let artifact = match baker_artifact(ctx) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    let mut actual = match package::packaged_tree(&artifact.unpacked) {
        Ok(files) => files,
        Err(message) => return Outcome::Fail(message),
    };
    actual.sort();
    if actual != expected {
        return Outcome::fail(file_set_diff(
            &expected,
            &actual,
            "baker packaged file set differs from the expected list.",
        ));
    }
    println!("baker packaged files:");
    for file in &actual {
        println!("{file}");
    }

    let bytes = match fs::read(&artifact.archive) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Outcome::fail(format!("could not read the baker archive: {error}"));
        }
    };
    let digest = hex::encode(Sha256::digest(&bytes));
    match fs::read(&artifact.archive) {
        Ok(again) if hex::encode(Sha256::digest(&again)) == digest => {}
        Ok(_) => {
            return Outcome::fail(
                "baker package changed while its SHA-256 digest was being verified.",
            );
        }
        Err(error) => {
            return Outcome::fail(format!("could not re-read the baker archive: {error}"));
        }
    }
    println!("baker package SHA-256: {digest}");

    if ctx.strict() {
        if let Err(message) = baker_provenance(ctx, &artifact) {
            return Outcome::fail(message);
        }
    }
    Outcome::Pass
}

fn baker_listed_files(ctx: &Ctx) -> Result<Vec<String>, String> {
    let mut args = vec!["package", "-p", "ph-surfaces-bake", "--locked", "--list"];
    if !ctx.strict() {
        args.push("--allow-dirty");
    }
    let listing = match crate::proc::capture(&crate::proc::cargo(), &args, &ctx.root) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => {
            return Err(format!(
                "cargo package -p ph-surfaces-bake --list failed.\n{}",
                output.stderr
            ));
        }
        Err(error) => return Err(format!("cargo could not run: {error}")),
    };
    let mut files: Vec<String> = listing
        .lines()
        .map(|line| line.trim().replace('\\', "/"))
        .filter(|line| !line.is_empty())
        .collect();
    files.sort();
    Ok(files)
}

fn file_set_diff(expected: &[String], actual: &[String], message: &str) -> String {
    let expected = expected.join("\n") + "\n";
    let actual = actual.join("\n") + "\n";
    let diff = TextDiff::from_lines(&expected, &actual)
        .unified_diff()
        .header("expected", "actual")
        .to_string();
    format!("{diff}{message}")
}

struct BakerArtifact {
    archive: PathBuf,
    unpacked: PathBuf,
}

fn baker_artifact(ctx: &Ctx) -> Result<&'static BakerArtifact, String> {
    static BUILT: OnceLock<Result<BakerArtifact, String>> = OnceLock::new();
    BUILT
        .get_or_init(|| build_baker_artifact(ctx))
        .as_ref()
        .map_err(String::clone)
}

fn build_baker_artifact(ctx: &Ctx) -> Result<BakerArtifact, String> {
    let directory = format!("ph-surfaces-bake-{}", ctx.config.package.version);
    let archive = ctx.path(&format!("target/package/{directory}.crate"));
    let unpacked = ctx.path(&format!("target/package/{directory}"));
    let _ = fs::remove_file(&archive);

    let mut args = vec!["package", "-p", "ph-surfaces-bake", "--locked"];
    if !ctx.strict() {
        args.push("--allow-dirty");
    }
    match crate::proc::run(&crate::proc::cargo(), &args, &ctx.root, &[]) {
        Ok(Some(0)) => {}
        Ok(_) => return Err("cargo package -p ph-surfaces-bake failed.".to_string()),
        Err(error) => return Err(format!("cargo could not run: {error}")),
    }
    if !archive.is_file() {
        return Err(format!(
            "expected {} to exist after cargo package -p ph-surfaces-bake.",
            archive.display()
        ));
    }
    if !unpacked.is_dir() {
        return Err(format!(
            "expected cargo to leave a baker verification unpack at {}.",
            unpacked.display()
        ));
    }
    Ok(BakerArtifact { archive, unpacked })
}

fn baker_provenance(ctx: &Ctx, artifact: &BakerArtifact) -> Result<(), String> {
    #[derive(Deserialize)]
    struct VcsInfo {
        git: VcsGit,
    }
    #[derive(Deserialize)]
    struct VcsGit {
        sha1: String,
        dirty: Option<bool>,
    }

    let vcs_info = artifact.unpacked.join(".cargo_vcs_info.json");
    if !vcs_info.is_file() {
        return Err("baker packaged crate is missing .cargo_vcs_info.json.".to_string());
    }
    let info: VcsInfo = fs::read(&vcs_info)
        .map_err(serde_json::Error::io)
        .and_then(|bytes| serde_json::from_slice(&bytes))
        .map_err(|error| format!("baker .cargo_vcs_info.json is unreadable: {error}"))?;
    let head = match crate::proc::capture("git", &["rev-parse", "--verify", "HEAD"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        _ => {
            return Err("could not resolve HEAD to compare baker packaged provenance.".to_string());
        }
    };
    if info.git.sha1 != head {
        return Err(format!(
            "baker packaged VCS SHA does not match HEAD: expected {head}, found {}",
            if info.git.sha1.is_empty() {
                "<missing>"
            } else {
                &info.git.sha1
            }
        ));
    }
    if info.git.dirty == Some(true) {
        return Err("baker packaged VCS provenance is marked dirty.".to_string());
    }
    println!("baker package VCS provenance: {} (clean)", info.git.sha1);
    Ok(())
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
