//! The packaged artifact: its file set, its provenance, its digest, and a real
//! downstream consumer built against it.
//!
//! `scripts/ci.sh` did all of this in one ~410-line `check_package_build`, which
//! meant a missing bare-metal target reported SKIP *after* the packaging,
//! digest, docs and doctests had already succeeded -- real evidence discarded
//! and recorded as "not run". Splitting it means only the part that actually
//! needs a target can skip.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::checks::cargo::EXAMPLES;
use crate::checks::embedded::TARGETS;
use crate::proc;
use crate::runner::{Ctx, Outcome};
use crate::sha256;
use crate::text;

pub const PACKAGE_NAME: &str = "ph-surfaces";
pub const PACKAGE_VERSION: &str = "0.1.0-incubating.1";

/// The exact file set the packaged artifact must contain, sorted.
///
/// `.cargo_vcs_info.json`, `Cargo.lock`, and `Cargo.toml.orig` are added by
/// cargo itself; everything else comes from the `include` allowlist in
/// `Cargo.toml`. This is an allowlist: a stray file is as much a failure as a
/// missing one.
pub const PACKAGED_FILES: [&str; 23] = [
    ".cargo_vcs_info.json",
    "Cargo.lock",
    "Cargo.toml",
    "Cargo.toml.orig",
    "LICENSE",
    "README.md",
    "examples/fail_safe_boundaries.rs",
    "examples/firmware_cost_budget.rs",
    "examples/firmware_quickstart.rs",
    "examples/mixed_calibration_map.rs",
    "examples/uniform_sensor_compensation.rs",
    "src/axis/binary.rs",
    "src/axis/bucketed.rs",
    "src/axis/linear.rs",
    "src/axis/mod.rs",
    "src/axis/uniform.rs",
    "src/boundary.rs",
    "src/error.rs",
    "src/evaluate.rs",
    "src/interp.rs",
    "src/lib.rs",
    "src/lookup.rs",
    "src/surface.rs",
];

/// Files a consumer has no use for and that must never reach the archive.
const NON_CONSUMER_PREFIXES: [&str; 10] = [
    "AGENTS.md",
    "CHANGELOG.md",
    "clippy.toml",
    "deny.toml",
    "rust-toolchain.toml",
    "scripts/",
    "tools/",
    "docs/",
    "tests/",
    ".github/",
];

pub struct Artifact {
    /// The `.crate` archive.
    pub archive: PathBuf,
    /// Cargo's own verification unpack of that archive.
    pub unpacked: PathBuf,
}

/// Build the archive once per run, so `--only 'package digest'` works on its own
/// without four checks each re-packaging the crate.
fn artifact(ctx: &Ctx) -> Result<&'static Artifact, String> {
    static BUILT: OnceLock<Result<Artifact, String>> = OnceLock::new();
    BUILT.get_or_init(|| build_artifact(ctx)).as_ref().map_err(String::clone)
}

fn build_artifact(ctx: &Ctx) -> Result<Artifact, String> {
    clean_release_tree(ctx)?;

    let directory = format!("{PACKAGE_NAME}-{PACKAGE_VERSION}");
    let archive = ctx.path(&format!("target/package/{directory}.crate"));
    let unpacked = ctx.path(&format!("target/package/{directory}"));

    // Remove the exact generated archive first: cargo reuses this fixed
    // destination, and a shorter rebuild could otherwise retain trailing bytes
    // from an older artifact on a platform with non-atomic replacement.
    let _ = fs::remove_file(&archive);

    // Cargo's own verify step unpacks the archive and builds it, so a source
    // file missing from `include` fails right here.
    let mut args = vec!["package", "--locked"];
    if !ctx.strict() {
        args.push("--allow-dirty");
    }
    match proc::run(&proc::cargo(), &args, &ctx.root, &[]) {
        Ok(Some(0)) => {}
        Ok(_) => return Err("cargo package failed.".to_string()),
        Err(error) => return Err(format!("cargo package could not run: {error}")),
    }

    if !archive.is_file() {
        return Err(format!(
            "expected {} to exist after cargo package.",
            archive.display()
        ));
    }
    if !unpacked.is_dir() {
        return Err(format!(
            "expected cargo to leave a verification unpack at {}.",
            unpacked.display()
        ));
    }

    Ok(Artifact { archive, unpacked })
}

