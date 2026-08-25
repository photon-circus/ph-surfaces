//! Frozen integer golden vectors for the runtime conformance suite.
//!
//! `--emit-golden` writes the checked-in rounding fixture as CSV under
//! `crates/surfaces/tests/conformance/golden/` from the working directory, or
//! `--out DIR`. It does not ingest caller samples. Those files are frozen
//! inputs: a failing test is an implementation defect until proven otherwise.
//! Regenerating them belongs in a dedicated freeze commit with no
//! implementation source. This is not `MAX_ERR_LSB`.

use std::fmt::Write as _;
use std::path::Path;

use crate::emit::checked_in_input;
use crate::quantize::QuantizedTable;

/// Write every checked-in golden CSV under `dir`.
///
/// # Errors
///
/// Returns an I/O error when the directory cannot be created or a file cannot
/// be written.
pub fn write_goldens(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("rounding.csv"), rounding_csv())?;
    Ok(())
}

/// Byte-stable CSV for the S4 rounding table: every `u16` in the declared
/// domain, `x,y,expected` from the runtime X-then-Y path.
#[must_use]
pub fn rounding_csv() -> String {
    evaluate_csv(&checked_in_table())
}

fn checked_in_table() -> QuantizedTable {
    checked_in_input()
        .quantize()
        .expect("checked-in fixture must quantize")
}

fn evaluate_csv(table: &QuantizedTable) -> String {
    let mut out = String::from("x,y,expected\n");
    let x0 = table.x[0];
    let x1 = *table.x.last().expect("validated axes have two knots");
    let y0 = table.y[0];
    let y1 = *table.y.last().expect("validated axes have two knots");
    for y in y0..=y1 {
        for x in x0..=x1 {
            let _ = writeln!(out, "{x},{y},{}", table.evaluate_u16(x, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{checked_in_table, rounding_csv, write_goldens};
    use crate::emit::checked_in_input;

    #[test]
    fn rounding_csv_is_byte_stable() {
        assert_eq!(rounding_csv(), rounding_csv());
        assert!(rounding_csv().starts_with("x,y,expected\n"));
        assert!(rounding_csv().contains("1,1,2\n"));
    }

    #[test]
    fn tracked_rounding_csv_matches_the_generator_without_rewriting() {
        let Some(on_disk) = freeze_csv() else {
            return;
        };
        assert_eq!(on_disk, rounding_csv());
    }

    #[test]
    fn write_goldens_matches_rounding_csv() {
        let dir = std::env::temp_dir().join("ph-surfaces-bake-golden");
        write_goldens(&dir).unwrap();
        let got = std::fs::read_to_string(dir.join("rounding.csv")).unwrap();
        assert_eq!(got, rounding_csv());
    }

    #[test]
    fn baker_u16_path_matches_the_frozen_sweep() {
        let table = checked_in_table();
        for row in rounding_csv().lines().skip(1) {
            if row.is_empty() {
                continue;
            }
            let mut parts = row.split(',');
            let px: u16 = parts.next().unwrap().parse().unwrap();
            let py: u16 = parts.next().unwrap().parse().unwrap();
            let expected: i32 = parts.next().unwrap().parse().unwrap();
            assert_eq!(table.evaluate_u16(px, py), expected);
        }
    }

    #[test]
    fn host_model_stays_within_max_err_lsb_on_the_u16_sweep_and_samples() {
        let input = checked_in_input();
        let table = input.quantize().unwrap();
        let bound = f64::from(table.max_err_lsb);
        let x0 = table.x[0];
        let x1 = *table.x.last().unwrap();
        let y0 = table.y[0];
        let y1 = *table.y.last().unwrap();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let runtime = f64::from(table.evaluate_u16(x, y));
                let host = table.reconstruct(f64::from(x), f64::from(y)) * table.scale;
                assert!((runtime - host).abs() <= bound);
            }
        }
        for sample in input.samples() {
            let Some(px) = exact_u16(sample.x) else {
                continue;
            };
            let Some(py) = exact_u16(sample.y) else {
                continue;
            };
            if px < x0 || px > x1 || py < y0 || py > y1 {
                continue;
            }
            let runtime = table.evaluate_u16(px, py);
            let scaled = sample.value * table.scale;
            assert!((f64::from(runtime) - scaled).abs() <= bound);
        }
    }

    fn exact_u16(value: f64) -> Option<u16> {
        if !value.is_finite() || value < 0.0 || value > f64::from(u16::MAX) {
            return None;
        }
        let n = value as u16;
        (f64::from(n) == value).then_some(n)
    }

    fn freeze_csv() -> Option<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../surfaces/tests/conformance/golden/rounding.csv");
        path.is_file()
            .then(|| std::fs::read_to_string(path).expect("readable freeze"))
    }
}
