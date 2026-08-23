//! Syntax- and structure-aware source and manifest ratchets.

use cargo_metadata::MetadataCommand;
use toml::Value;

use crate::proc;
use crate::runner::{Ctx, Outcome};
use crate::text::{self, Scan};

fn findings(summary: &str, hits: Vec<String>) -> Outcome {
    if hits.is_empty() {
        return Outcome::Pass;
    }
    Outcome::fail(format!("{}\n{summary}", hits.join("\n")))
}

fn manifest(ctx: &Ctx, relative: &str) -> Result<Value, String> {
    let path = ctx.path(relative);
    let source =
        text::read_text(&path).map_err(|error| format!("{relative} is unreadable: {error}"))?;
    toml::from_str(&source).map_err(|error| format!("{relative} is invalid TOML: {error}"))
}

pub fn no_std_unconditional(ctx: &Ctx) -> Outcome {
    let lib = match text::read_text(&ctx.path("src/lib.rs")) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("src/lib.rs is unreadable: {error}")),
    };
    if !lib.lines().any(|line| line == "#![no_std]") {
        return Outcome::fail("src/lib.rs must declare an unconditional #![no_std].");
    }

    let root_manifest = match manifest(ctx, "Cargo.toml") {
        Ok(manifest) => manifest,
        Err(error) => return Outcome::fail(error),
    };
    if root_manifest.get("features").is_some() {
        return Outcome::fail("Cargo.toml declares a [features] table; none exists on this crate.");
    }

    match text::source_findings(
        &ctx.root,
        &ctx.config.source_policy.runtime_roots,
        &ctx.config.source_policy,
        Scan::FeatureCfg,
    ) {
        Ok(hits) => findings(
            "src: a cfg names a feature, but this crate declares none.",
            hits,
        ),
        Err(error) => Outcome::fail(error),
    }
}

pub fn integer_only(ctx: &Ctx) -> Outcome {
    let policy = &ctx.config.source_policy;
    for (dirs, scan, summary) in [
        (
            policy.oracle_roots.as_slice(),
            Scan::AllCode,
            "src/tests/examples: floating point or ph-curves appears in code.",
        ),
        (
            policy.runtime_roots.as_slice(),
            Scan::Runtime,
            "src: runtime code violates the core-only integer policy.",
        ),
        (
            policy.example_roots.as_slice(),
            Scan::Examples,
            "examples: code uses a host/allocator path, output macro, or unsafe.",
        ),
    ] {
        match text::source_findings(&ctx.root, dirs, policy, scan) {
            Ok(hits) if hits.is_empty() => {}
            Ok(hits) => return findings(summary, hits),
            Err(error) => return Outcome::fail(error),
        }
    }

    match text::read_text(&ctx.path("src/lib.rs")) {
        Ok(lib) if lib.lines().any(|line| line == "#![forbid(unsafe_code)]") => Outcome::Pass,
        Ok(_) => Outcome::fail("src/lib.rs must declare #![forbid(unsafe_code)]."),
        Err(error) => Outcome::fail(format!("src/lib.rs is unreadable: {error}")),
    }
}

pub fn no_ph_curves(ctx: &Ctx) -> Outcome {
    for relative in &ctx.config.source_policy.dependency_manifests {
        let path = ctx.path(relative);
        if !path.is_file() {
            continue;
        }
        let value = match manifest(ctx, relative) {
            Ok(value) => value,
            Err(error) => return Outcome::fail(error),
        };
        if value_names_ph_curves(&value) {
            return Outcome::fail(format!("{relative} names ph-curves outside a comment."));
        }
    }

    let lock = match manifest(ctx, "Cargo.lock") {
        Ok(lock) => lock,
        Err(error) => return Outcome::fail(error),
    };
    if value_names_ph_curves(&lock) {
        return Outcome::fail("Cargo.lock mentions a ph-curves package or source.");
    }

    let metadata = match metadata(ctx, false) {
        Ok(metadata) => metadata,
        Err(error) => return Outcome::fail(error),
    };
    if metadata
        .packages
        .iter()
        .any(|package| is_ph_curves(package.name.as_str()))
    {
        return Outcome::fail("cargo metadata declares or resolves a ph-curves package.");
    }
    Outcome::Pass
}

