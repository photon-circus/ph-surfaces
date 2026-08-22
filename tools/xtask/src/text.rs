//! File reading and the small pattern scanners the source ratchets need.
//!
//! `scripts/ci.sh` expressed these as `grep -E` over `find | sort -z | xargs -0
//! cat` pipelines with hand-built POSIX regexes. Those pipelines needed
//! `tr -d '\r'` at some sites and not others, which is how a CRLF checkout could
//! turn a FAIL into a SKIP. Reading files here means CR handling is decided
//! once, in `read_text`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One physical line of one file, carrying enough context to report it the way
/// `grep -n` did.
pub struct Line {
    pub path: PathBuf,
    pub number: usize,
    pub text: String,
}

impl Line {
    /// `path:line: text`, the shape the shell gate printed on a hit.
    pub fn render(&self) -> String {
        format!(
            "{}:{}: {}",
            self.path.display().to_string().replace('\\', "/"),
            self.number,
            self.text
        )
    }
}

/// Read a file as text with CR stripped, so every caller sees LF regardless of
/// how the worktree was checked out. The `line endings` check is what asserts
/// the bytes on disk really are LF; this is about never miscomparing.
pub fn read_text(path: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&fs::read(path)?).replace('\r', ""))
}

/// Every `*.rs` under `dir`, recursively, in a stable sorted order.
pub fn rust_sources(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    if dir.is_dir() {
        collect(dir, &mut found)?;
    }
    found.sort();
    Ok(found)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Lines of every `*.rs` under each directory, with full-line comments removed.
///
/// Doc comments legitimately discuss floating point, allocation, features, and
/// the banned crate by name; none of that is a code path. A trailing comment on
/// a code line is *not* stripped, matching the shell gate.
pub fn code_lines(root: &Path, dirs: &[&str]) -> io::Result<Vec<Line>> {
    let mut lines = Vec::new();
    for dir in dirs {
        for path in rust_sources(&root.join(dir))? {
            let text = read_text(&path)?;
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            for (index, raw) in text.lines().enumerate() {
                let trimmed = raw.trim_start();
                if trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('*')
                {
                    continue;
                }
                lines.push(Line {
                    path: relative.clone(),
                    number: index + 1,
                    text: raw.to_string(),
                });
            }
        }
    }
    Ok(lines)
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn word_before(bytes: &[u8], at: usize) -> bool {
    at > 0 && is_word(bytes[at - 1])
}

fn word_after(bytes: &[u8], at: usize) -> bool {
    at < bytes.len() && is_word(bytes[at])
}

/// `\bneedle` -- `needle` not preceded by a word character.
pub fn contains_at_word_start(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices(needle)
        .any(|(at, _)| !word_before(bytes, at))
}

/// `\bneedle\b` -- `needle` as a whole word.
pub fn contains_word(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices(needle)
        .any(|(at, _)| !word_before(bytes, at) && !word_after(bytes, at + needle.len()))
}

/// `\bname[[:space:]]*!` -- a macro invocation of `name`.
pub fn contains_macro_call(line: &str, name: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices(name).any(|(at, _)| {
        if word_before(bytes, at) {
            return false;
        }
        let mut cursor = at + name.len();
        while matches!(bytes.get(cursor), Some(b' ' | b'\t')) {
            cursor += 1;
        }
        bytes.get(cursor) == Some(&b'!')
    })
}

/// `extern[[:space:]]+crate[[:space:]]+<name>`.
pub fn contains_extern_crate(line: &str, name: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices("extern").any(|(at, _)| {
        if word_before(bytes, at) {
            return false;
        }
        let after_extern = &line[at + "extern".len()..];
        let trimmed = after_extern.trim_start_matches([' ', '\t']);
        if trimmed.len() == after_extern.len() {
            return false;
        }
        let Some(after_crate) = trimmed.strip_prefix("crate") else {
            return false;
        };
        let trimmed = after_crate.trim_start_matches([' ', '\t']);
        if trimmed.len() == after_crate.len() {
            return false;
        }
        trimmed
            .strip_prefix(name)
            .is_some_and(|rest| !rest.as_bytes().first().copied().is_some_and(is_word))
    })
}

/// How the two ph-curves ratchets spell the separator.
///
/// The source ratchet used `ph.curves` -- any character -- because a code path
/// naming the crate at all is a finding. The manifest ratchet used
/// `ph[-_]curves`, the two spellings that actually resolve to the crate.
#[derive(Clone, Copy)]
pub enum Separator {
    Any,
    DashOrUnderscore,
}

/// The banned crate name, under the given separator rule.
pub fn contains_ph_curves(line: &str, separator: Separator, ignore_case: bool) -> bool {
    let haystack = if ignore_case {
        line.to_ascii_lowercase()
    } else {
        line.to_string()
    };
    let bytes = haystack.as_bytes();
    haystack.match_indices("ph").any(|(at, _)| {
        let tail = at + 3;
        if haystack.len() < tail + 6 || &haystack[tail..tail + 6] != "curves" {
            return false;
        }
        match separator {
            Separator::Any => true,
            Separator::DashOrUnderscore => matches!(bytes[at + 2], b'-' | b'_'),
        }
    })
}

/// The byte offset of the first Rust float literal on the line, if any.
///
/// The shell gate needed two alternating POSIX regexes for this. The awkward
/// case is that `1..2` is an integer range, not a trailing-dot float.
pub fn find_float_literal(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() || word_before(bytes, at) {
            at += 1;
            continue;
        }
        if float_literal_end(bytes, at).is_some() {
            return Some(at);
        }
        // Skip the whole numeric run so `0xf32` cannot be re-entered partway
        // and mistaken for a suffixed literal.
        while at < bytes.len() && (is_word(bytes[at]) || bytes[at] == b'.') {
            at += 1;
        }
    }
    None
}

/// The end offset of a float literal starting at `start`, if there is one.
fn float_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let digits = |from: usize| {
        let mut cursor = from;
        while matches!(bytes.get(cursor), Some(byte) if byte.is_ascii_digit() || *byte == b'_') {
            cursor += 1;
        }
        cursor
    };

    let mut cursor = digits(start);
    let mut fractional = false;

    if bytes.get(cursor) == Some(&b'.') {
        let after_dot = digits(cursor + 1);
        if after_dot > cursor + 1 {
            cursor = after_dot;
            fractional = true;
        } else {
            // `1.` is a float only when what follows is neither a word
            // character nor another `.`, which keeps `1..2` an integer range
            // and `1.method()` a method call.
            return match bytes.get(cursor + 1) {
                None => Some(cursor + 1),
                Some(&byte) if !is_word(byte) && byte != b'.' => Some(cursor + 1),
                _ => None,
            };
        }
    }

    let mut exponent = false;
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        let mut probe = cursor + 1;
        if matches!(bytes.get(probe), Some(b'+' | b'-')) {
            probe += 1;
        }
        let after_exponent = digits(probe);
        if after_exponent > probe {
            cursor = after_exponent;
            exponent = true;
        }
    }

    let mut suffix = false;
    let mut probe = cursor;
    if bytes.get(probe) == Some(&b'_') {
        probe += 1;
    }
    if bytes.len() >= probe + 3 && matches!(&bytes[probe..probe + 3], b"f32" | b"f64") {
        cursor = probe + 3;
        suffix = true;
    }

    if !(fractional || exponent || suffix) || word_after(bytes, cursor) {
        return None;
    }
    Some(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_forms_are_not_float_literals() {
        for line in [
            "let range = 1..2;",
            "let value: u16 = 1_000;",
            "let mask = 0xf32;",
            "let axis_1 = index32;",
            "const LIMIT: i32 = 423;",
            "arr[1].get()",
        ] {
            assert_eq!(find_float_literal(line), None, "{line}");
        }
    }

    #[test]
    fn every_float_shape_is_caught() {
        for line in [
            "let x = 1.5;",
            "let x = 4.23e14;",
            "let x = 1e5;",
            "let x = 1f32;",
            "let x = 1_f64;",
            "let x = 2.;",
            "let x = 1.0E-3;",
        ] {
            assert!(find_float_literal(line).is_some(), "{line}");
        }
    }

    #[test]
    fn word_scanners_respect_boundaries() {
        assert!(contains_word("let x: f32 = y;", "f32"));
        assert!(!contains_word("let xf32y = 1;", "f32"));
        assert!(contains_at_word_start("use alloc::vec::Vec;", "alloc::"));
        assert!(!contains_at_word_start("use myalloc::thing;", "alloc::"));
        assert!(contains_macro_call("println !(\"hi\");", "println"));
        assert!(!contains_macro_call("let printlnx = 1;", "println"));
        assert!(contains_extern_crate("extern  crate  alloc;", "alloc"));
        assert!(!contains_extern_crate("extern crate allocator;", "alloc"));
        assert!(contains_ph_curves(
            "ph-curves",
            Separator::DashOrUnderscore,
            false
        ));
        assert!(contains_ph_curves(
            "ph_curves",
            Separator::DashOrUnderscore,
            false
        ));
        assert!(!contains_ph_curves(
            "ph curves",
            Separator::DashOrUnderscore,
            false
        ));
        assert!(contains_ph_curves("ph curves", Separator::Any, false));
        assert!(!contains_ph_curves("ph--curves", Separator::Any, false));
    }
}
