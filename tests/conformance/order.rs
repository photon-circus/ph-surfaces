//! The X-then-Y composition order: the locked fixture from issue #6, and
//! evidence that the suite distinguishes it from Y-then-X.

use crate::fixtures::*;
use crate::reference;
use ph_surfaces::BilinearSurface;

#[test]
fn the_locked_order_fixture_returns_the_x_then_y_value() {
    // Axes [0, 2], rows [[0, 0], [1, 3]], input (1, 1): X on the lower row is
    // 0, X on the upper row is (1 + 3) / 2 = 2, Y between them is 1.
    assert_eq!(ORDER.evaluate(1, 1), Ok(1));
    assert_eq!(reference::evaluate(&ORDER, 1, 1), Ok(1));
}

#[test]
fn the_locked_order_fixture_rejects_y_then_x() {
    // Y-then-X: columns are (0 + 1) / 2 = 0.5 -> 1 and (0 + 3) / 2 = 1.5 -> 2,
    // each rounded before X sees them, and X between those is 1.5 -> 2. So a
    // reversed implementation returns 2 here and this fixture necessarily
    // fails it.
    assert_eq!(reference::mutant_evaluate_y_then_x(&ORDER, 1, 1), Ok(2));
    assert_ne!(
        ORDER.evaluate(1, 1),
        reference::mutant_evaluate_y_then_x(&ORDER, 1, 1)
    );
}

#[test]
fn the_locked_order_fixture_agrees_with_the_reference_everywhere() {
    // 3 x 3 = 9 points, exhaustive; (1, 1) is the only point where the two
    // orders differ, which is why the fixture names it.
    reference::assert_exhaustive_domain(&ORDER, "ORDER");
    let mut differing = Vec::new();
    for y in 0..=2u16 {
        for x in 0..=2u16 {
            if ORDER.evaluate(x, y) != reference::mutant_evaluate_y_then_x(&ORDER, x, y) {
                differing.push((x, y));
            }
        }
    }
    assert_eq!(differing, [(1, 1)]);
}

/// Counts the points of a small domain where the two orders disagree.
fn order_sensitive_points<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
) -> usize {
    let mut count = 0;
    for y in surface.y_min()..=surface.y_max() {
        for x in surface.x_min()..=surface.x_max() {
            if surface.evaluate(x, y) != reference::mutant_evaluate_y_then_x(surface, x, y) {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn every_rounding_fixture_contains_points_that_discriminate_the_order() {
    // The suite's numerical fixtures are not order-blind: each of these has at
    // least one point where Y-then-X returns something else, so an order swap
    // in the implementation cannot pass their exhaustive reference sweeps.
    assert!(order_sensitive_points(&GRID) > 0, "GRID");
    assert!(order_sensitive_points(&SIGNED) > 0, "SIGNED");
    assert!(order_sensitive_points(&COLUMNS) > 0, "COLUMNS");
    assert!(order_sensitive_points(&ELEVATION) > 0, "ELEVATION");
    assert!(order_sensitive_points(&CORRECTION) > 0, "CORRECTION");
}

#[test]
fn planar_and_row_shifted_fixtures_are_order_blind_and_the_suite_knows_it() {
    // On exact planar data both orders agree everywhere. INSET's rows differ
    // by a constant, which also makes both orders agree. Those fixtures serve
    // closed-form values and boundary evidence, not order evidence, and this
    // test records that so nobody mistakes them for it.
    assert_eq!(order_sensitive_points(&PLANE), 0);
    assert_eq!(order_sensitive_points(&INSET), 0);
}
