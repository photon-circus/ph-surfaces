//! Numerical behaviour in domain: exact knots, hand-computable cases,
//! nonuniform cells, signed / flat / decreasing data, and half-way rounding.

use crate::fixtures::*;
use crate::reference;

#[test]
fn every_stored_point_of_every_fixture_returns_exactly() {
    reference::assert_every_knot_exact(&ORDER, "ORDER");
    reference::assert_every_knot_exact(&PLANE, "PLANE");
    reference::assert_every_knot_exact(&GRID, "GRID");
    reference::assert_every_knot_exact(&SHAPES, "SHAPES");
    reference::assert_every_knot_exact(&COLUMNS, "COLUMNS");
    reference::assert_every_knot_exact(&TIES, "TIES");
    reference::assert_every_knot_exact(&SIGNED, "SIGNED");
    reference::assert_every_knot_exact(&SIGNED_NEGATED, "SIGNED_NEGATED");
    reference::assert_every_knot_exact(&INSET, "INSET");
    reference::assert_every_knot_exact(&EXTREME, "EXTREME");
    reference::assert_every_knot_exact(&EDGES, "EDGES");
    reference::assert_every_knot_exact(&ELEVATION, "ELEVATION");
    reference::assert_every_knot_exact(&CORRECTION, "CORRECTION");
}

#[test]
fn a_hand_computable_plane_is_exact_at_every_declared_point() {
    // value = 10 * x + 100 * y is a plane, and bilinear interpolation of a
    // plane over integer coordinates is exact: the numerator is always a
    // multiple of the span. So every point of the 11 x 11 domain has a closed
    // form and no rounding is involved.
    for y in 0..=10u16 {
        for x in 0..=10u16 {
            let expected = 10 * i32::from(x) + 100 * i32::from(y);
            assert_eq!(PLANE.evaluate(x, y), Ok(expected), "plane at ({x}, {y})");
        }
    }
}

#[test]
fn a_nonuniform_grid_evaluates_hand_computed_interior_points() {
    // Cell x in [0, 10], y in [0, 4]. At x = 5: lower row (0, 100) -> 50,
    // upper row (10, 110) -> 60. At y = 2: (50 + 60) / 2 = 55.
    assert_eq!(GRID.evaluate(5, 2), Ok(55));
    // Wide X segment [10, 110] at x = 60: (100, 300) -> 200, (110, 310) -> 210,
    // then y = 2 -> 205.
    assert_eq!(GRID.evaluate(60, 2), Ok(205));
    // Upper Y segment [4, 8] at y = 6 on the x = 10 column: (110 + 60) / 2 = 85.
    assert_eq!(GRID.evaluate(10, 6), Ok(85));
    // Interior on both axes: rows (110 + 310) / 2 = 210 and (60 + 260) / 2 =
    // 160, then Y midway = 185.
    assert_eq!(GRID.evaluate(60, 6), Ok(185));
    // A point that rounds: x = 7 in [0, 10] on row y = 0 is 70 exactly, on row
    // y = 4 it is 10 + 70 = 80; y = 1 in [0, 4] gives 70 + 10 / 4 = 72.5 -> 73.
    assert_eq!(GRID.evaluate(7, 1), Ok(73));
    // Same X on the negative-crossing cell: rows y = 4 -> 80, y = 8 -> -40 + 70
    // = 30; y = 5 gives 80 - 50 / 4 = 67.5 -> 68.
    assert_eq!(GRID.evaluate(7, 5), Ok(68));
}

#[test]
fn a_nonuniform_grid_agrees_with_the_reference_over_its_whole_domain() {
    // 111 x 9 = 999 points, exhaustive.
    let count = reference::assert_exhaustive_domain(&GRID, "GRID");
    assert_eq!(count, 111 * 9);
}

#[test]
fn rows_of_every_sign_and_slope_compose_correctly() {
    // Positive increasing row, X midway.
    assert_eq!(SHAPES.evaluate(50, 0), Ok(15));
    // Negative decreasing row, X midway.
    assert_eq!(SHAPES.evaluate(50, 10), Ok(-15));
    // Flat row: every X returns the constant.
    for x in 0..=200u16 {
        assert_eq!(SHAPES.evaluate(x, 20), Ok(7), "flat row at x = {x}");
    }
    // Zero-crossing row: the crossing sits on the middle knot.
    assert_eq!(SHAPES.evaluate(100, 30), Ok(0));
    assert_eq!(SHAPES.evaluate(50, 30), Ok(-50));
    assert_eq!(SHAPES.evaluate(150, 30), Ok(50));
    // Between the positive and negative rows the Y midpoint is zero.
    for x in [0, 100, 200] {
        assert_eq!(SHAPES.evaluate(x, 5), Ok(0));
    }
    // 201 x 31 = 6_231 points, exhaustive.
    reference::assert_exhaustive_domain(&SHAPES, "SHAPES");
}

