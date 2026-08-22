//! The check registry protocol and the aggregate report.
//!
//! `RELEASING.md` asserts `! grep -Eq '^  (FAIL|SKIP)  ' target/release-ci.log`
//! against a teed run, so the two-leading-space summary lines below are a
//! machine-read contract, not cosmetics. Reformatting them silently disarms the
//! release gate.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    /// The fast inner loop: source ratchets and the host contract.
    Dev,
    /// Everything that can run here. A missing optional tool is a SKIP.
    Full,
    /// Release evidence. Every would-be SKIP is a FAIL.
    Release,
}

impl Profile {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "dev" => Some(Self::Dev),
            "full" => Some(Self::Full),
            "release" => Some(Self::Release),
            _ => None,
        }
    }
}

/// A reviewed dated nightly: exactly `nightly-YYYY-MM-DD`.
///
/// The moving alias `nightly`, another channel, a host-qualified triple, and a
/// custom rustup alias are all rejected. Release evidence must name a toolchain
/// that cannot move underneath a later re-run.
pub fn is_dated_nightly(name: &str) -> bool {
    let Some(date) = name.strip_prefix("nightly-") else {
        return false;
    };
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && year.bytes().all(|b| b.is_ascii_digit())
        && month
            .parse::<u8>()
            .ok()
            .is_some_and(|value| (1..=12).contains(&value))
        && day
            .parse::<u8>()
            .ok()
            .is_some_and(|value| (1..=31).contains(&value))
}

/// Release evidence must be a complete matrix against a reviewed dated nightly.
///
/// `--only` can otherwise exit 0 after running a subset, which would present a
/// partial run as release evidence. Any `--nightly` value other than
/// `nightly-YYYY-MM-DD` is the same class of hole: a moving alias or a
/// different installed toolchain can still produce a green log.
pub fn validate_release(profile: Profile, only: &[String], nightly: &str) -> Result<(), String> {
    if profile != Profile::Release {
        return Ok(());
    }
    if !only.is_empty() {
        return Err(
            "the release profile cannot combine with --only; a partial run is not release evidence"
                .to_string(),
        );
    }
    if !is_dated_nightly(nightly) {
        return Err(
            "the release profile requires --nightly nightly-YYYY-MM-DD, not the moving alias"
                .to_string(),
        );
    }
    Ok(())
}

impl fmt::Display for Profile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Dev => "dev",
            Self::Full => "full",
            Self::Release => "release",
        })
    }
}

/// A check reports exactly one of three states. `Skip` carries its reason, which
/// the shell gate's bare `return 2` could not.
pub enum Outcome {
    Pass,
    Skip(String),
    Fail(String),
}

impl Outcome {
    pub fn skip(reason: impl Into<String>) -> Self {
        Self::Skip(reason.into())
    }

    pub fn fail(reason: impl Into<String>) -> Self {
        Self::Fail(reason.into())
    }
}

/// Everything a check is allowed to depend on.
///
/// `root` is explicit rather than a process-wide `cd`, which is what lets the
/// mutation tests point a check at a mutated copy of the tree and call it
/// directly instead of re-entering the whole gate as a subprocess.
pub struct Ctx {
    pub root: PathBuf,
    pub profile: Profile,
    pub nightly: String,
    pub skip_embedded: bool,
}

