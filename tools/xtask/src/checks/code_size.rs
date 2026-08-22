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

/// Locate one `llvm-tools-preview` binary in the sysroot, or the reason the
/// measurement cannot run.
fn llvm_tool(ctx: &Ctx, tool: &str) -> Result<PathBuf, String> {
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

    let bin = Path::new(&sysroot)
        .join("lib/rustlib")
        .join(&host)
        .join("bin");
    for name in [tool.to_string(), format!("{tool}.exe")] {
        let candidate = bin.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "{tool} not found under {}; skipping \
         (rustup component add llvm-tools-preview)",
        bin.display()
    ))
}

fn llvm_nm(ctx: &Ctx) -> Result<PathBuf, String> {
    llvm_tool(ctx, "llvm-nm")
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
    snapshot
        .push_str("# Profile: opt-level=s, lto=false, codegen-units=1, panic=abort, debug=false\n");
    snapshot.push_str(
        "# Tool: llvm-nm --demangle --print-size \
         (single-pairing compiler object .text total)\n",
    );
    snapshot.push_str("# Pairings:\n");
    snapshot.push_str("#   ph_eval_binary   Binary\u{d7}Binary ELEVATION 5\u{d7}4\n");
    snapshot.push_str("#   ph_eval_linear   Linear\u{d7}Linear 3\u{d7}2\n");
    snapshot.push_str("#   ph_eval_uniform  Uniform\u{d7}Uniform 2\u{d7}2\n");
    snapshot.push_str("#   ph_eval_mixed    BucketedAxis<17, 8> \u{d7} UniformAxis<9, 0, 200>\n");
    snapshot.push_str(
        "#   ph_interp_kernel shared scalar interpolation, measured from the\n\
         #                    ph-surfaces rlib: non-generic, so it is not in the\n\
         #                    per-pairing objects above and is paid once, not per\n\
         #                    pairing\n",
    );
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
        let rlib = ph_surfaces_rlib(ctx, target, PAIRINGS[0].0)?;
        let size = kernel_size(&nm, &rlib)?;
        snapshot.push_str(&format!("ph_interp_kernel {size}\n"));
    }

    Ok(snapshot)
}

/// Build one pairing's measurement object and return its path.
fn pairing_object(ctx: &Ctx, target: &str, name: &str, feature: &str) -> Result<PathBuf, String> {
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
    fs::read_dir(&deps)
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
        })
}

fn pairing_size(
    ctx: &Ctx,
    nm: &Path,
    target: &str,
    name: &str,
    feature: &str,
) -> Result<String, String> {
    let object = pairing_object(ctx, target, name, feature)?;
    text_size(nm, &object)
}

/// The `ph-surfaces` rlib a pairing was linked against.
///
/// Non-generic items -- the `interp` kernel above all -- are compiled into
/// the dependency rlib, not re-instantiated in the measurement object, so a
/// per-object `.text` total alone would silently exclude the shared scalar
/// interpolation (and its 64-bit division). The kernel is measured and
/// disassembled from here.
fn ph_surfaces_rlib(ctx: &Ctx, target: &str, name: &str) -> Result<PathBuf, String> {
    let deps = ctx
        .path(&format!("target/xt/cs/{target}/{name}"))
        .join(target)
        .join("release/deps");
    fs::read_dir(&deps)
        .map_err(|error| format!("{} is unreadable: {error}", deps.display()))?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|file| file.to_str())
                .is_some_and(|file| file.starts_with("libph_surfaces-") && file.ends_with(".rlib"))
        })
        .ok_or_else(|| "expected a libph_surfaces rlib next to the measurement object".to_string())
}

/// Sum the sizes of the `ph_surfaces::interp` text symbols in the rlib, as
/// `%08x`. `llvm-nm --demangle` reads archives, printing one block per member.
fn kernel_size(nm: &Path, rlib: &Path) -> Result<String, String> {
    let listing = match proc::capture(
        &nm.display().to_string(),
        &[
            "--demangle",
            "--print-size",
            "--defined-only",
            &rlib.display().to_string(),
        ],
        rlib.parent().unwrap_or(Path::new(".")),
    ) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => return Err(format!("llvm-nm failed on the rlib.\n{}", output.stderr)),
        Err(error) => return Err(format!("llvm-nm could not run: {error}")),
    };

    let mut total: u64 = 0;
    let mut found = false;
    for line in listing.lines() {
        if !line.contains("ph_surfaces::interp::") {
            continue;
        }
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
            "no ph_surfaces::interp text symbols found in {}",
            rlib.display()
        ));
    }
    Ok(format!("{total:08x}"))
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

/// Where one target's emitted-instruction snapshot lives.
pub fn asm_snapshot_path(target: &str) -> String {
    format!("docs/asm-snapshot-{target}.txt")
}

