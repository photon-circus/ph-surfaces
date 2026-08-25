//! Delimited sample-point ingest.

use crate::error::BakeError;

/// One measured point: host `f64` coordinates and value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Sample X coordinate.
    pub x: f64,
    /// Sample Y coordinate.
    pub y: f64,
    /// Sample value in the caller's units, before the output scale is applied.
    pub value: f64,
}

impl Sample {
    /// Builds a sample record.
    #[must_use]
    pub const fn new(x: f64, y: f64, value: f64) -> Self {
        Self { x, y, value }
    }
}

/// Parses one X Y value point per non-empty, non-comment line.
///
/// Fields are separated by whitespace and/or commas. Blank lines and lines
/// whose first non-whitespace character is `#` are skipped. There is no
/// CSV crate and no expression language.
///
/// # Errors
///
/// Returns [`BakeError::MalformedLine`] when a kept line is not exactly three
/// finite numbers.
pub fn parse_samples(text: &str) -> Result<Vec<Sample>, BakeError> {
    let mut samples = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        samples.push(parse_line(line, trimmed)?);
    }
    Ok(samples)
}

fn parse_line(line: usize, trimmed: &str) -> Result<Sample, BakeError> {
    let mut parts = split_fields(trimmed);
    let x = parse_finite(line, parts.next())?;
    let y = parse_finite(line, parts.next())?;
    let value = parse_finite(line, parts.next())?;
    if parts.next().is_some() {
        return Err(BakeError::MalformedLine { line });
    }
    Ok(Sample { x, y, value })
}

fn split_fields(trimmed: &str) -> impl Iterator<Item = &str> {
    trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
}

fn parse_finite(line: usize, part: Option<&str>) -> Result<f64, BakeError> {
    let part = part.ok_or(BakeError::MalformedLine { line })?;
    let value: f64 = part
        .parse()
        .map_err(|_| BakeError::MalformedLine { line })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BakeError::MalformedLine { line })
    }
}

#[cfg(test)]
mod tests {
    use super::{Sample, parse_samples};
    use crate::error::BakeError;

    #[test]
    fn whitespace_and_commas_are_accepted() {
        let samples = parse_samples("0 0 1.5\n10,5,2\n20, 10, 3\n").unwrap();
        assert_eq!(
            samples,
            [
                Sample::new(0.0, 0.0, 1.5),
                Sample::new(10.0, 5.0, 2.0),
                Sample::new(20.0, 10.0, 3.0),
            ]
        );
    }

    #[test]
    fn blank_lines_and_hash_comments_are_skipped() {
        let text = "# header\n\n0 0 1\n  # indented comment\n10 5 2\n";
        let samples = parse_samples(text).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], Sample::new(0.0, 0.0, 1.0));
        assert_eq!(samples[1], Sample::new(10.0, 5.0, 2.0));
    }

    #[test]
    fn too_few_fields_is_a_closed_error() {
        assert_eq!(
            parse_samples("0 0\n"),
            Err(BakeError::MalformedLine { line: 1 })
        );
    }

    #[test]
    fn too_many_fields_is_a_closed_error() {
        assert_eq!(
            parse_samples("# skip\n0 0 1 2\n"),
            Err(BakeError::MalformedLine { line: 2 })
        );
    }

    #[test]
    fn an_unparseable_token_is_a_closed_error() {
        assert_eq!(
            parse_samples("0 foo 1\n"),
            Err(BakeError::MalformedLine { line: 1 })
        );
    }

    #[test]
    fn a_non_finite_number_is_a_closed_error() {
        assert_eq!(
            parse_samples("0 0 inf\n"),
            Err(BakeError::MalformedLine { line: 1 })
        );
    }

    #[test]
    fn physical_line_numbers_count_skipped_lines() {
        assert_eq!(
            parse_samples("# comment\n\n0 0\n"),
            Err(BakeError::MalformedLine { line: 3 })
        );
    }
}
