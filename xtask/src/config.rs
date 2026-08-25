//! Declarative policy for the verification gate.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path};

use serde::Deserialize;

use crate::runner::Profile;

pub const RELATIVE_PATH: &str = "xtask/config.ron";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub package: Package,
    pub examples: Vec<String>,
    pub source_policy: SourcePolicy,
    pub targets: Vec<Target>,
    pub code_size: CodeSize,
    pub checks: Vec<CheckSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub manifest: ManifestFloor,
    pub files: Vec<String>,
    pub non_consumer_prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFloor {
    pub publish: String,
    pub license: String,
    pub edition: String,
    pub rust_version: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicy {
    pub runtime_roots: Vec<String>,
    pub oracle_roots: Vec<String>,
    pub example_roots: Vec<String>,
    pub arithmetic_kernel: String,
    pub forbidden_example_types: Vec<String>,
    pub forbidden_example_macros: Vec<String>,
    pub dependency_manifests: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub triple: String,
    pub asm_snapshot: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSize {
    pub snapshot: String,
    pub kernel_symbol: String,
    pub kernel_path_fragment: String,
    pub kernel_mangled_fragment: String,
    pub pairings: Vec<Pairing>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pairing {
    pub symbol: String,
    pub feature: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSpec {
    pub name: String,
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub opt_in: Option<OptIn>,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum OptIn {
    Coverage,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum Action {
    LineEndings,
    NoStdUnconditional,
    IntegerOnly,
    NoPhCurves,
    ManifestFloor,
    Fmt,
    Test,
    ReleaseTest,
    Examples,
    Clippy,
    Doc,
    Coverage,
    PackageList,
    PackageBuild,
    PackageProvenance,
    PackageDigest,
    PackageConsumer,
    CodeSizeSnapshot,
    GuardSelftest,
    Deny,
    SecretScan,
    PublishLock,
    CoreOnly { target: String },
    EmbeddedTarget { target: String },
}

impl Action {
    fn singleton_name(&self) -> Option<&'static str> {
        Some(match self {
            Self::LineEndings => "LineEndings",
            Self::NoStdUnconditional => "NoStdUnconditional",
            Self::IntegerOnly => "IntegerOnly",
            Self::NoPhCurves => "NoPhCurves",
            Self::ManifestFloor => "ManifestFloor",
            Self::Fmt => "Fmt",
            Self::Test => "Test",
            Self::ReleaseTest => "ReleaseTest",
            Self::Examples => "Examples",
            Self::Clippy => "Clippy",
            Self::Doc => "Doc",
            Self::Coverage => "Coverage",
            Self::PackageList => "PackageList",
            Self::PackageBuild => "PackageBuild",
            Self::PackageProvenance => "PackageProvenance",
            Self::PackageDigest => "PackageDigest",
            Self::PackageConsumer => "PackageConsumer",
            Self::CodeSizeSnapshot => "CodeSizeSnapshot",
            Self::GuardSelftest => "GuardSelftest",
            Self::Deny => "Deny",
            Self::SecretScan => "SecretScan",
            Self::PublishLock => "PublishLock",
            Self::CoreOnly { .. } | Self::EmbeddedTarget { .. } => return None,
        })
    }

    pub fn target_id(&self) -> Option<&str> {
        match self {
            Self::CoreOnly { target } | Self::EmbeddedTarget { target } => Some(target),
            _ => None,
        }
    }
}

impl Config {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(RELATIVE_PATH);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("{} is unreadable: {error}", path.display()))?;
        let config: Self = ron::from_str(&text)
            .map_err(|error| format!("{} is invalid: {error}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|target| target.id == id)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported xtask configuration schema {}; expected 1",
                self.schema_version
            ));
        }
        nonempty("package name", &self.package.name)?;
        nonempty("package version", &self.package.version)?;
        unique("examples", &self.examples)?;
        unique("package files", &self.package.files)?;
        unique("non-consumer prefixes", &self.package.non_consumer_prefixes)?;
        unique("runtime roots", &self.source_policy.runtime_roots)?;
        unique("oracle roots", &self.source_policy.oracle_roots)?;
        unique("example roots", &self.source_policy.example_roots)?;
        unique(
            "forbidden example types",
            &self.source_policy.forbidden_example_types,
        )?;
        unique(
            "forbidden example macros",
            &self.source_policy.forbidden_example_macros,
        )?;
        unique(
            "dependency manifests",
            &self.source_policy.dependency_manifests,
        )?;
        nonempty("code-size snapshot", &self.code_size.snapshot)?;
        nonempty("kernel symbol", &self.code_size.kernel_symbol)?;
        nonempty("kernel path fragment", &self.code_size.kernel_path_fragment)?;
        nonempty(
            "kernel mangled fragment",
            &self.code_size.kernel_mangled_fragment,
        )?;

        for path in self
            .package
            .files
            .iter()
            .chain(self.package.non_consumer_prefixes.iter())
            .chain(self.source_policy.runtime_roots.iter())
            .chain(self.source_policy.oracle_roots.iter())
            .chain(self.source_policy.example_roots.iter())
            .chain([&self.source_policy.arithmetic_kernel])
            .chain(self.source_policy.dependency_manifests.iter())
            .chain([&self.code_size.snapshot])
        {
            relative_path(path)?;
        }

        if self.targets.is_empty() || self.code_size.pairings.is_empty() || self.checks.is_empty() {
            return Err("targets, code-size pairings, and checks must not be empty".to_string());
        }
        let target_ids: Vec<String> = self.targets.iter().map(|item| item.id.clone()).collect();
        let target_triples: Vec<String> = self
            .targets
            .iter()
            .map(|item| item.triple.clone())
            .collect();
        unique("target IDs", &target_ids)?;
        unique("target triples", &target_triples)?;
        for target in &self.targets {
            nonempty("target ID", &target.id)?;
            nonempty("target triple", &target.triple)?;
            relative_path(&target.asm_snapshot)?;
        }

        let symbols: Vec<String> = self
            .code_size
            .pairings
            .iter()
            .map(|item| item.symbol.clone())
            .collect();
        let features: Vec<String> = self
            .code_size
            .pairings
            .iter()
            .map(|item| item.feature.clone())
            .collect();
        unique("pairing symbols", &symbols)?;
        unique("pairing features", &features)?;
        for pairing in &self.code_size.pairings {
            nonempty("pairing description", &pairing.description)?;
        }

        let check_names: Vec<String> = self.checks.iter().map(|item| item.name.clone()).collect();
        unique("check names", &check_names)?;
        let mut singleton_counts: HashMap<&str, usize> = HashMap::new();
        let mut core_targets = HashSet::new();
        let mut embedded_targets = HashSet::new();
        for check in &self.checks {
            nonempty("check name", &check.name)?;
            if check.profiles.is_empty() {
                return Err(format!("check `{}` has no profiles", check.name));
            }
            let mut profiles = HashSet::new();
            if !check
                .profiles
                .iter()
                .all(|profile| profiles.insert(*profile))
            {
                return Err(format!("check `{}` repeats a profile", check.name));
            }
            if let Some(name) = check.action.singleton_name() {
                *singleton_counts.entry(name).or_default() += 1;
            }
            match (&check.action, check.opt_in) {
                (Action::Coverage, Some(OptIn::Coverage)) => {}
                (Action::Coverage, _) => {
                    return Err("the coverage action must be enabled by the coverage opt-in".into());
                }
                (_, Some(OptIn::Coverage)) => {
                    return Err(format!(
                        "check `{}` uses the coverage opt-in with a non-coverage action",
                        check.name
                    ));
                }
                (_, None) => {}
            }
            if let Some(target) = check.action.target_id() {
                if self.target(target).is_none() {
                    return Err(format!(
                        "check `{}` references unknown target `{target}`",
                        check.name
                    ));
                }
                match &check.action {
                    Action::CoreOnly { .. } => {
                        if !core_targets.insert(target) {
                            return Err(format!("target `{target}` has two core-only checks"));
                        }
                    }
                    Action::EmbeddedTarget { .. } => {
                        if !embedded_targets.insert(target) {
                            return Err(format!("target `{target}` has two embedded checks"));
                        }
                    }
                    _ => {}
                }
            }
        }
        for required in [
            "LineEndings",
            "NoStdUnconditional",
            "IntegerOnly",
            "NoPhCurves",
            "ManifestFloor",
            "Fmt",
            "Test",
            "ReleaseTest",
            "Examples",
            "Clippy",
            "Doc",
            "Coverage",
            "PackageList",
            "PackageBuild",
            "PackageProvenance",
            "PackageDigest",
            "PackageConsumer",
            "CodeSizeSnapshot",
            "GuardSelftest",
            "Deny",
            "SecretScan",
            "PublishLock",
        ] {
            if singleton_counts.get(required) != Some(&1) {
                return Err(format!(
                    "configuration requires exactly one `{required}` action"
                ));
            }
        }
        for target in &self.targets {
            if !core_targets.contains(target.id.as_str())
                || !embedded_targets.contains(target.id.as_str())
            {
                return Err(format!(
                    "target `{}` requires one core-only and one embedded check",
                    target.id
                ));
            }
        }
        Ok(())
    }
}

fn nonempty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

fn unique(label: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    let mut seen = HashSet::new();
    for value in values {
        nonempty(label, value)?;
        if !seen.insert(value) {
            return Err(format!("{label} contains duplicate `{value}`"));
        }
    }
    Ok(())
}

fn relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!(
            "configuration path `{value}` must be a normalized relative path"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_configuration_is_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits one level below the repository root");
        Config::load(root).unwrap();
    }

    #[test]
    fn invalid_schema_is_rejected() {
        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.schema_version = 2;
        assert!(config.validate().unwrap_err().contains("schema"));
    }

    #[test]
    fn duplicates_and_unknown_targets_are_rejected() {
        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.examples.push(config.examples[0].clone());
        assert!(config.validate().unwrap_err().contains("duplicate"));

        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.checks.last_mut().unwrap().action = Action::EmbeddedTarget {
            target: "missing".to_string(),
        };
        assert!(config.validate().unwrap_err().contains("unknown target"));
    }

    #[test]
    fn invalid_paths_missing_handlers_and_empty_profiles_are_rejected() {
        assert!(ron::from_str::<Config>("(").is_err());

        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.code_size.snapshot = "../outside".to_string();
        assert!(config.validate().unwrap_err().contains("relative path"));

        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.checks.remove(0);
        assert!(config.validate().unwrap_err().contains("LineEndings"));

        let mut config: Config = ron::from_str(include_str!("../config.ron")).unwrap();
        config.checks[0].profiles.clear();
        assert!(config.validate().unwrap_err().contains("no profiles"));
    }
}
