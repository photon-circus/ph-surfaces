//! Check implementations and the closed dispatch from declarative actions.

pub mod cargo;
pub mod code_size;
pub mod deny;
pub mod embedded;
pub mod history;
pub mod line_endings;
pub mod package;
pub mod publish_lock;
pub mod ratchets;

use crate::config::Action;
use crate::runner::{Ctx, Outcome};

pub fn run_action(ctx: &Ctx, action: &Action) -> Outcome {
    match action {
        Action::LineEndings => line_endings::line_endings(ctx),
        Action::NoStdUnconditional => ratchets::no_std_unconditional(ctx),
        Action::IntegerOnly => ratchets::integer_only(ctx),
        Action::NoPhCurves => ratchets::no_ph_curves(ctx),
        Action::ManifestFloor => ratchets::manifest_floor(ctx),
        Action::Fmt => cargo::fmt(ctx),
        Action::Test => cargo::test(ctx),
        Action::ReleaseTest => cargo::release_test(ctx),
        Action::Examples => cargo::examples(ctx),
        Action::Clippy => cargo::clippy(ctx),
        Action::Doc => cargo::doc(ctx),
        Action::Coverage => cargo::coverage(ctx),
        Action::PackageList => package::package_list(ctx),
        Action::PackageBuild => package::package_build(ctx),
        Action::PackageProvenance => package::package_provenance(ctx),
        Action::PackageDigest => package::package_digest(ctx),
        Action::PackageConsumer => package::package_consumer(ctx),
        Action::CodeSizeSnapshot => code_size::code_size_snapshot(ctx),
        Action::GuardSelftest => guard_selftest(ctx),
        Action::Deny => deny::deny(ctx),
        Action::SecretScan => history::secret_scan(ctx),
        Action::PublishLock => publish_lock::publish_lock(ctx),
        Action::CoreOnly { target } => {
            let triple = &ctx.config.target(target).expect("validated target").triple;
            embedded::core_only(ctx, triple)
        }
        Action::EmbeddedTarget { target } => {
            let triple = &ctx.config.target(target).expect("validated target").triple;
            embedded::embedded_target(ctx, triple)
        }
    }
}

fn guard_selftest(ctx: &Ctx) -> Outcome {
    let target_dir = ctx.path("target/xt/selftest");
    let target_dir = target_dir.display().to_string();
    cargo::step(
        ctx,
        &crate::proc::cargo(),
        &["test", "--manifest-path", "xtask/Cargo.toml"],
        &[("CARGO_TARGET_DIR", target_dir.as_str())],
    )
}
