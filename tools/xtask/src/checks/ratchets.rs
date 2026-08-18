//! Source and manifest ratchets.
//!
//! `cargo test` cannot observe the *absence* of a code path, so these checks are
//! the mechanical evidence for the crate's hard invariants: no `ph-curves`, an
//! unconditional `#![no_std]`, and integer-only, core-only, unsafe-free runtime
//! code. The matching proof for the allocator is the nightly `-Z build-std=core`
//! build, not anything here.

use crate::proc;
use crate::runner::{Ctx, Outcome};
use crate::text::{self, Line, Separator};

/// Report every offending line the way `grep -n` did, then fail with `summary`.
fn reject(summary: &str, hits: Vec<&Line>) -> Outcome {
    let mut message = String::new();
    for hit in hits {
        message.push_str(&hit.render());
        message.push('\n');
    }
    message.push_str(summary);
    Outcome::Fail(message)
}

fn first<'a>(lines: &'a [Line], predicate: impl Fn(&str) -> bool) -> Vec<&'a Line> {
    lines
        .iter()
        .filter(|line| predicate(&line.text))
        .take(20)
        .collect()
}

pub fn no_std_unconditional(ctx: &Ctx) -> Outcome {
    let lib = match text::read_text(&ctx.path("src/lib.rs")) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("src/lib.rs is unreadable: {error}")),
    };

    if lib
        .lines()
        .any(|line| line.starts_with("#![cfg_attr(") && line.contains("no_std"))
    {
        return Outcome::fail("src/lib.rs: #![no_std] is feature-conditional.");
    }
    if !lib.lines().any(|line| line == "#![no_std]") {
        return Outcome::fail("src/lib.rs must declare an unconditional #![no_std].");
    }

    // Cargo unifies features across the graph, so the only way to be sure no
    // feature can relax `no_std` is for there to be no feature and no cfg on
    // one. A `[features]` table must arrive together with a proof that none of
    // its members touches the crate-level attribute.
    let manifest = match text::read_text(&ctx.path("Cargo.toml")) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("Cargo.toml is unreadable: {error}")),
    };
    if manifest.lines().any(|line| line.starts_with("[features]")) {
        return Outcome::fail("Cargo.toml declares a [features] table; none exists on this crate.");
    }

    let code = match text::code_lines(&ctx.root, &["src"]) {
        Ok(lines) => lines,
        Err(error) => return Outcome::fail(format!("could not read src: {error}")),
    };

    let hits = first(&code, |line| {
        line.match_indices("cfg_attr")
            .any(|(at, _)| line[at..].contains("no_std"))
    });
    if !hits.is_empty() {
        return reject("src: no_std is under a cfg_attr somewhere.", hits);
    }

    let hits = first(&code, names_a_feature);
    if !hits.is_empty() {
        return reject(
            "src: a cfg names a feature, but this crate declares none.",
            hits,
        );
    }

    Outcome::Pass
}

/// `feature[[:space:]]*=[[:space:]]*"`.
fn names_a_feature(line: &str) -> bool {
    line.match_indices("feature").any(|(at, _)| {
        let rest = line[at + "feature".len()..].trim_start_matches([' ', '\t']);
        rest.strip_prefix('=')
            .is_some_and(|tail| tail.trim_start_matches([' ', '\t']).starts_with('"'))
    })
}

pub fn integer_only(ctx: &Ctx) -> Outcome {
    // Full-line comments are stripped first: doc comments legitimately discuss
    // floating point, magnitude bounds such as 4.23e14, and the banned crate by
    // name, and none of that is a code path.
    //
    // The conformance suite and the Cargo examples are held to the float and
    // `ph-curves` bans too -- expected values must come from an integer
    // reference or hand computation. Tests run under the std harness, so only
    // runtime `src/` and the embedded example code carry the host-path bans.
    let all_code = match text::code_lines(&ctx.root, &["src", "tests", "examples"]) {
        Ok(lines) => lines,
        Err(error) => return Outcome::fail(format!("could not read sources: {error}")),
    };
    let runtime_code = match text::code_lines(&ctx.root, &["src"]) {
        Ok(lines) => lines,
        Err(error) => return Outcome::fail(format!("could not read src: {error}")),
    };
    let example_code = match text::code_lines(&ctx.root, &["examples"]) {
        Ok(lines) => lines,
        Err(error) => return Outcome::fail(format!("could not read examples: {error}")),
    };

    let hits = first(&all_code, |line| {
        text::contains_word(line, "f32") || text::contains_word(line, "f64")
    });
    if !hits.is_empty() {
        return reject(
            "src/tests/examples: code names a floating-point type.",
            hits,
        );
    }

    let hits = first(&all_code, |line| text::find_float_literal(line).is_some());
    if !hits.is_empty() {
        return reject(
            "src/tests/examples: code contains a floating-point literal.",
            hits,
        );
    }

    let hits = first(&runtime_code, host_path);
    if !hits.is_empty() {
        return reject("src: runtime code reaches for alloc or std.", hits);
    }

    let hits = first(&example_code, example_host_path);
    if !hits.is_empty() {
        return reject(
            "examples: code uses an allocator/std path, host output/debug macro, or unsafe.",
            hits,
        );
    }

    let hits = first(&all_code, |line| {
        text::contains_ph_curves(line, Separator::Any, false)
    });
    if !hits.is_empty() {
        return reject("src/tests/examples: code references ph-curves.", hits);
    }

    // A negative search for `unsafe` would match this very declaration, so
    // assert the crate-level ban is present instead.
    match text::read_text(&ctx.path("src/lib.rs")) {
        Ok(lib) if lib.lines().any(|line| line == "#![forbid(unsafe_code)]") => Outcome::Pass,
        Ok(_) => Outcome::fail("src/lib.rs must declare #![forbid(unsafe_code)]."),
        Err(error) => Outcome::fail(format!("src/lib.rs is unreadable: {error}")),
    }
}

