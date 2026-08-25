//! Frozen baker vectors. Integer CSV only; not a float oracle.
//!
//! `crates/surfaces/tests/conformance/golden/*.csv` are frozen inputs. A
//! failing test is an implementation defect until proven otherwise. Do not
//! edit a golden or this module to silence a failure.

use ph_surfaces::BilinearSurface;

/// S4 rounding table: knots `[0, 2]`, values `[[0, 1], [1, 2]]`.
static X: [u16; 2] = [0, 2];
static Y: [u16; 2] = [0, 2];
static VALUES: [[i32; 2]; 2] = [[0, 1], [1, 2]];
static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

const ROUNDING: &str = include_str!("golden/rounding.csv");

#[test]
fn baker_rounding_golden_matches_public_evaluate_on_the_declared_domain() {
    let mut rows = 0usize;
    for line in ROUNDING.lines() {
        if line.is_empty() || line == "x,y,expected" {
            continue;
        }
        let mut parts = line.split(',');
        let x: u16 = parts
            .next()
            .and_then(|t| t.parse().ok())
            .expect("golden x is u16");
        let y: u16 = parts
            .next()
            .and_then(|t| t.parse().ok())
            .expect("golden y is u16");
        let expected: i32 = parts
            .next()
            .and_then(|t| t.parse().ok())
            .expect("golden expected is i32");
        assert!(parts.next().is_none(), "golden rows are x,y,expected");
        assert_eq!(SURFACE.evaluate(x, y), Ok(expected));
        rows += 1;
    }
    assert_eq!(rows, 9);
}
