//! The code-size snapshot.
//!
//! Non-normative: not a guarantee, not total flash, and not WCET. It records
//! compiler-emitted function `.text` sizes for four named strategy pairings,
//! built from `tools/code-size` -- which the retired
//! `scripts/measure-code-size.sh` generated from a 136-line heredoc.
//!
//! This is the check issue #27 opened on. It failed on a clean Windows checkout
//! because tracked LF became CRLF while generated output stayed LF. The
//! comparison here is between two in-memory strings that were both read through
//! `text::read_text`, and the `line endings` check guards the cause.

use std::fs;
use std::path::{Path, PathBuf};

use crate::checks::embedded::TARGETS;
use crate::proc;
use crate::runner::{Ctx, Outcome};
use crate::text;

/// The four measured pairings: the exported symbol name, and the crate feature
/// that instantiates it.
const PAIRINGS: [(&str, &str); 4] = [
    ("ph_eval_binary", "pairing-binary"),
    ("ph_eval_linear", "pairing-linear"),
    ("ph_eval_mixed", "pairing-mixed"),
    ("ph_eval_uniform", "pairing-uniform"),
];

pub const SNAPSHOT: &str = "docs/code-size-snapshot.txt";

/// Everything the measurement needs, or the reason it cannot run.
fn llvm_nm(ctx: &Ctx) -> Result<PathBuf, String> {
    let sysroot = match proc::capture("rustc", &["--print", "sysroot"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        _ => return Err("could not resolve the rustc sysroot".to_string()),
    };
    let host = match proc::capture("rustc", &["-vV"], &ctx.root) {
        Ok(output) if output.ok() => output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .map(str::to_string)
            .ok_or_else(|| "rustc -vV did not report a host triple".to_string())?,
        _ => return Err("could not run rustc -vV".to_string()),
    };

    let bin = Path::new(&sysroot).join("lib/rustlib").join(&host).join("bin");
    for name in ["llvm-nm", "llvm-nm.exe"] {
        let candidate = bin.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "llvm-nm not found under {}; skipping \
         (rustup component add llvm-tools-preview)",
        bin.display()
    ))
}

/// Produce the snapshot text, or the reason the measurement cannot run.
pub fn measure(ctx: &Ctx) -> Result<String, String> {
    for target in TARGETS {
        match proc::target_installed(target, &ctx.root) {
            Ok(true) => {}
            _ => {
                return Err(format!(
                    "target {target} not installed; skipping (rustup target add {target})"
                ));
            }
        }
    }
    let nm = llvm_nm(ctx)?;

    let rustc_version = match proc::capture("rustc", &["--version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        _ => return Err("could not read the rustc version".to_string()),
    };

    let mut snapshot = String::new();
    snapshot.push_str("# ph-surfaces code-size snapshot (non-normative)\n");
    snapshot.push_str(&format!("# Toolchain: {rustc_version}\n"));
    snapshot.push_str(
        "# Profile: opt-level=s, lto=false, codegen-units=1, panic=abort, debug=false\n",
    );
    snapshot.push_str(
        "# Tool: llvm-nm --demangle --print-size \
         (single-pairing compiler object .text total)\n",
    );
    snapshot.push_str("# Pairings:\n");
    snapshot.push_str("#   ph_eval_binary   Binary\u{d7}Binary ELEVATION 5\u{d7}4\n");
    snapshot.push_str("#   ph_eval_linear   Linear\u{d7}Linear 3\u{d7}2\n");
    snapshot.push_str("#   ph_eval_uniform  Uniform\u{d7}Uniform 2\u{d7}2\n");
    snapshot
        .push_str("#   ph_eval_mixed    BucketedAxis<17, 8> \u{d7} UniformAxis<9, 0, 200>\n");
    snapshot.push_str("#\n");
    snapshot.push_str("# This is not a guarantee, not total flash, and not WCET.\n\n");

    for (index, target) in TARGETS.iter().enumerate() {
        if index > 0 {
            snapshot.push('\n');
        }
        snapshot.push_str(target);
        snapshot.push('\n');
        for (name, feature) in PAIRINGS {
            let size = pairing_size(ctx, &nm, target, name, feature)?;
            snapshot.push_str(&format!("{name} {size}\n"));
        }
    }

    Ok(snapshot)
}

fn pairing_size(
    ctx: &Ctx,
    nm: &Path,
    target: &str,
    name: &str,
    feature: &str,
) -> Result<String, String> {
    // Short scratch path: the shell version nested
    // target/<triple>/<pairing>/<triple>/release/deps, which approaches Windows
    // MAX_PATH from a deep worktree.
    let build_dir = ctx.path(&format!("target/xt/cs/{target}/{name}"));
    let build_dir_text = build_dir.display().to_string();

    match proc::run(
        &proc::cargo(),
        &[
            "rustc",
            "--release",
            "--target",
            target,
            "--no-default-features",
            "--features",
            feature,
            "--",
            "--emit=obj",
        ],
        &ctx.path("tools/code-size"),
        &[("CARGO_TARGET_DIR", build_dir_text.as_str())],
    ) {
        Ok(Some(0)) => {}
        Ok(_) => return Err(format!("the {name} measurement build failed")),
        Err(error) => return Err(format!("cargo could not run: {error}")),
    }

    let deps = build_dir.join(target).join("release/deps");
    let object = fs::read_dir(&deps)
        .map_err(|error| format!("{} is unreadable: {error}", deps.display()))?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|extension| extension == "o")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("ph_surfaces_code_size-"))
        })
        .ok_or_else(|| {
            "expected a ph_surfaces_code_size object after the measurement build".to_string()
        })?;

    text_size(nm, &object)
}

