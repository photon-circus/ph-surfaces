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

const CRATE_LIB: &str = "crates/surfaces/src/lib.rs";
const CRATE_MANIFEST: &str = "crates/surfaces/Cargo.toml";

pub fn no_std_unconditional(ctx: &Ctx) -> Outcome {
    let lib = match text::read_text(&ctx.path(CRATE_LIB)) {
        Ok(text) => text,
        Err(error) => return Outcome::fail(format!("{CRATE_LIB} is unreadable: {error}")),
    };
    if !lib.lines().any(|line| line == "#![no_std]") {
        return Outcome::fail(format!(
            "{CRATE_LIB} must declare an unconditional #![no_std]."
        ));
    }

    let crate_manifest = match manifest(ctx, CRATE_MANIFEST) {
        Ok(manifest) => manifest,
        Err(error) => return Outcome::fail(error),
    };
    if crate_manifest.get("features").is_some() {
        return Outcome::fail(format!(
            "{CRATE_MANIFEST} declares a [features] table; none exists on this crate."
        ));
    }

    match text::source_findings(
        &ctx.root,
        &ctx.config.source_policy.runtime_roots,
        &ctx.config.source_policy,
        Scan::FeatureCfg,
    ) {
        Ok(hits) => findings(
            "crates/surfaces/src: a cfg names a feature, but this crate declares none.",
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

    match text::read_text(&ctx.path(CRATE_LIB)) {
        Ok(lib) if lib.lines().any(|line| line == "#![forbid(unsafe_code)]") => Outcome::Pass,
        Ok(_) => Outcome::fail(format!("{CRATE_LIB} must declare #![forbid(unsafe_code)].")),
        Err(error) => Outcome::fail(format!("{CRATE_LIB} is unreadable: {error}")),
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
        // The baker package itself is named `ph-surfaces-bake`; only the
        // runtime crate's manifest is forbidden from naming it.
        if relative == CRATE_MANIFEST && value_names_ph_surfaces_bake(&value) {
            return Outcome::fail(format!(
                "{relative} names ph-surfaces-bake; the runtime crate must not reach the baker."
            ));
        }
    }

    let lock = match manifest(ctx, "Cargo.lock") {
        Ok(lock) => lock,
        Err(error) => return Outcome::fail(error),
    };
    if value_names_ph_curves(&lock) {
        return Outcome::fail("Cargo.lock mentions a ph-curves package or source.");
    }
    if runtime_lock_names_bake(&lock) {
        return Outcome::fail(
            "Cargo.lock records a ph-surfaces-bake dependency on the runtime package.",
        );
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
    if metadata_runtime_depends_on_bake(&metadata) {
        return Outcome::fail(
            "cargo metadata records a ph-surfaces-bake dependency on the runtime package.",
        );
    }
    Outcome::Pass
}

pub fn manifest_floor(ctx: &Ctx) -> Outcome {
    let root = match manifest(ctx, "Cargo.toml") {
        Ok(root) => root,
        Err(error) => return Outcome::fail(error),
    };
    let Some(workspace) = root.get("workspace").and_then(Value::as_table) else {
        return Outcome::fail("Cargo.toml must declare the workspace.");
    };
    if workspace.get("members").is_none() {
        return Outcome::fail("Cargo.toml workspace.members must be present.");
    }
    if workspace_lists_xtask_as_default_member(workspace) {
        return Outcome::fail(
            "Cargo.toml default-members must omit the gate so a bare cargo build touches shipped packages only.",
        );
    }
    if root.get("package").is_some() {
        return Outcome::fail(
            "root Cargo.toml must be a virtual workspace: it must not contain [package].",
        );
    }

    let crate_manifest = match manifest(ctx, CRATE_MANIFEST) {
        Ok(manifest) => manifest,
        Err(error) => return Outcome::fail(error),
    };
    let Some(package) = crate_manifest.get("package").and_then(Value::as_table) else {
        return Outcome::fail(format!("{CRATE_MANIFEST} must contain [package]."));
    };
    let expected = &ctx.config.package;
    for (field, wanted) in [
        ("name", expected.name.as_str()),
        ("version", expected.version.as_str()),
    ] {
        if package.get(field).and_then(Value::as_str) != Some(wanted) {
            return Outcome::fail(format!(
                "{CRATE_MANIFEST} package.{field} must be `{wanted}`."
            ));
        }
    }
    if package.get("publish").map(Value::to_string).as_deref()
        != Some(expected.manifest.publish.as_str())
    {
        return Outcome::fail(format!(
            "{CRATE_MANIFEST} package.publish must be `{}`.",
            expected.manifest.publish
        ));
    }

    let metadata = match metadata(ctx, true) {
        Ok(metadata) => metadata,
        Err(error) => return Outcome::fail(error),
    };
    let Some(crate_package) = metadata
        .packages
        .iter()
        .find(|package| package.name.as_str() == expected.name)
    else {
        return Outcome::fail(format!(
            "cargo metadata did not identify the `{}` package.",
            expected.name
        ));
    };
    if crate_package.license.as_deref() != Some(expected.manifest.license.as_str()) {
        return Outcome::fail(format!(
            "{CRATE_MANIFEST} license must resolve to `{}`.",
            expected.manifest.license
        ));
    }
    if crate_package.edition.as_str() != expected.manifest.edition {
        return Outcome::fail(format!(
            "{CRATE_MANIFEST} edition must resolve to `{}`.",
            expected.manifest.edition
        ));
    }
    match crate_package.rust_version.as_ref() {
        Some(version) if version.to_string() == expected.manifest.rust_version => {}
        _ => {
            return Outcome::fail(format!(
                "{CRATE_MANIFEST} rust-version must resolve to `{}`.",
                expected.manifest.rust_version
            ));
        }
    }
    let mut dependencies: Vec<String> = crate_package
        .dependencies
        .iter()
        .map(|dependency| dependency.name.to_string())
        .collect();
    dependencies.sort();
    let mut expected_dependencies = expected.manifest.dependencies.clone();
    expected_dependencies.sort();
    if dependencies != expected_dependencies {
        return Outcome::fail(format!(
            "{CRATE_MANIFEST} dependencies differ: expected {expected_dependencies:?}, found {dependencies:?}."
        ));
    }

    match text::read_text(&ctx.path("LICENSE")) {
        Ok(license) if license.contains("MIT License") => Outcome::Pass,
        Ok(_) => Outcome::fail("LICENSE must be the MIT License."),
        Err(error) => Outcome::fail(format!("LICENSE is unreadable: {error}")),
    }
}

fn workspace_lists_xtask_as_default_member(workspace: &toml::map::Map<String, Value>) -> bool {
    workspace
        .get("default-members")
        .and_then(Value::as_array)
        .is_some_and(|members| {
            members.iter().any(|member| {
                member
                    .as_str()
                    .is_some_and(|path| path == "xtask" || path.ends_with("/xtask"))
            })
        })
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
        // `--locked` (not `--offline`): the workspace lockfile includes xtask
        // host crates, some of them target-specific, and those must be
        // fetchable. The lockfile still pins every version.
        command.other_options(vec!["--locked".into(), "--all-features".into()]);
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

fn is_ph_surfaces_bake(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "ph-surfaces-bake" | "ph_surfaces_bake"
    )
}

fn value_names_token(value: &Value, is_name: fn(&str) -> bool) -> bool {
    match value {
        Value::String(value) => value
            .split(|character: char| {
                !character.is_ascii_alphanumeric() && character != '-' && character != '_'
            })
            .any(is_name),
        Value::Array(values) => values.iter().any(|value| value_names_token(value, is_name)),
        Value::Table(values) => values
            .iter()
            .any(|(key, value)| is_name(key) || value_names_token(value, is_name)),
        _ => false,
    }
}

fn value_names_ph_curves(value: &Value) -> bool {
    value_names_token(value, is_ph_curves)
}

fn value_names_ph_surfaces_bake(value: &Value) -> bool {
    value_names_token(value, is_ph_surfaces_bake)
}

fn runtime_lock_names_bake(lock: &Value) -> bool {
    lock.get("package")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|package| {
            package.get("name").and_then(Value::as_str) == Some("ph-surfaces")
                && package
                    .get("dependencies")
                    .is_some_and(value_names_ph_surfaces_bake)
        })
}

fn metadata_runtime_depends_on_bake(metadata: &cargo_metadata::Metadata) -> bool {
    let runtime = metadata
        .packages
        .iter()
        .find(|package| package.name.as_str() == "ph-surfaces");
    let Some(runtime) = runtime else {
        return false;
    };
    if runtime
        .dependencies
        .iter()
        .any(|dependency| is_ph_surfaces_bake(dependency.name.as_str()))
    {
        return true;
    }
    let Some(resolve) = metadata.resolve.as_ref() else {
        return false;
    };
    resolve
        .nodes
        .iter()
        .find(|node| node.id == runtime.id)
        .is_some_and(|node| {
            node.deps
                .iter()
                .any(|dep| is_ph_surfaces_bake(dep.name.as_str()))
        })
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

    #[test]
    fn toml_value_scan_names_the_baker_only_as_an_exact_token() {
        let banned: Value =
            toml::from_str("[dependencies]\nph-surfaces-bake = { path = \"../surfaces-bake\" }\n")
                .unwrap();
        let underscored: Value =
            toml::from_str("[dev-dependencies]\nph_surfaces_bake = \"1\"\n").unwrap();
        let near: Value =
            toml::from_str("[dependencies]\nph-surfaces-bake-extra = \"1\"\n").unwrap();
        assert!(value_names_ph_surfaces_bake(&banned));
        assert!(value_names_ph_surfaces_bake(&underscored));
        assert!(!value_names_ph_surfaces_bake(&near));
    }

    #[test]
    fn runtime_lock_entry_names_bake_only_on_the_runtime_package() {
        let dependent: Value = toml::from_str(
            r#"
[[package]]
name = "ph-surfaces"
version = "0.1.0"
dependencies = ["ph-surfaces-bake"]

[[package]]
name = "ph-surfaces-bake"
version = "0.1.0"
"#,
        )
        .unwrap();
        assert!(runtime_lock_names_bake(&dependent));

        let member_only: Value = toml::from_str(
            r#"
[[package]]
name = "ph-surfaces"
version = "0.1.0"

[[package]]
name = "ph-surfaces-bake"
version = "0.1.0"
"#,
        )
        .unwrap();
        assert!(!runtime_lock_names_bake(&member_only));
    }
}