fn host_path(line: &str) -> bool {
    text::contains_at_word_start(line, "alloc::")
        || text::contains_at_word_start(line, "std::")
        || text::contains_extern_crate(line, "alloc")
        || text::contains_extern_crate(line, "std")
}

fn example_host_path(line: &str) -> bool {
    const TYPES: [&str; 5] = ["Vec", "String", "Box", "Rc", "Arc"];
    const MACROS: [&str; 6] = ["vec", "print", "println", "eprint", "eprintln", "dbg"];

    host_path(line)
        || TYPES.iter().any(|name| text::contains_word(line, name))
        || MACROS
            .iter()
            .any(|name| text::contains_macro_call(line, name))
        || text::contains_word(line, "unsafe")
}

pub fn no_ph_curves(ctx: &Ctx) -> Outcome {
    // Three layers, each catching a form the others cannot:
    //   1. manifest text: an optional, target-specific, dev, build, path, Git,
    //      [patch], or [replace] entry is rejected before cargo resolves it,
    //      even when the path or Git source does not exist yet;
    //   2. Cargo.lock: any resolved package or source;
    //   3. cargo metadata with every feature enabled: every declared dependency
    //      of every kind, and every resolved package.
    // deny.toml bans the name as a fourth layer, for the transitive case.
    for manifest in ["Cargo.toml", ".cargo/config.toml", ".cargo/config"] {
        let path = ctx.path(manifest);
        if !path.is_file() {
            continue;
        }
        let text = match text::read_text(&path) {
            Ok(text) => text,
            Err(error) => return Outcome::fail(format!("{manifest} is unreadable: {error}")),
        };
        // A full-line TOML comment may explain the ban; a value may not name it.
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            if text::contains_ph_curves(line, Separator::DashOrUnderscore, true) {
                return Outcome::fail(format!(
                    "{manifest}:{}: {line}\n{manifest} names ph-curves outside a comment.",
                    index + 1
                ));
            }
        }
    }

    let lock = ctx.path("Cargo.lock");
    if !lock.is_file() {
        return Outcome::fail(
            "Cargo.lock is missing; the lockfile is part of the repository floor.",
        );
    }
    match text::read_text(&lock) {
        Ok(text) => {
            if text
                .lines()
                .any(|line| text::contains_ph_curves(line, Separator::DashOrUnderscore, true))
            {
                return Outcome::fail("Cargo.lock mentions a ph-curves package or source.");
            }
        }
        Err(error) => return Outcome::fail(format!("Cargo.lock is unreadable: {error}")),
    }

    let metadata = match proc::capture(
        &proc::cargo(),
        &[
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--all-features",
        ],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => {
            return Outcome::fail(format!("cargo metadata failed.\n{}", output.stderr));
        }
        Err(error) => return Outcome::fail(format!("cargo metadata could not run: {error}")),
    };

    if text::contains_ph_curves(&metadata, Separator::DashOrUnderscore, true) {
        return Outcome::fail("cargo metadata declares or resolves a ph-curves package.");
    }

    Outcome::Pass
}

pub fn manifest_floor(ctx: &Ctx) -> Outcome {
    let manifest = match text::read_text(&ctx.path("Cargo.toml")) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("Cargo.toml is unreadable: {error}")),
    };
    let has = |wanted: &str| manifest.lines().any(|line| line == wanted);

    if manifest.lines().any(|line| line.starts_with("[workspace]")) {
        return Outcome::fail(
            "Cargo.toml must not declare a workspace before a second package exists.",
        );
    }
    for (wanted, complaint) in [
        (
            "version = \"0.1.0-incubating.1\"",
            "Cargo.toml version must be 0.1.0-incubating.1.",
        ),
        (
            "publish = false",
            "Cargo.toml must keep publish = false until a separate release decision.",
        ),
        ("license = \"MIT\"", "Cargo.toml license must be MIT."),
        ("edition = \"2024\"", "Cargo.toml edition must be 2024."),
        (
            "rust-version = \"1.94.0\"",
            "Cargo.toml rust-version must be 1.94.0.",
        ),
    ] {
        if !has(wanted) {
            return Outcome::fail(complaint);
        }
    }

    match proc::capture(
        &proc::cargo(),
        &[
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--no-deps",
        ],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => {
            if !output.stdout.contains("\"dependencies\":[]") {
                return Outcome::fail("dependency tables must stay empty on this scaffold.");
            }
        }
        Ok(output) => {
            return Outcome::fail(format!(
                "cargo metadata failed while checking dependency tables.\n{}",
                output.stderr
            ));
        }
        Err(error) => return Outcome::fail(format!("cargo metadata could not run: {error}")),
    }

    let readme = match text::read_text(&ctx.path("README.md")) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("README.md is unreadable: {error}")),
    };
    for (needle, complaint) in [
        ("Incubating", "README.md must record Lifecycle Incubating."),
        (
            "0.1.0-incubating.1",
            "README.md must record version 0.1.0-incubating.1.",
        ),
        ("Libraries", "README.md must record Domain Libraries."),
    ] {
        if !readme.contains(needle) {
            return Outcome::fail(complaint);
        }
    }

    match text::read_text(&ctx.path("LICENSE")) {
        Ok(license) if license.contains("MIT License") => Outcome::Pass,
        Ok(_) => Outcome::fail("LICENSE must be the MIT License."),
        Err(error) => Outcome::fail(format!("LICENSE is unreadable: {error}")),
    }
}
