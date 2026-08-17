//! Full-range operands: axes containing `0` and `u16::MAX`, grids containing
//! `i32::MIN` and `i32::MAX`. Every expected value is representable, and the
//! reference asserts as much by narrowing with `i32::try_from`.
//!
//! The `EXTREME` domain has 2^32 points and is *sampled*, never enumerated;
//! the sampling rule is `reference::sample_axis(0, u16::MAX, 1_024)` on each
//! axis (a stride of 63 plus both endpoints, their neighbours, and the two
//! middle coordinates), about 1_030 x 1_030 points. No exhaustive claim is
//! made for it. The tiny end cells of `EDGES` are enumerated exhaustively.

use crate::fixtures::*;
use crate::reference;

#[test]
fn the_extreme_corners_return_their_stored_values() {
    assert_eq!(EXTREME.evaluate(0, 0), Ok(i32::MIN));
    assert_eq!(EXTREME.evaluate(u16::MAX, 0), Ok(i32::MAX));
    assert_eq!(EXTREME.evaluate(0, u16::MAX), Ok(i32::MAX));
    assert_eq!(EXTREME.evaluate(u16::MAX, u16::MAX), Ok(i32::MIN));
}

#[test]
fn the_extreme_saddle_centre_rounds_away_from_zero() {
    // MAX - MIN = 65_535 * 65_537, so at x = 32_767 both rows are exact:
    // lower row MIN + 65_537 * 32_767 = -32_769, upper row MAX - 65_537 *
    // 32_767 = 32_768. The Y step at y = 32_767 then has numerator
    // -32_769 * 65_535 + 65_537 * 32_767 = -65_536 over span 65_535, which is
    // -1.00002 and rounds to the documented -1.
    let mid = u16::MAX / 2;
    assert_eq!(EXTREME.evaluate(mid, mid), Ok(-1));
    assert_eq!(
        EXTREME.evaluate(mid, mid),
        reference::evaluate(&EXTREME, mid, mid)
    );
    // The three neighbours across the centre, where the saddle changes sign.
    assert_eq!(
        EXTREME.evaluate(mid + 1, mid + 1),
        reference::evaluate(&EXTREME, mid + 1, mid + 1)
    );
    assert_eq!(
        EXTREME.evaluate(mid + 1, mid),
        reference::evaluate(&EXTREME, mid + 1, mid)
    );
    assert_eq!(
        EXTREME.evaluate(mid, mid + 1),
        reference::evaluate(&EXTREME, mid, mid + 1)
    );
}

#[test]
fn full_width_endpoints_and_neighbours_agree_with_the_reference() {
    // Full-width endpoint cases: the coordinate one in from each end on the
    // widest span, where the weights are 1 and 65_534.
    for x in [0, 1, u16::MAX - 1, u16::MAX] {
        for y in [0, 1, u16::MAX - 1, u16::MAX] {
            assert_eq!(
                EXTREME.evaluate(x, y),
                reference::evaluate(&EXTREME, x, y),
                "({x}, {y})"
            );
        }
    }
    // Hand values on the lower row (MIN to MAX over 65_535): one step in is
    // MIN + 4_294_967_295 / 65_535 = MIN + 65_537 exactly.
    assert_eq!(EXTREME.evaluate(1, 0), Ok(i32::MIN + 65_537));
    assert_eq!(EXTREME.evaluate(u16::MAX - 1, 0), Ok(i32::MAX - 65_537));
}

#[test]
fn a_sampled_sweep_of_the_full_range_agrees_with_the_reference() {
    // Sampled, per the module documentation. No exhaustive claim.
    let xs = reference::sample_axis(0, u16::MAX, 1_024);
    let ys = reference::sample_axis(0, u16::MAX, 1_024);
    assert!(xs.len() > 1_024 && xs.contains(&0) && xs.contains(&u16::MAX));
    let (lo, hi) = reference::value_hull(&EXTREME);
    for &y in &ys {
        for &x in &xs {
            let value = EXTREME.evaluate(x, y);
            assert_eq!(value, reference::evaluate(&EXTREME, x, y), "({x}, {y})");
            let value = value.expect("in domain");
            assert!((lo..=hi).contains(&value));
        }
    }
}

