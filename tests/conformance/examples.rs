//! Two unrelated device-neutral example maps. They demonstrate the shape and
//! rounding behaviour of the evaluator on realistic-looking tables; they make
//! no claim about any sensor, vendor, or total measurement accuracy.

use crate::fixtures::*;
use crate::reference;
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

// ---------------------------------------------------------------------------
// Mixed-sign nonuniform elevation / plane map.
// ---------------------------------------------------------------------------

#[test]
fn the_elevation_map_returns_every_stored_height_exactly() {
    reference::assert_every_knot_exact(&ELEVATION, "ELEVATION");
}

#[test]
fn the_elevation_map_agrees_with_the_reference_over_its_whole_domain() {
    // 181 x 151 = 27_331 points, exhaustive.
    let count = reference::assert_exhaustive_domain(&ELEVATION, "ELEVATION");
    assert_eq!(count, 181 * 151);
}

#[test]
fn the_elevation_map_evaluates_hand_computed_points() {
    // (10, 20): X cell [0, 25] at offset 10. Row y = 0: -120 + 85 * 10 / 25 =
    // -86 exactly. Row y = 40: -80 + 90 * 10 / 25 = -44. Y at 20 of 40:
    // (-86 + -44) / 2 = -65. Below datum, between two below-datum knots.
    assert_eq!(ELEVATION.evaluate(10, 20), Ok(-65));
    // (75, 100): X cell [60, 100] at offset 15 of 40. Row y = 90: 130 - 42 *
    // 15 / 40 = 114.25 -> 114. Row y = 150: 70 + 40 * 15 / 40 = 85. Y at 10 of
    // 60: 114 - 29 * 10 / 60 = 109.17 -> 109.
    assert_eq!(ELEVATION.evaluate(75, 100), Ok(109));
    // (140, 60): X midpoint of [100, 180]. Row y = 40: (60 - 20) / 2 = 20. Row
    // y = 90: (88 + 5) / 2 = 46.5 -> 47. Y at 20 of 50: 20 + 27 * 20 / 50 =
    // 30.8 -> 31.
    assert_eq!(ELEVATION.evaluate(140, 60), Ok(31));
    // The ridge line: the y = 90 row peaks at x = 60 and falls both ways, so
    // the value at the peak knot exceeds both neighbours at the same Y.
    let peak = ELEVATION.evaluate(60, 90).unwrap();
    assert!(peak > ELEVATION.evaluate(59, 90).unwrap());
    assert!(peak > ELEVATION.evaluate(61, 90).unwrap());
    // Zero crossings exist between the -15 and 55 knots on the y = 90 row.
    assert!(ELEVATION.evaluate(0, 90).unwrap() < 0);
    assert!(ELEVATION.evaluate(25, 90).unwrap() > 0);
}

#[test]
fn the_elevation_map_clamps_beyond_its_far_x_edge_and_rejects_the_rest() {
    // A realistic policy: positions past the last X column continue the edge
    // height, everything else is out of map. Written as a `static` the way a
    // firmware consumer would.
    static MAP: BilinearSurface<5, 4> =
        BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES)
            .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));
    assert_eq!(MAP.evaluate(500, 90), MAP.evaluate(180, 90));
    assert_eq!(MAP.evaluate(u16::MAX, 0), Ok(-60));
    assert_eq!(
        MAP.evaluate(500, 151),
        Err(SurfaceError::YAbove {
            coordinate: 151,
            bound: 150
        })
    );
    assert_eq!(
        MAP.evaluate(180, 151),
        Err(SurfaceError::YAbove {
            coordinate: 151,
            bound: 150
        })
    );
    // In domain the policy changes nothing.
    for y in [0u16, 40, 77, 150] {
        for x in [0u16, 30, 60, 179, 180] {
            assert_eq!(MAP.evaluate(x, y), ELEVATION.evaluate(x, y));
        }
    }
    // And the clamped edge never leaves the hull of the map.
    let (lo, hi) = reference::value_hull(&MAP);
    for y in 0..=150u16 {
        let value = MAP.evaluate(u16::MAX, y).expect("X clamps, Y in domain");
        assert!((lo..=hi).contains(&value));
        assert_eq!(Ok(value), reference::evaluate(&MAP, u16::MAX, y));
    }
}