impl Ctx {
    /// Release evidence mode: a would-be SKIP is recorded as a FAIL.
    pub fn strict(&self) -> bool {
        self.profile == Profile::Release
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

pub struct Check {
    pub name: &'static str,
    pub profiles: &'static [Profile],
    pub run: fn(&Ctx) -> Outcome,
}

impl Check {
    fn selected(&self, ctx: &Ctx, only: &[String]) -> bool {
        if !only.is_empty() {
            return only.iter().any(|wanted| wanted == self.name);
        }
        self.profiles.contains(&ctx.profile)
    }
}

#[derive(Default)]
struct Report {
    lines: Vec<String>,
    failed: usize,
    skipped: usize,
}

impl Report {
    fn record(&mut self, verdict: &str, name: &str, note: &str) {
        self.lines.push(format!("  {verdict}  {name}{note}"));
    }

    fn print(&self) {
        println!("\nSummary");
        for line in &self.lines {
            println!("{line}");
        }
        if self.skipped > 0 {
            println!(
                "\n{} check(s) SKIPPED. A skipped check is not a passed check.",
                self.skipped
            );
            println!(
                "Install the missing tool or target and re-run before treating this as verified."
            );
        }
    }
}

/// Run the selected checks and return the process exit code.
///
/// Every check runs even after an earlier failure unless `fail_fast` is set,
/// so one run reports every problem rather than only the first.
pub fn run(ctx: &Ctx, checks: &[Check], only: &[String], fail_fast: bool) -> i32 {
    let mut report = Report::default();

    for check in checks {
        if !check.selected(ctx, only) {
            continue;
        }

        println!("\n==> {}", check.name);
        // Subprocesses inherit this stdout, so flush before they can interleave.
        let _ = io::stdout().flush();

        match (check.run)(ctx) {
            Outcome::Pass => report.record("PASS", check.name, ""),
            Outcome::Skip(reason) => {
                if ctx.strict() {
                    println!("{reason}");
                    eprintln!(
                        "{} cannot be skipped in the {} profile.",
                        check.name, ctx.profile
                    );
                    report.record("FAIL", check.name, " (would skip)");
                    report.failed += 1;
                    if fail_fast {
                        report.print();
                        return finish(&report);
                    }
                    continue;
                }
                println!("{reason}");
                report.record("SKIP", check.name, "");
                report.skipped += 1;
            }
            Outcome::Fail(reason) => {
                eprintln!("{reason}");
                report.record("FAIL", check.name, "");
                report.failed += 1;
                if fail_fast {
                    report.print();
                    return finish(&report);
                }
            }
        }
    }

    if !only.is_empty() {
        let known: Vec<&str> = checks.iter().map(|check| check.name).collect();
        for wanted in only {
            if !known.contains(&wanted.as_str()) {
                eprintln!("no such check: {wanted}");
                return 1;
            }
        }
    }

    report.print();
    finish(&report)
}

fn finish(report: &Report) -> i32 {
    if report.failed > 0 {
        println!("\n{} check(s) failed.", report.failed);
        return 1;
    }
    println!("\nAll runnable checks passed.");
    0
}

/// Print the registry, so `--only` targets can be discovered without reading
/// the source.
pub fn list(checks: &[Check]) {
    for check in checks {
        let profiles: Vec<String> = check
            .profiles
            .iter()
            .map(|profile| profile.to_string())
            .collect();
        println!("{:<34} {}", check.name, profiles.join(","));
    }
}

/// Walk up from `start` to the directory holding the repository manifest.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut cursor = Some(start);
    while let Some(directory) = cursor {
        if directory.join("Cargo.toml").is_file() && directory.join("src/lib.rs").is_file() {
            return Some(directory.to_path_buf());
        }
        cursor = directory.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dated_nightly_is_exactly_nightly_yyyy_mm_dd() {
        assert!(is_dated_nightly("nightly-2026-08-08"));
        assert!(is_dated_nightly("nightly-2020-01-31"));
        assert!(!is_dated_nightly("nightly"));
        assert!(!is_dated_nightly("stable"));
        assert!(!is_dated_nightly("beta"));
        assert!(!is_dated_nightly("nightly-YYYY-MM-DD"));
        assert!(!is_dated_nightly("nightly-2026-8-8"));
        assert!(!is_dated_nightly(
            "nightly-2026-08-08-x86_64-unknown-linux-gnu"
        ));
        assert!(!is_dated_nightly("nightly-2026-13-01"));
        assert!(!is_dated_nightly("nightly-2026-00-01"));
        assert!(!is_dated_nightly("nightly-2026-01-00"));
        assert!(!is_dated_nightly("nightly-2026-01-32"));
        assert!(!is_dated_nightly("+nightly-2026-08-08"));
        assert!(!is_dated_nightly(""));
    }

    #[test]
    fn release_rejects_partial_selection_and_undated_toolchains() {
        let dated = "nightly-2026-08-08";
        assert!(validate_release(Profile::Release, &[], dated).is_ok());
        assert!(
            validate_release(Profile::Release, &["fmt".into()], dated)
                .unwrap_err()
                .contains("--only")
        );
        assert!(validate_release(Profile::Release, &[], "nightly").is_err());
        assert!(validate_release(Profile::Release, &[], "stable").is_err());
        assert!(
            validate_release(
                Profile::Release,
                &[],
                "nightly-2026-08-08-x86_64-unknown-linux-gnu"
            )
            .is_err()
        );
        assert!(validate_release(Profile::Full, &["fmt".into()], "nightly").is_ok());
        assert!(validate_release(Profile::Dev, &[], "nightly").is_ok());
    }
}