/// Release evidence must describe a commit, not a desk.
fn clean_release_tree(ctx: &Ctx) -> Result<(), String> {
    if !ctx.strict() {
        return Ok(());
    }
    match proc::capture("git", &["rev-parse", "--verify", "HEAD"], &ctx.root) {
        Ok(output) if output.ok() => {}
        _ => {
            return Err(
                "strict package verification requires a Git checkout with a HEAD commit."
                    .to_string(),
            );
        }
    }
    match proc::capture(
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
        &ctx.root,
    ) {
        Ok(output) if output.ok() && output.stdout.trim().is_empty() => Ok(()),
        Ok(output) if output.ok() => Err(format!(
            "strict package verification requires a clean Git worktree:\n{}",
            output.stdout
        )),
        _ => Err("git status failed.".to_string()),
    }
}

/// What `cargo package --list` says will ship.
pub fn package_list(ctx: &Ctx) -> Outcome {
    if let Err(message) = clean_release_tree(ctx) {
        return Outcome::Fail(message);
    }

    let mut args = vec!["package", "--locked", "--list"];
    if !ctx.strict() {
        args.push("--allow-dirty");
    }
    let listing = match proc::capture(&proc::cargo(), &args, &ctx.root) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => return Outcome::fail(format!("cargo package --list failed.\n{}", output.stderr)),
        Err(error) => return Outcome::fail(format!("cargo could not run: {error}")),
    };

    // Cargo prints native separators on Windows; compare in one spelling.
    let files: Vec<String> = listing
        .lines()
        .map(|line| line.trim().replace('\\', "/"))
        .filter(|line| !line.is_empty())
        .collect();
    for file in &files {
        println!("{file}");
    }

    for required in PACKAGED_FILES {
        // Cargo synthesizes these three; they are not tracked sources, so
        // `--list` reports them but the required-source check is about the
        // allowlist.
        if matches!(
            required,
            ".cargo_vcs_info.json" | "Cargo.lock" | "Cargo.toml.orig"
        ) {
            continue;
        }
        if !files.iter().any(|file| file == required) {
            return Outcome::fail(format!("packaged crate is missing {required}"));
        }
    }

    let strays: Vec<&String> = files
        .iter()
        .filter(|file| {
            NON_CONSUMER_PREFIXES
                .iter()
                .any(|prefix| file.starts_with(prefix))
        })
        .collect();
    if !strays.is_empty() {
        let listed: Vec<String> = strays.iter().map(|file| (*file).clone()).collect();
        return Outcome::fail(format!(
            "{}\npackaged crate contains non-consumer artifacts.",
            listed.join("\n")
        ));
    }

    Outcome::Pass
}

/// Cargo's verification unpack of the archive, not `--list`: the file set must
/// match exactly. Walking that tree keeps the check free of host `tar`.
pub fn package_build(ctx: &Ctx) -> Outcome {
    let artifact = match artifact(ctx) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    let mut actual = match packaged_tree(&artifact.unpacked) {
        Ok(files) => files,
        Err(message) => return Outcome::Fail(message),
    };
    actual.sort();

    let mut expected: Vec<String> = PACKAGED_FILES.iter().map(|file| file.to_string()).collect();
    expected.sort();

    if actual != expected {
        let missing: Vec<&String> = expected
            .iter()
            .filter(|file| !actual.contains(file))
            .collect();
        let unexpected: Vec<&String> = actual
            .iter()
            .filter(|file| !expected.contains(file))
            .collect();
        let mut message = String::new();
        for file in missing {
            message.push_str(&format!("- {file}\n"));
        }
        for file in unexpected {
            message.push_str(&format!("+ {file}\n"));
        }
        message
            .push_str("packaged file set differs from the expected list (- expected, + actual).");
        return Outcome::Fail(message);
    }

    println!("packaged files:");
    for file in &actual {
        println!("{file}");
    }
    Outcome::Pass
}

/// Files in cargo's verification unpack, excluding the `target/` directory the
/// verify build may leave behind. That directory is not part of the crate.
fn packaged_tree(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    collect_packaged_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_packaged_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("could not read {}: {error}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry under {}: {error}",
                current.display()
            )
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("{} is not under {}", path.display(), root.display()))?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        if relative == "target" || relative.starts_with("target/") {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not stat {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_packaged_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(relative);
        } else {
            return Err(format!(
                "packaged crate contains a non-file entry at {relative}"
            ));
        }
    }
    Ok(())
}

/// The archive digest, computed here rather than by whichever hashing utility a
/// machine happens to carry.
pub fn package_digest(ctx: &Ctx) -> Outcome {
    let artifact = match artifact(ctx) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    let bytes = match fs::read(&artifact.archive) {
        Ok(bytes) => bytes,
        Err(error) => return Outcome::fail(format!("could not read the packaged archive: {error}")),
    };
    let digest = sha256::hex(&bytes);

    // Re-read and re-hash: a digest that describes a file which changed while it
    // was being measured is not evidence.
    match fs::read(&artifact.archive) {
        Ok(again) if sha256::hex(&again) == digest => {}
        Ok(_) => {
            return Outcome::fail("package changed while its SHA-256 digest was being verified.");
        }
        Err(error) => return Outcome::fail(format!("could not re-read the archive: {error}")),
    }

    println!("package SHA-256: {digest}");
    Outcome::Pass
}

