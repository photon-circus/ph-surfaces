//! Checked-in generated-source regeneration and drift detection.
//!
//! `cargo xtask generate` writes the baker-owned artifact. CI re-renders the
//! same source in memory and compares it to the file on disk through
//! `text::read_text` so a CRLF checkout cannot fail the comparison. A SKIP is
//! not a pass: this check either matches or fails.

use std::fs;

use similar::TextDiff;

use crate::runner::{Ctx, Outcome};
use crate::text;

/// Write the checked-in generated module from the baker's fixture renderer.
pub fn write(ctx: &Ctx) -> Result<(), String> {
    let relative = &ctx.config.baker.generated;
    let path = ctx.path(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    fs::write(&path, ph_surfaces_bake::checked_in_source())
        .map_err(|error| format!("could not write {relative}: {error}"))?;
    println!("wrote {relative}");
    Ok(())
}

/// Re-render the checked-in module in memory and compare it to the file.
pub fn generated_source(ctx: &Ctx) -> Outcome {
    let relative = &ctx.config.baker.generated;
    let expected = ph_surfaces_bake::checked_in_source();
    let path = ctx.path(relative);
    let actual = match text::read_text(&path) {
        Ok(text) => text,
        Err(error) => {
            return Outcome::fail(format!(
                "cannot read {relative}: {error}; run cargo xtask generate"
            ));
        }
    };
    if actual == expected {
        println!("generated source: {relative}");
        return Outcome::Pass;
    }
    let diff = TextDiff::from_lines(&expected, &actual)
        .unified_diff()
        .header("expected", "committed")
        .to_string();
    Outcome::fail(format!(
        "{diff}{relative} differs from the baker output; run cargo xtask generate"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::runner::{Ctx, Profile};
    use std::sync::Arc;

    fn ctx(root: &std::path::Path) -> Ctx {
        Ctx {
            root: root.to_path_buf(),
            profile: Profile::Full,
            nightly: "nightly".to_string(),
            skip_embedded: false,
            coverage: false,
            config: Arc::new(Config::load(root).expect("committed configuration must load")),
        }
    }

    #[test]
    fn committed_generated_source_matches_the_renderer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits one level below the repository root");
        assert!(
            matches!(generated_source(&ctx(root)), Outcome::Pass),
            "committed generated source drifted; run cargo xtask generate"
        );
    }
}