// ---------------------------------------------------------------------------
// Asymmetric process-correction map.
// ---------------------------------------------------------------------------

#[test]
fn the_correction_map_returns_every_stored_correction_exactly() {
    reference::assert_every_knot_exact(&CORRECTION, "CORRECTION");
}

#[test]
fn the_correction_map_agrees_with_the_reference_over_its_whole_domain() {
    // 161 x 121 = 19_481 points, exhaustive.
    let count = reference::assert_exhaustive_domain(&CORRECTION, "CORRECTION");
    assert_eq!(count, 161 * 121);
}

#[test]
fn the_correction_map_evaluates_hand_computed_points_including_a_tie() {
    // (47, 5): X cell [40, 55] at offset 7 of 15. Row y = 0: 125 - 45 * 7 / 15
    // = 104 exactly. Row y = 10: 90 - 49 * 7 / 15 = 67.13 -> 67. Y at 5 of 10:
    // (104 + 67) / 2 = 85.5, an exact positive tie -> 86.
    assert_eq!(CORRECTION.evaluate(47, 5), Ok(86));
    assert_eq!(
        reference::mutant_evaluate_ties_toward_zero(&CORRECTION, 47, 5),
        Ok(85)
    );
    // (145, 100): X midpoint of [90, 200]. Row y = 70: (-150 - 260) / 2 =
    // -205. Row y = 120: (-199 - 333) / 2 = -266. Y at 30 of 50: -205 - 61 *
    // 30 / 50 = -241.6 -> -242.
    assert_eq!(CORRECTION.evaluate(145, 100), Ok(-242));
    // (60, 40): X cell [55, 90] at offset 5 of 35. Row y = 25: -7 - 54 * 5 / 35
    // = -14.71 -> -15. Row y = 70: -95 - 55 * 5 / 35 = -102.86 -> -103. Y at
    // 15 of 45: -15 - 88 * 15 / 45 = -44.33 -> -44.
    assert_eq!(CORRECTION.evaluate(60, 40), Ok(-44));
    // The correction changes sign along the y = 0 row between x = 55 and 90.
    assert!(CORRECTION.evaluate(55, 0).unwrap() > 0);
    assert!(CORRECTION.evaluate(90, 0).unwrap() < 0);
}

#[test]
fn the_correction_map_is_asymmetric_in_both_axes() {
    // Guard against a fixture that accidentally became symmetric: mirroring
    // either axis changes values, so the map really does exercise the
    // direction-dependent parts of the evaluator.
    assert_ne!(CORRECTION.evaluate(41, 5), CORRECTION.evaluate(199, 5));
    assert_ne!(CORRECTION.evaluate(70, 1), CORRECTION.evaluate(70, 119));
    // And row differences are not constant, so no plane hides inside it.
    let a = CORRECTION_VALUES[1][0] - CORRECTION_VALUES[0][0];
    let b = CORRECTION_VALUES[1][3] - CORRECTION_VALUES[0][3];
    assert_ne!(a, b);
}

#[test]
fn the_correction_map_holds_the_last_load_row_above_its_range() {
    // Loads beyond the table hold the last row; setpoints outside the table
    // are rejected on both sides, and loads below zero cannot occur on a u16
    // axis that starts at zero.
    static MAP: BilinearSurface<4, 5> =
        BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES)
            .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));
    for x in 40..=200u16 {
        assert_eq!(MAP.evaluate(x, 121), MAP.evaluate(x, 120), "x = {x}");
        assert_eq!(MAP.evaluate(x, u16::MAX), MAP.evaluate(x, 120), "x = {x}");
    }
    assert_eq!(
        MAP.evaluate(39, 500),
        Err(SurfaceError::XBelow {
            coordinate: 39,
            bound: 40
        })
    );
    assert_eq!(
        MAP.evaluate(201, 500),
        Err(SurfaceError::XAbove {
            coordinate: 201,
            bound: 200
        })
    );
}