#[test]
fn a_flat_column_and_decreasing_columns_compose_correctly() {
    // The x = 5 column is -4 on every row, so every Y on it returns -4.
    for y in 0..=9u16 {
        assert_eq!(COLUMNS.evaluate(5, y), Ok(-4), "flat column at y = {y}");
    }
    // The x = 0 column decreases along Y: 30, 10, -30. Between rows 3 and 9 at
    // y = 6: (10 + -30) / 2 = -10.
    assert_eq!(COLUMNS.evaluate(0, 6), Ok(-10));
    // The x = 9 column: 20, 0, -60. y = 1 in [0, 3]: 20 - 20 / 3 = 13.33 -> 13.
    assert_eq!(COLUMNS.evaluate(9, 1), Ok(13));
    // 10 x 10 = 100 points, exhaustive.
    reference::assert_exhaustive_domain(&COLUMNS, "COLUMNS");
}

#[test]
fn positive_and_negative_half_way_values_round_away_from_zero() {
    // Lower row at x = 1: (0 + 1) / 2 = 0.5 -> 1. Upper row: -0.5 -> -1.
    assert_eq!(TIES.evaluate(1, 0), Ok(1));
    assert_eq!(TIES.evaluate(1, 2), Ok(-1));
    // Y between the two rounded results: (1 + -1) / 2 = 0 exactly.
    assert_eq!(TIES.evaluate(1, 1), Ok(0));
    // The X endpoints between rows: (0 + 0) / 2 = 0 and (1 + -1) / 2 = 0.
    assert_eq!(TIES.evaluate(0, 1), Ok(0));
    assert_eq!(TIES.evaluate(2, 1), Ok(0));
    // 3 x 3 = 9 points, exhaustive.
    reference::assert_exhaustive_domain(&TIES, "TIES");
}

#[test]
fn a_tie_at_the_final_y_step_also_rounds_away_from_zero() {
    // The TIES fixture places its ties on the X rows. Here the tie is on the
    // final Y step instead: on the x = 2 knot column of SIGNED the rows hold
    // -2 (y = 0) and 4 (y = 4), so y = 1 is -2 + 6 / 4 = -0.5 -> -1 and y = 3
    // is -2 + 18 / 4 = 2.5 -> 3. Both signs of a final-step tie, and the
    // toward-zero mutant differs on both.
    assert_eq!(SIGNED.evaluate(2, 1), Ok(-1));
    assert_eq!(SIGNED.evaluate(2, 3), Ok(3));
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&SIGNED, 2, 1),
        Ok(0)
    );
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&SIGNED, 2, 3),
        Ok(2)
    );
    // Non-tie interior points for shape: x = 0 column (1, -7) at y = 3 is
    // 1 - 24 / 4 = -5; x = 10 column (5, -3) at y = 1 is 5 - 8 / 4 = 3.
    assert_eq!(SIGNED.evaluate(0, 3), Ok(-5));
    assert_eq!(SIGNED.evaluate(10, 1), Ok(3));
    // 11 x 7 = 77 points, exhaustive.
    reference::assert_exhaustive_domain(&SIGNED, "SIGNED");
}

#[test]
fn negating_the_grid_negates_every_result_exactly() {
    // Ties-away-from-zero is odd-symmetric, so sign reflection is exact at
    // every point, ties included. Rounding toward +inf or half-even would
    // break this on any tie. 11 x 7 = 77 points, exhaustive, and the sweep
    // asserts that at least one tie actually occurred so the property is not
    // vacuous.
    let mut ties = 0;
    for y in 0..=6u16 {
        for x in 0..=10u16 {
            let value = SIGNED.evaluate(x, y).expect("in domain");
            let mirrored = SIGNED_NEGATED.evaluate(x, y).expect("in domain");
            assert_eq!(mirrored, -value, "sign reflection at ({x}, {y})");
            if reference::mutant_evaluate_ties_toward_zero(&SIGNED, x, y) != Ok(value) {
                ties += 1;
            }
        }
    }
    assert!(ties > 0, "the signed fixture must contain at least one tie");
}

#[test]
fn ties_toward_zero_would_fail_on_named_points() {
    // Direct discrimination of the rounding mutation on hand-visible ties.
    assert_ne!(
        reference::mutant_evaluate_ties_toward_zero(&TIES, 1, 0),
        Ok(1)
    );
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&TIES, 1, 0),
        Ok(0)
    );
    assert_ne!(
        reference::mutant_evaluate_ties_toward_zero(&TIES, 1, 2),
        Ok(-1)
    );
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&TIES, 1, 2),
        Ok(0)
    );
    // GRID (7, 1) is 72.5 -> 73 accepted, 72 under the mutant.
    assert_eq!(GRID.evaluate(7, 1), Ok(73));
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&GRID, 7, 1),
        Ok(72)
    );
}

#[test]
fn ties_toward_zero_mutant_preserves_nearest_rounding_on_non_ties() {
    // At two thirds, both tie policies round to the nearest integer. The
    // mutant differs from the accepted policy only when the remainder is
    // exactly half the denominator.
    assert_eq!(reference::segment(2, 0, 3, 0, 1), 1);
    assert_eq!(reference::mutant_segment_ties_toward_zero(2, 0, 3, 0, 1), 1);
    assert_eq!(reference::segment(2, 0, 3, 0, -1), -1);
    assert_eq!(
        reference::mutant_segment_ties_toward_zero(2, 0, 3, 0, -1),
        -1
    );
}

#[test]
fn the_inset_domain_agrees_with_the_reference_exhaustively() {
    // 31 x 301 = 9_331 points under the default all-Error policy.
    let count = reference::assert_exhaustive_domain(&INSET, "INSET");
    assert_eq!(count, 31 * 301);
}