/// The packaged commit must be the commit under review.
pub fn package_provenance(ctx: &Ctx) -> Outcome {
    let artifact = match artifact(ctx) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    let vcs_info = artifact.unpacked.join(".cargo_vcs_info.json");
    if !vcs_info.is_file() {
        return Outcome::fail("packaged crate is missing .cargo_vcs_info.json.");
    }
    let info = match text::read_text(&vcs_info) {
        Ok(info) => info,
        Err(error) => return Outcome::fail(format!(".cargo_vcs_info.json is unreadable: {error}")),
    };

    let head = match proc::capture("git", &["rev-parse", "--verify", "HEAD"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        _ => return Outcome::fail("could not resolve HEAD to compare packaged provenance."),
    };

    let packaged = json_string_field(&info, "sha1").unwrap_or_default();
    if packaged != head {
        return Outcome::fail(format!(
            "packaged VCS SHA does not match HEAD: expected {head}, found {}",
            if packaged.is_empty() {
                "<missing>"
            } else {
                &packaged
            }
        ));
    }

    // Cargo 1.94 omits the member entirely for a clean tree, so its absence is
    // correct. Only an explicit `true` is a finding.
    if json_bool_field(&info, "dirty") == Some(true) {
        return Outcome::fail("packaged VCS provenance is marked dirty.");
    }

    println!("package VCS provenance: {packaged} (clean)");
    Outcome::Pass
}

fn json_string_field(text: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}\"");
    let after = text.split_once(&needle)?.1;
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let value = after.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn json_bool_field(text: &str, name: &str) -> Option<bool> {
    let needle = format!("\"{name}\"");
    let after = text.split_once(&needle)?.1;
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Prove the artifact from the artifact: its own rustdoc, doctests, and
/// examples, then a real downstream `#![no_std]` crate built against it.
pub fn package_consumer(ctx: &Ctx) -> Outcome {
    let artifact = match artifact(ctx) {
        Ok(artifact) => artifact,
        Err(message) => return Outcome::Fail(message),
    };

    // Keep the scratch path short: a deep worktree plus cargo's own nesting
    // approaches Windows MAX_PATH.
    let scratch = ctx.path("target/xt/consumer");
    let target_dir = scratch.join("t");
    let _ = fs::remove_dir_all(&scratch);
    if let Err(error) = fs::create_dir_all(&scratch) {
        return Outcome::fail(format!("could not create {}: {error}", scratch.display()));
    }
    let target_dir_text = target_dir.display().to_string();
    let env: Vec<(&str, &str)> = vec![("CARGO_TARGET_DIR", target_dir_text.as_str())];

    // 1. The unpacked crate proves its own documentation from the packaged
    //    files alone: rustdoc with warnings denied, every doctest (README Rust
    //    blocks included, because README.md ships and src/lib.rs compiles it),
    //    and every Cargo example. The unpacked crate has no tests/ directory,
    //    so `cargo test --doc` there is unit tests plus doctests only.
    let cargo = proc::cargo();
    let mut doc_env = env.clone();
    doc_env.push(("RUSTDOCFLAGS", "-D warnings"));
    if let outcome @ Outcome::Fail(_) = run_in(
        &artifact.unpacked,
        &cargo,
        &["doc", "--offline", "--no-deps"],
        &doc_env,
    ) {
        return outcome;
    }
    if let outcome @ Outcome::Fail(_) = run_in(
        &artifact.unpacked,
        &cargo,
        &["test", "--offline", "--doc"],
        &env,
    ) {
        return outcome;
    }
    for example in EXAMPLES {
        if let outcome @ Outcome::Fail(_) = run_in(
            &artifact.unpacked,
            &cargo,
            &["run", "--offline", "--example", example],
            &env,
        ) {
            return outcome;
        }
    }

    // 2. A fresh downstream `#![no_std]` consumer, compiled against the
    //    unpacked artifact. `tools/consumer` is real checked-in source -- the
    //    shell gate generated it from a 279-line heredoc that could never be
    //    formatted or linted. Only its dependency path is rewritten here.
    let consumer = scratch.join("c");
    if let Err(error) = copy_consumer(ctx, &consumer, &artifact.unpacked) {
        return Outcome::fail(error);
    }

    if let outcome @ Outcome::Fail(_) = run_in(&consumer, &cargo, &["build", "--offline"], &env) {
        return outcome;
    }
    if let outcome @ Outcome::Fail(_) = run_in(&consumer, &cargo, &["test", "--offline"], &env) {
        return outcome;
    }

    // The consumer's fresh lockfile may name only the two packages: the
    // packaged crate really has no runtime dependency.
    match text::read_text(&consumer.join("Cargo.lock")) {
        Ok(lock) => {
            let allowed = [
                format!("name = \"{PACKAGE_NAME}\""),
                "name = \"ph-surfaces-consumer\"".to_string(),
            ];
            let unexpected: Vec<&str> = lock
                .lines()
                .filter(|line| line.starts_with("name = "))
                .filter(|line| !allowed.iter().any(|name| name == line.trim()))
                .collect();
            if !unexpected.is_empty() {
                return Outcome::fail(format!(
                    "{}\nthe downstream consumer resolved an unexpected package.",
                    unexpected.join("\n")
                ));
            }
        }
        Err(error) => return Outcome::fail(format!("consumer Cargo.lock is unreadable: {error}")),
    }

    // 3. A core-only consumer build is what proves the sixteen strategy pairings
    //    themselves are allocation-free: the library's own core-only build
    //    compiles no instantiation of them, because a generic axis strategy is
    //    only monomorphised where a surface names it.
    let mut skipped: Vec<String> = Vec::new();
    let core_only = match proc::component_installed(&ctx.nightly, "rust-src", &ctx.root) {
        Ok(true) => true,
        _ => {
            skipped.push(format!(
                "{} with rust-src missing; core-only pairing proof skipped",
                ctx.nightly
            ));
            false
        }
    };

    for target in TARGETS {
        match proc::target_installed(target, &ctx.root) {
            Ok(true) => {}
            _ => {
                skipped.push(format!(
                    "target {target} not installed; package target proof skipped"
                ));
                continue;
            }
        }
        if let outcome @ Outcome::Fail(_) = run_in(
            &consumer,
            &cargo,
            &["build", "--offline", "--target", target],
            &env,
        ) {
            return outcome;
        }
        if core_only
            && let outcome @ Outcome::Fail(_) = run_in(
                &consumer,
                "rustup",
                &[
                    "run",
                    &ctx.nightly,
                    "cargo",
                    "build",
                    "--offline",
                    "--target",
                    target,
                    "-Z",
                    "build-std=core",
                ],
                &env,
            )
        {
            return outcome;
        }
    }

    if skipped.is_empty() {
        Outcome::Pass
    } else {
        Outcome::skip(skipped.join("\n"))
    }
}

fn run_in(directory: &Path, program: &str, args: &[&str], env: &[(&str, &str)]) -> Outcome {
    match proc::run(program, args, directory, env) {
        Ok(Some(0)) => Outcome::Pass,
        Ok(Some(code)) => Outcome::fail(format!("{program} {} exited {code}", args.join(" "))),
        Ok(None) => Outcome::fail(format!("{program} {} was terminated", args.join(" "))),
        Err(error) => Outcome::fail(format!("{program} could not run: {error}")),
    }
}

/// Copy `tools/consumer` next to the unpacked crate, repointing its path
/// dependency at the artifact under test.
fn copy_consumer(ctx: &Ctx, destination: &Path, unpacked: &Path) -> Result<(), String> {
    let source = ctx.path("tools/consumer");
    fs::create_dir_all(destination.join("src"))
        .map_err(|error| format!("could not create the consumer scratch: {error}"))?;

    let manifest = text::read_text(&source.join("Cargo.toml"))
        .map_err(|error| format!("tools/consumer/Cargo.toml is unreadable: {error}"))?;
    let vendored = unpacked.display().to_string().replace('\\', "/");
    let manifest = manifest.replace(
        "ph-surfaces = { path = \"../..\" }",
        &format!("ph-surfaces = {{ path = \"{vendored}\" }}"),
    );
    fs::write(destination.join("Cargo.toml"), manifest)
        .map_err(|error| format!("could not write the consumer manifest: {error}"))?;

    let lib = text::read_text(&source.join("src/lib.rs"))
        .map_err(|error| format!("tools/consumer/src/lib.rs is unreadable: {error}"))?;
    fs::write(destination.join("src/lib.rs"), lib)
        .map_err(|error| format!("could not write the consumer source: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn packaged_tree_lists_files_and_ignores_verify_target() {
        let root = std::env::temp_dir().join(format!(
            "ph-surfaces-packaged-tree-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        File::create(root.join("Cargo.toml")).unwrap();
        File::create(root.join("src/lib.rs")).unwrap();
        File::create(root.join("target/debug/dummy")).unwrap();

        let mut files = packaged_tree(&root).unwrap();
        files.sort();
        assert_eq!(files, ["Cargo.toml", "src/lib.rs"]);

        let _ = fs::remove_dir_all(&root);
    }
}
