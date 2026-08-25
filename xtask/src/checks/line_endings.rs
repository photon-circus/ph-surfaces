//! The LF policy, enforced rather than worked around.
//!
//! `.gitattributes` declares `* text=auto eol=lf`, but a worktree created before
//! that policy -- or cloned with `core.autocrlf=true` under an older attributes
//! file -- can still hold CRLF on disk. That is the failure issue #27 opened on:
//! a tracked snapshot input became CRLF while generated output stayed LF, and
//! the comparison broke. The shell gate patched the symptom with `tr -d '\r'` at
//! the comparison sites; this checks the cause.

use std::path::PathBuf;

use crate::proc;
use crate::runner::{Ctx, Outcome};

pub fn line_endings(ctx: &Ctx) -> Outcome {
    let listing = match proc::capture("git", &["ls-files"], &ctx.root) {
        Ok(output) if output.ok() => output.stdout,
        Ok(output) => {
            return Outcome::fail(format!("git ls-files failed.\n{}", output.stderr));
        }
        Err(error) => return Outcome::skip(format!("git is not available: {error}")),
    };

    let mut offenders: Vec<String> = Vec::new();
    for relative in listing.lines().filter(|line| !line.is_empty()) {
        let path = PathBuf::from(&ctx.root).join(relative);
        let Ok(bytes) = std::fs::read(&path) else {
            // A file listed but absent is a sparse or partial checkout, not a
            // line-ending finding.
            continue;
        };
        // NUL means git treats it as binary; CR there is data, not a line ending.
        if bytes.contains(&0) {
            continue;
        }
        if let Some(at) = bytes.iter().position(|byte| *byte == b'\r') {
            let line = 1 + bytes[..at].iter().filter(|byte| **byte == b'\n').count();
            offenders.push(format!("{relative}:{line}: carriage return"));
        }
    }

    if offenders.is_empty() {
        return Outcome::Pass;
    }

    offenders.truncate(20);
    Outcome::fail(format!(
        "{}\ntracked text files must use LF. Re-normalize with:\n  \
         git add --renormalize . && git checkout -- .",
        offenders.join("\n")
    ))
}