/// Sum the sizes of every defined text symbol in the object, as `%08x`.
fn text_size(nm: &Path, object: &Path) -> Result<String, String> {
    let listing = match proc::capture(
        &nm.display().to_string(),
        &[
            "--demangle",
            "--print-size",
            "--defined-only",
            &object.display().to_string(),
        ],
        object.parent().unwrap_or(Path::new(".")),
    ) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => return Err(format!("llvm-nm failed.\n{}", output.stderr)),
        Err(error) => return Err(format!("llvm-nm could not run: {error}")),
    };

    let mut total: u64 = 0;
    let mut found = false;
    for line in listing.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || !matches!(fields[2], "T" | "t") {
            continue;
        }
        let size = u64::from_str_radix(fields[1], 16)
            .map_err(|_| format!("llvm-nm reported a non-hexadecimal size: {line}"))?;
        total += size;
        found = true;
    }

    if !found {
        return Err(format!(
            "no text symbols found in {}\n{listing}",
            object.display()
        ));
    }
    Ok(format!("{total:08x}"))
}

pub fn code_size_snapshot(ctx: &Ctx) -> Outcome {
    if ctx.skip_embedded {
        return Outcome::skip("--skip-embedded; skipping code-size measurement");
    }

    let measured = match measure(ctx) {
        Ok(snapshot) => snapshot,
        Err(reason) => return Outcome::skip(reason),
    };
    let committed = match text::read_text(&ctx.path(SNAPSHOT)) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("{SNAPSHOT} is unreadable: {error}")),
    };

    if measured == committed {
        return Outcome::Pass;
    }

    let mut diff = String::new();
    let mut expected = committed.lines();
    let mut actual = measured.lines();
    loop {
        match (expected.next(), actual.next()) {
            (None, None) => break,
            (left, right) if left == right => continue,
            (left, right) => {
                if let Some(line) = left {
                    diff.push_str(&format!("- {line}\n"));
                }
                if let Some(line) = right {
                    diff.push_str(&format!("+ {line}\n"));
                }
            }
        }
    }

    Outcome::fail(format!(
        "{diff}code-size snapshot differs from {SNAPSHOT} (- committed, + measured).\n\
         Re-run `cargo xtask code-size --write` and commit the output."
    ))
}