pub fn manifest_floor(ctx: &Ctx) -> Outcome {
    let root = match manifest(ctx, "Cargo.toml") {
        Ok(root) => root,
        Err(error) => return Outcome::fail(error),
    };
    if root.get("workspace").is_some() {
        return Outcome::fail(
            "Cargo.toml must not declare a workspace before a second package exists.",
        );
    }
    let Some(package) = root.get("package").and_then(Value::as_table) else {
        return Outcome::fail("Cargo.toml must contain [package].");
    };
    let expected = &ctx.config.package;
    for (field, wanted) in [
        ("name", expected.name.as_str()),
        ("version", expected.version.as_str()),
        ("license", expected.manifest.license.as_str()),
        ("edition", expected.manifest.edition.as_str()),
        ("rust-version", expected.manifest.rust_version.as_str()),
    ] {
        if package.get(field).and_then(Value::as_str) != Some(wanted) {
            return Outcome::fail(format!("Cargo.toml package.{field} must be `{wanted}`."));
        }
    }
    if package.get("publish").map(Value::to_string).as_deref()
        != Some(expected.manifest.publish.as_str())
    {
        return Outcome::fail(format!(
            "Cargo.toml package.publish must be `{}`.",
            expected.manifest.publish
        ));
    }

    let metadata = match metadata(ctx, true) {
        Ok(metadata) => metadata,
        Err(error) => return Outcome::fail(error),
    };
    let Some(root_package) = metadata.root_package() else {
        return Outcome::fail("cargo metadata did not identify the root package.");
    };
    let mut dependencies: Vec<String> = root_package
        .dependencies
        .iter()
        .map(|dependency| dependency.name.to_string())
        .collect();
    dependencies.sort();
    let mut expected_dependencies = expected.manifest.dependencies.clone();
    expected_dependencies.sort();
    if dependencies != expected_dependencies {
        return Outcome::fail(format!(
            "Cargo.toml dependencies differ: expected {expected_dependencies:?}, found {dependencies:?}."
        ));
    }

    match text::read_text(&ctx.path("LICENSE")) {
        Ok(license) if license.contains("MIT License") => Outcome::Pass,
        Ok(_) => Outcome::fail("LICENSE must be the MIT License."),
        Err(error) => Outcome::fail(format!("LICENSE is unreadable: {error}")),
    }
}

fn metadata(ctx: &Ctx, no_deps: bool) -> Result<cargo_metadata::Metadata, String> {
    let mut command = MetadataCommand::new();
    command
        .cargo_path(proc::cargo())
        .current_dir(&ctx.root)
        .manifest_path(ctx.path("Cargo.toml"));
    if no_deps {
        command.no_deps();
    } else {
        command.other_options(vec!["--offline".into(), "--all-features".into()]);
    }
    command
        .exec()
        .map_err(|error| format!("cargo metadata failed: {error}"))
}

fn is_ph_curves(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "ph-curves" | "ph_curves"
    )
}

fn value_names_ph_curves(value: &Value) -> bool {
    match value {
        Value::String(value) => value
            .split(|character: char| {
                !character.is_ascii_alphanumeric() && character != '-' && character != '_'
            })
            .any(is_ph_curves),
        Value::Array(values) => values.iter().any(value_names_ph_curves),
        Value::Table(values) => values
            .iter()
            .any(|(key, value)| is_ph_curves(key) || value_names_ph_curves(value)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_value_scan_ignores_near_names() {
        let banned: Value = toml::from_str("[dependencies]\nph_curves = \"1\"\n").unwrap();
        let allowed: Value = toml::from_str("[dependencies]\nph-curves-extra = \"1\"\n").unwrap();
        assert!(value_names_ph_curves(&banned));
        assert!(!value_names_ph_curves(&allowed));
    }
}
