//! Publication policy for every resolved workspace member.
//!
//! `ph-surfaces` may publish only to crates.io. `ph-surfaces-bake` may publish
//! only to crates.io. The gate runner remains permanently unpublished.
//! Membership and Cargo's resolved `publish` values come from `cargo metadata
//! --no-deps`, so an implicitly admitted workspace member cannot escape
//! classification.

use std::path::Path;

use serde::Deserialize;

use crate::proc;
use crate::runner::{Ctx, Outcome};

const CRATES_IO_PACKAGES: &[&str] = &["ph-surfaces", "ph-surfaces-bake"];
const LOCKED_PACKAGES: &[&str] = &["xtask"];

/// The slice of `cargo metadata` output this check needs.
///
/// Unlike `xtask/config.ron`, this schema belongs to Cargo, so unknown fields
/// are ignored rather than rejected.
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    name: String,
    manifest_path: String,
    /// `None` when the manifest omits `publish` (publishable anywhere),
    /// `Some([])` for `publish = false`, otherwise the allowed registries.
    publish: Option<Vec<String>>,
}

/// Classify every resolved workspace member.
pub fn publish_lock(ctx: &Ctx) -> Outcome {
    // No `--locked`: a member added without refreshing the lockfile is exactly
    // the case this gate must report by name, not hide behind a lockfile error.
    let output = match proc::capture(
        &proc::cargo(),
        &["metadata", "--format-version", "1", "--no-deps"],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => {
            return Outcome::fail(format!(
                "cargo metadata --no-deps failed.\n{}",
                output.stderr
            ));
        }
        Err(error) => return Outcome::fail(format!("cargo could not run: {error}")),
    };

    let mut packages = match parse(&output) {
        Ok(packages) => packages,
        Err(error) => return Outcome::fail(error),
    };
    packages.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));

    let mut failures = Vec::new();
    for package in &packages {
        let manifest = relative_manifest(ctx, &package.manifest_path);
        match policy_outcome(package) {
            Outcome::Pass | Outcome::PassWithNote(_) => {
                println!("publication-metadata ({manifest}): ok");
            }
            Outcome::Fail(reason) => {
                println!("publication-metadata ({manifest}): FAIL");
                failures.push(reason);
            }
            Outcome::Skip(_) => unreachable!("publish_lock does not skip"),
        }
    }

    if failures.is_empty() {
        Outcome::Pass
    } else {
        Outcome::fail(failures.join("\n"))
    }
}

fn relative_manifest(ctx: &Ctx, manifest_path: &str) -> String {
    Path::new(manifest_path)
        .strip_prefix(&ctx.root)
        .map(|path| path.display().to_string().replace('\\', "/"))
        .unwrap_or_else(|_| manifest_path.replace('\\', "/"))
}

fn policy_outcome(package: &MetaPackage) -> Outcome {
    if CRATES_IO_PACKAGES.contains(&package.name.as_str()) {
        return if is_crates_io_only(package.publish.as_deref()) {
            Outcome::Pass
        } else {
            Outcome::fail(format!(
                "{} must set publish = [\"crates-io\"]",
                package.name
            ))
        };
    }

    if LOCKED_PACKAGES.contains(&package.name.as_str()) {
        return if is_locked(package.publish.as_deref()) {
            Outcome::Pass
        } else {
            Outcome::fail(format!("{} must retain publish = false", package.name))
        };
    }

    Outcome::fail(format!(
        "{} is an unclassified workspace member; assign an explicit publication policy",
        package.name
    ))
}

fn parse(json: &str) -> Result<Vec<MetaPackage>, String> {
    let metadata: Metadata = serde_json::from_str(json)
        .map_err(|error| format!("parsing `cargo metadata` output: {error}"))?;
    if metadata.packages.is_empty() {
        return Err("`cargo metadata --no-deps` reported no workspace members".to_string());
    }
    Ok(metadata.packages)
}

fn is_locked(publish: Option<&[String]>) -> bool {
    publish.is_some_and(<[String]>::is_empty)
}

fn is_crates_io_only(publish: Option<&[String]>) -> bool {
    publish.is_some_and(|registries| registries == ["crates-io"])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, publish: Option<Vec<String>>) -> MetaPackage {
        MetaPackage {
            name: name.to_owned(),
            manifest_path: format!("/w/{name}/Cargo.toml"),
            publish,
        }
    }

    #[test]
    fn product_crate_is_limited_to_crates_io() {
        assert!(matches!(
            policy_outcome(&package("ph-surfaces", Some(vec!["crates-io".to_owned()]))),
            Outcome::Pass
        ));
        assert!(matches!(
            policy_outcome(&package("ph-surfaces", None)),
            Outcome::Fail(_)
        ));
        assert!(matches!(
            policy_outcome(&package(
                "ph-surfaces-bake",
                Some(vec!["crates-io".to_owned()])
            )),
            Outcome::Pass
        ));
        assert!(matches!(
            policy_outcome(&package("ph-surfaces-bake", None)),
            Outcome::Fail(_)
        ));
    }

    #[test]
    fn tooling_stays_locked() {
        assert!(matches!(
            policy_outcome(&package("xtask", Some(vec![]))),
            Outcome::Pass
        ));
        assert!(matches!(
            policy_outcome(&package("xtask", None)),
            Outcome::Fail(_)
        ));
    }

    #[test]
    fn unknown_members_fail_closed() {
        assert!(matches!(
            policy_outcome(&package("new-member", Some(vec![]))),
            Outcome::Fail(_)
        ));
    }

    #[test]
    fn metadata_is_parsed_into_members() {
        let json = r#"{
            "packages": [
                {"name": "ph-surfaces", "manifest_path": "/w/a/Cargo.toml",
                 "publish": ["crates-io"], "unknown_future_field": 7},
                {"name": "xtask", "manifest_path": "/w/b/Cargo.toml",
                 "publish": []}
            ],
            "workspace_root": "/w"
        }"#;
        let packages = parse(json).unwrap();
        assert_eq!(packages.len(), 2);
        assert!(is_crates_io_only(packages[0].publish.as_deref()));
        assert!(is_locked(packages[1].publish.as_deref()));
    }

    #[test]
    fn empty_or_malformed_metadata_is_rejected() {
        assert!(parse(r#"{"packages": []}"#).is_err());
        assert!(parse("not json").is_err());
    }
}