/// Produce the per-target emitted-instruction snapshots, or the reason the
/// measurement cannot run.
///
/// Informational, not a gate: the committed files make instruction-level
/// changes — a new branch on the hot path, a library call appearing, a
/// select turning into a jump — visible in ordinary review diffs whenever
/// `cargo xtask asm --write` is re-run, without blocking a toolchain bump on
/// instruction scheduling noise.
pub fn emit_asm(ctx: &Ctx) -> Result<Vec<(String, String)>, String> {
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
    let objdump = llvm_tool(ctx, "llvm-objdump")?;

    let rustc_version = match proc::capture("rustc", &["--version"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        _ => return Err("could not read the rustc version".to_string()),
    };

    let mut snapshots: Vec<(String, String)> = Vec::new();
    for target in TARGETS {
        let mut snapshot = String::new();
        snapshot.push_str("# ph-surfaces emitted-instruction snapshot (non-normative)\n");
        snapshot.push_str(&format!("# Target: {target}\n"));
        snapshot.push_str(&format!("# Toolchain: {rustc_version}\n"));
        snapshot.push_str(
            "# Profile: opt-level=s, lto=false, codegen-units=1, panic=abort, debug=false\n",
        );
        snapshot.push_str(
            "# Tool: llvm-objdump -d -r --demangle --no-show-raw-insn \
             (single-pairing compiler object); relocation lines name the\n\
             # out-of-object callees behind otherwise-anonymous branch targets\n",
        );
        snapshot.push_str(
            "# This is not a timing, WCET, or branch-freedom guarantee; it is the\n\
             # instruction stream to review when one of those properties matters.\n",
        );
        for (name, feature) in PAIRINGS {
            let object = pairing_object(ctx, target, name, feature)?;
            snapshot.push_str(&format!("\n## {name} ({feature})\n"));
            snapshot.push_str(&disassemble(&objdump, &object)?);
        }
        // The scalar kernel -- `interpolate_segment` and the rounding
        // division, with the crate's only 64-bit arithmetic -- is non-generic
        // and lives in the ph-surfaces rlib, not in the pairing objects.
        let rlib = ph_surfaces_rlib(ctx, target, PAIRINGS[0].0)?;
        snapshot.push_str("\n## ph_interp_kernel (ph_surfaces::interp, from the rlib)\n");
        let kernel = disassemble(&objdump, &rlib)?;
        snapshot.push_str(&only_interp_sections(&kernel));
        snapshots.push((asm_snapshot_path(target), snapshot));
    }
    Ok(snapshots)
}

/// Keep only the `ph_surfaces::interp` sections of an rlib disassembly. The
/// archive listing interleaves per-member headers naming absolute paths;
/// section-block filtering drops those along with every unrelated section.
fn only_interp_sections(listing: &str) -> String {
    let mut kept = String::new();
    let mut keeping = false;
    for line in listing.lines() {
        if line.starts_with("Disassembly of section") {
            keeping = line.contains("ph_surfaces6interp") || line.contains("ph_surfaces::interp");
        } else if line.contains("file format") {
            // A new archive member's header; its line names an absolute path.
            keeping = false;
        }
        if keeping {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    kept
}

/// The object's disassembly with the machine-specific preamble (which embeds
/// an absolute object path) dropped, so the snapshot is checkout-independent.
fn disassemble(objdump: &Path, object: &Path) -> Result<String, String> {
    let listing = match proc::capture(
        &objdump.display().to_string(),
        &[
            "-d",
            "-r",
            "--demangle",
            "--no-show-raw-insn",
            &object.display().to_string(),
        ],
        object.parent().unwrap_or(Path::new(".")),
    ) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => return Err(format!("llvm-objdump failed.\n{}", output.stderr)),
        Err(error) => return Err(format!("llvm-objdump could not run: {error}")),
    };

    let mut body = String::new();
    let mut seen_disassembly = false;
    for line in listing.lines() {
        if line.starts_with("Disassembly of section") {
            seen_disassembly = true;
        }
        if seen_disassembly {
            body.push_str(line.trim_end());
            body.push('\n');
        }
    }
    if !seen_disassembly {
        return Err(format!(
            "llvm-objdump printed no disassembly for {}",
            object.display()
        ));
    }
    Ok(body)
}

/// The comparable measurement body: target and `ph_eval_*` size lines only.
///
/// The `#` header records provenance -- toolchain, profile, tool -- and is
/// refreshed by `cargo xtask code-size --write`. It is deliberately not part
/// of the gate's equality: a toolchain bump that leaves every measured size
/// unchanged must not fail the check on the header's version string alone.
fn measurements(snapshot: &str) -> Vec<&str> {
    snapshot
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect()
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

    let measured_body = measurements(&measured);
    let committed_body = measurements(&committed);

    if measured_body == committed_body {
        if measured != committed {
            println!(
                "note: every measured size matches, but the snapshot header is stale; \
                 re-run `cargo xtask code-size --write` to refresh its provenance."
            );
        }
        return Outcome::Pass;
    }

    let mut diff = String::new();
    let mut expected = committed_body.iter();
    let mut actual = measured_body.iter();
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
        "{diff}code-size measurements differ from {SNAPSHOT} (- committed, + measured).\n\
         Re-run `cargo xtask code-size --write` and commit the output."
    ))
}