#[test]
fn axes_containing_zero_and_u16_max_as_knots_evaluate_their_end_cells_exhaustively() {
    // The X cells [0, 1] and [65_534, 65_535] are one unit wide, so every X
    // coordinate in them is a knot; every Y in [0, 4] is enumerated. That is
    // the whole declared domain of these two end columns, exhaustively.
    for x in [0u16, 1, 65_534, 65_535] {
        for y in 0..=4u16 {
            assert_eq!(
                EDGES.evaluate(x, y),
                reference::evaluate(&EDGES, x, y),
                "({x}, {y})"
            );
        }
    }
    // Knots hold the extremes and return them exactly.
    assert_eq!(EDGES.evaluate(0, 0), Ok(i32::MIN));
    assert_eq!(EDGES.evaluate(65_535, 0), Ok(i32::MAX));
    assert_eq!(EDGES.evaluate(0, 2), Ok(i32::MAX));
    assert_eq!(EDGES.evaluate(65_535, 2), Ok(i32::MIN));
    // Between i32::MIN and 0 over a unit X cell there is nothing to
    // interpolate: both are knots. Along Y between MIN (y = 0) and MAX (y = 2)
    // at y = 1: (MIN + MAX) / 2 = -0.5 -> -1, a negative tie at full width.
    assert_eq!(EDGES.evaluate(0, 1), Ok(-1));
    // Between MAX (y = 0) and MIN (y = 2) on the x = 65_535 column, y = 1 is
    // the same numerator, so also -1: the tie rounds by sign of the value, not
    // by direction of travel.
    assert_eq!(EDGES.evaluate(65_535, 1), Ok(-1));
    // Between MIN + 1 (y = 4) and MAX (y = 2) on x = 0... row 2 col 0 is MIN+1,
    // row 1 col 0 is MAX: y = 3 is (MAX + MIN + 1) / 2 = 0 exactly.
    assert_eq!(EDGES.evaluate(0, 3), Ok(0));
}

#[test]
fn the_wide_interior_cells_of_the_edge_grid_agree_with_a_sampled_sweep() {
    // The interior X cells [1, 32_768] and [32_768, 65_534] are wide, so they
    // are sampled: every Y (0..=4) against a 512-step sample of X. No
    // exhaustive claim for these two cells.
    let xs = reference::sample_axis(1, 65_534, 512);
    for &x in &xs {
        for y in 0..=4u16 {
            assert_eq!(
                EDGES.evaluate(x, y),
                reference::evaluate(&EDGES, x, y),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn every_full_range_result_stays_inside_the_hull_of_its_cell() {
    // The v0.1 proof: each rounded step lies in the convex hull of its two
    // endpoints, so no result leaves the hull of the four corner values. On the
    // EXTREME saddle the hull is the whole i32 range, which is why the proof
    // has no overflow variant to report; on EDGES the end cells have narrower
    // hulls that are checked here.
    let xs = reference::sample_axis(0, u16::MAX, 256);
    for &x in &xs {
        for y in 0..=4u16 {
            let value = EDGES.evaluate(x, y).expect("in domain");
            let (lo, hi) = reference::value_hull(&EDGES);
            assert!((lo..=hi).contains(&value));
        }
    }
    // The [65_534, 65_535] x [2, 4] cell has values 1, MIN, MIN + 1, MAX - 1;
    // its hull is [MIN, MAX - 1] and every point of the cell (2 x 3 = 6, all
    // enumerated) stays inside it.
    for x in [65_534u16, 65_535] {
        for y in 2..=4u16 {
            let value = EDGES.evaluate(x, y).expect("in domain");
            assert!(value < i32::MAX, "({x}, {y}) = {value}");
        }
    }
}
