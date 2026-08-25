//! Implementation-line budget for the host baker.
//!
//! `crates/surfaces-bake/src` is capped at a declared number of
//! implementation lines. Tests (`#[cfg(test)]` tails, matching the
//! integer-only scanner), fixtures, and generated output directories are
//! excluded. Exceeding the cap is a FAIL, not a quiet raise.

use std::path::Path;

use crate::runner::{Ctx, Outcome};
use crate::text;

const EXCLUDED_DIR_NAMES: &[&str] = &["fixtures", "generated", "golden", "goldens", "out"];

/// Count implementation lines under the configured baker `src` and compare
/// them to the cap.
pub fn baker_line_budget(ctx: &Ctx) -> Outcome {
    let baker = &ctx.config.baker;
    let src = ctx.path(&baker.src);
    if !src.is_dir() {
        return Outcome::fail(format!(
            "{} is missing; the baker crate floor must exist.",
            baker.src
        ));
    }

    let sources = match text::rust_sources(&src) {
        Ok(sources) => sources,
        Err(error) => return Outcome::fail(format!("{} is unreadable: {error}", baker.src)),
    };

    let mut total = 0usize;
    let mut counted_files = 0usize;
    for path in sources {
        if path_is_excluded(&src, &path) {
            continue;
        }
        let source = match text::read_text(&path) {
            Ok(source) => source,
            Err(error) => {
                return Outcome::fail(format!("{} is unreadable: {error}", path.display()));
            }
        };
        let count = match text::implementation_line_count(&source) {
            Ok(count) => count,
            Err(error) => {
                return Outcome::fail(format!("{} is not valid Rust: {error}", path.display()));
            }
        };
        counted_files += 1;
        total = total.saturating_add(count);
    }

    if counted_files == 0 {
        return Outcome::fail(format!(
            "{} contains no implementation sources to budget.",
            baker.src
        ));
    }

    println!(
        "baker line budget: {total} / {} implementation lines in {}",
        baker.max_implementation_lines, baker.src
    );

    if total > baker.max_implementation_lines {
        return Outcome::fail(format!(
            "{}: {total} implementation lines; the cap is {}.\n\
             Exceeding the baker line budget is a FAIL, not a quiet raise.",
            baker.src, baker.max_implementation_lines
        ));
    }
    Outcome::Pass
}

fn path_is_excluded(src: &Path, path: &Path) -> bool {
    path.strip_prefix(src)
        .ok()
        .and_then(|relative| relative.parent())
        .is_some_and(|parent| {
            parent.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|name| EXCLUDED_DIR_NAMES.contains(&name))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fixture_and_generated_directories_are_excluded() {
        let src = PathBuf::from("crates/surfaces-bake/src");
        assert!(path_is_excluded(&src, &src.join("fixtures/sample.rs")));
        assert!(path_is_excluded(&src, &src.join("generated/out.rs")));
        assert!(!path_is_excluded(&src, &src.join("lib.rs")));
        assert!(!path_is_excluded(&src, &src.join("ingest.rs")));
    }
}
