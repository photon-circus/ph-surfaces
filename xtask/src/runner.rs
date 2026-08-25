//! The check registry protocol and the aggregate report.
//!
//! `RELEASING.md` extracts the last `Summary` block from a teed run and rejects
//! `FAIL` or `SKIP` rows there, so the heading and two-leading-space summary
//! lines below are a machine-read contract, not cosmetics. Reformatting them
//! silently disarms the release gate.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anstyle::{AnsiColor, Effects, Style};
use serde::Deserialize;
use time::Date;
use time::macros::format_description;

use crate::config::{CheckSpec, Config, OptIn};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Deserialize)]
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
    date.len() == 10 && Date::parse(date, format_description!("[year]-[month]-[day]")).is_ok()
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
    PassWithNote(String),
    Skip(String),
    Fail(String),
}

impl Outcome {
    pub fn pass_with_note(note: impl Into<String>) -> Self {
        Self::PassWithNote(note.into())
    }

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
    pub coverage: bool,
    pub config: Arc<Config>,
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

impl CheckSpec {
    fn selected(&self, ctx: &Ctx, only: &[String]) -> bool {
        if !only.is_empty() {
            return only.iter().any(|wanted| wanted == &self.name);
        }
        self.profiles.contains(&ctx.profile)
            && match self.opt_in {
                None => true,
                Some(OptIn::Coverage) => ctx.coverage,
            }
    }
}

#[derive(Clone, Copy)]
enum Verdict {
    Pass,
    Skip,
    Fail,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Skip => "SKIP",
            Self::Fail => "FAIL",
        }
    }

    fn style(self) -> Style {
        let color = match self {
            Self::Pass => AnsiColor::Green,
            Self::Skip => AnsiColor::Yellow,
            Self::Fail => AnsiColor::Red,
        };
        Style::new()
            .fg_color(Some(color.into()))
            .effects(Effects::BOLD)
    }
}

struct ReportLine {
    verdict: Verdict,
    name: String,
    note: Option<String>,
}

impl ReportLine {
    #[cfg(test)]
    fn plain(&self) -> String {
        match &self.note {
            Some(note) => format!("  {}  {} — {note}", self.verdict.label(), self.name),
            None => format!("  {}  {}", self.verdict.label(), self.name),
        }
    }
}

#[derive(Default)]
struct Report {
    lines: Vec<ReportLine>,
    failed: usize,
    skipped: usize,
}

impl Report {
    fn record(&mut self, verdict: Verdict, name: &str, note: Option<String>) {
        self.lines.push(ReportLine {
            verdict,
            name: name.to_owned(),
            note,
        });
    }

    fn print(&self) {
        let heading = Style::new()
            .fg_color(Some(AnsiColor::Cyan.into()))
            .effects(Effects::BOLD);
        anstream::println!("\n{}Summary{}", heading.render(), heading.render_reset());
        for line in &self.lines {
            let style = line.verdict.style();
            let note = line
                .note
                .as_deref()
                .map(|note| format!(" — {note}"))
                .unwrap_or_default();
            anstream::println!(
                "  {}{}{}  {}{}",
                style.render(),
                line.verdict.label(),
                style.render_reset(),
                line.name,
                note
            );
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

fn summary_reason(reason: &str) -> String {
    reason
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Run the selected checks and return the process exit code.
///
/// Every check runs even after an earlier failure unless `fail_fast` is set,
/// so one run reports every problem rather than only the first.
pub fn run(ctx: &Ctx, checks: &[CheckSpec], only: &[String], fail_fast: bool) -> i32 {
    let mut report = Report::default();

    for check in checks {
        if !check.selected(ctx, only) {
            continue;
        }

        println!("\n==> {}", check.name);
        // Subprocesses inherit this stdout, so flush before they can interleave.
        let _ = io::stdout().flush();

        match crate::checks::run_action(ctx, &check.action) {
            Outcome::Pass => report.record(Verdict::Pass, &check.name, None),
            Outcome::PassWithNote(note) => {
                report.record(Verdict::Pass, &check.name, Some(summary_reason(&note)));
            }
            Outcome::Skip(reason) => {
                let reason_summary = summary_reason(&reason);
                if ctx.strict() {
                    println!("{reason}");
                    eprintln!(
                        "{} cannot be skipped in the {} profile.",
                        check.name, ctx.profile
                    );
                    report.record(
                        Verdict::Fail,
                        &check.name,
                        Some(format!("would skip: {reason_summary}")),
                    );
                    report.failed += 1;
                    if fail_fast {
                        report.print();
                        return finish(&report);
                    }
                    continue;
                }
                println!("{reason}");
                report.record(Verdict::Skip, &check.name, Some(reason_summary));
                report.skipped += 1;
            }
            Outcome::Fail(reason) => {
                eprintln!("{reason}");
                report.record(Verdict::Fail, &check.name, None);
                report.failed += 1;
                if fail_fast {
                    report.print();
                    return finish(&report);
                }
            }
        }
    }

    if !only.is_empty() {
        let known: Vec<&str> = checks.iter().map(|check| check.name.as_str()).collect();
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
        let style = Verdict::Fail.style();
        anstream::println!(
            "\n{}{} check(s) failed.{}",
            style.render(),
            report.failed,
            style.render_reset()
        );
        return 1;
    }
    let style = Verdict::Pass.style();
    anstream::println!(
        "\n{}All runnable checks passed.{}",
        style.render(),
        style.render_reset()
    );
    0
}

/// Print the registry, so `--only` targets can be discovered without reading
/// the source.
pub fn list(checks: &[CheckSpec]) {
    for check in checks {
        let profiles: Vec<String> = check
            .profiles
            .iter()
            .map(|profile| profile.to_string())
            .collect();
        let opt_in = match check.opt_in {
            None => "",
            Some(OptIn::Coverage) => " [--coverage]",
        };
        println!("{:<34} {}{opt_in}", check.name, profiles.join(","));
    }
}

/// Walk up from `start` to the directory holding the repository manifest.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut cursor = Some(start);
    while let Some(directory) = cursor {
        if directory.join("xtask/config.ron").is_file()
            && directory.join("crates/surfaces/Cargo.toml").is_file()
        {
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
        assert!(!is_dated_nightly("nightly-2026-02-31"));
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

    #[test]
    fn coverage_is_opt_in_but_only_selects_it_explicitly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits one level below the repository root")
            .to_path_buf();
        let config = Arc::new(Config::load(&root).unwrap());
        let coverage = config
            .checks
            .iter()
            .find(|check| check.name == "coverage")
            .unwrap();
        let mut ctx = Ctx {
            root,
            profile: Profile::Full,
            nightly: "nightly".to_string(),
            skip_embedded: false,
            coverage: false,
            config: Arc::clone(&config),
        };

        assert!(!coverage.selected(&ctx, &[]));
        assert!(coverage.selected(&ctx, &["coverage".to_string()]));
        ctx.coverage = true;
        assert!(coverage.selected(&ctx, &[]));
    }

    #[test]
    fn skip_summary_preserves_the_machine_prefix_and_explains_the_skip() {
        let line = ReportLine {
            verdict: Verdict::Skip,
            name: "embedded build".to_string(),
            note: Some(summary_reason(
                "target is not installed\ninstall it with rustup target add",
            )),
        };

        assert_eq!(
            line.plain(),
            "  SKIP  embedded build — target is not installed; install it with rustup target add"
        );
    }
}
