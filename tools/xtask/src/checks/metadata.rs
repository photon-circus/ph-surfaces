//! Required GitHub repository metadata.
//!
//! Tracked by issue #28, which supplies the real settings. Until then this
//! reports SKIP for an ordinary run and, because the runner escalates every
//! would-be SKIP in the release profile, FAIL for release evidence -- which is
//! exactly what issue #27 asked for. The strict branches the shell gate
//! repeated at each site are therefore not duplicated here.

use crate::proc;
use crate::runner::{Ctx, Outcome};
use crate::text;

const TOPICS: [&str; 5] = ["rust", "embedded", "no-std", "no-alloc", "interpolation"];
const LIFECYCLE: &str = "Incubating";
const DOMAIN: &str = "Libraries";

/// `owner/name`, read from the manifest rather than repeated as a constant.
fn slug(ctx: &Ctx) -> Result<String, String> {
    let manifest = text::read_text(&ctx.path("Cargo.toml"))
        .map_err(|error| format!("Cargo.toml is unreadable: {error}"))?;
    let line = manifest
        .lines()
        .find(|line| line.starts_with("repository = "))
        .ok_or_else(|| "Cargo.toml declares no repository".to_string())?;
    let url = line
        .split('"')
        .nth(1)
        .ok_or_else(|| format!("could not parse: {line}"))?;
    url.strip_prefix("https://github.com/")
        .map(|slug| slug.trim_end_matches(".git").to_string())
        .ok_or_else(|| format!("repository is not a github.com URL: {url}"))
}

pub fn github_metadata(ctx: &Ctx) -> Outcome {
    let slug = match slug(ctx) {
        Ok(slug) => slug,
        Err(message) => return Outcome::Fail(message),
    };

    let topics = match proc::capture(
        "gh",
        &[
            "api",
            &format!("repos/{slug}"),
            "--jq",
            ".topics | join(\",\")",
        ],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => output.stdout.trim().to_string(),
        Ok(_) => {
            return Outcome::skip("GitHub topics are not readable with this token");
        }
        Err(_) => return Outcome::skip("gh not installed; skipping (see https://cli.github.com)"),
    };

    println!("topics: {topics}");
    if topics.is_empty() {
        return Outcome::skip(format!(
            "GitHub topics are unset; required: {}",
            TOPICS.join(", ")
        ));
    }

    let present: Vec<&str> = topics.split(',').map(str::trim).collect();
    let missing: Vec<&str> = TOPICS
        .iter()
        .filter(|topic| !present.contains(topic))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Outcome::fail(format!("missing GitHub topics: {}", missing.join(" ")));
    }

    let properties = match proc::capture(
        "gh",
        &[
            "api",
            &format!("repos/{slug}/properties/values"),
            "--jq",
            ".[] | [.property_name, (.value // \"\")] | @tsv",
        ],
        &ctx.root,
    ) {
        Ok(output) if output.ok() => output.stdout,
        _ => return Outcome::skip("GitHub custom properties are not readable with this token"),
    };
    print!("{properties}");

    let value = |wanted: &str| {
        properties
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(name, _)| *name == wanted)
            .map(|(_, value)| value.trim().to_string())
            .unwrap_or_default()
    };

    let mut unset: Vec<&str> = Vec::new();
    for (property, required) in [("Lifecycle", LIFECYCLE), ("Domain", DOMAIN)] {
        let found = value(property);
        if found.is_empty() {
            println!("{property} custom property is unset.");
            unset.push(property);
        } else if found != required {
            return Outcome::fail(format!(
                "{property} custom property must be {required}, found: {found}"
            ));
        }
    }

    if !unset.is_empty() {
        return Outcome::skip(format!(
            "GitHub {} custom propert{} unset",
            unset.join(" and "),
            if unset.len() == 1 { "y is" } else { "ies are" }
        ));
    }

    Outcome::Pass
}

/// The workflow is the one piece of the gate that still is not Rust, so lint it.
///
/// `RELEASING.md` had this as a manual step, which means it ran only when
/// someone remembered.
pub fn actionlint(ctx: &Ctx) -> Outcome {
    match proc::run("actionlint", &["-color"], &ctx.root, &[]) {
        Ok(Some(0)) => Outcome::Pass,
        Ok(Some(code)) => Outcome::fail(format!("actionlint exited {code}")),
        Ok(None) => Outcome::fail("actionlint was terminated"),
        Err(_) => Outcome::skip(
            "actionlint not installed; skipping \
             (go install github.com/rhysd/actionlint/cmd/actionlint@latest)",
        ),
    }
}
