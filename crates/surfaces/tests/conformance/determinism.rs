//! Stateless determinism: repeated and interleaved evaluation, fresh versus
//! warmed handles, and copies of a handle all answer identically, and no
//! evaluation changes anything observable through the public accessors.

use crate::fixtures::*;
use ph_surfaces::{BilinearSurface, BoundaryPolicy};

#[test]
fn repeated_evaluation_returns_identical_results() {
    let probes = [
        (60u16, 6u16),
        (0, 0),
        (110, 8),
        (7, 1),
        (7, 5),
        (111, 0),
        (0, 9),
    ];
    let first: Vec<_> = probes.iter().map(|&(x, y)| GRID.evaluate(x, y)).collect();
    for _ in 0..256 {
        for (i, &(x, y)) in probes.iter().enumerate() {
            assert_eq!(GRID.evaluate(x, y), first[i], "({x}, {y})");
        }
    }
}

#[test]
fn interleaving_other_coordinates_does_not_change_a_result() {
    // There is no cached cell to invalidate: sweeping every other point of
    // the domain (and out of it) between two evaluations of the same point
    // leaves the answer unchanged.
    let before = ELEVATION.evaluate(77, 33);
    for y in 0..=150u16 {
        for x in 0..=180u16 {
            let _ = ELEVATION.evaluate(x, y);
        }
    }
    let _ = ELEVATION.evaluate(u16::MAX, u16::MAX);
    let _ = ELEVATION.evaluate(0, u16::MAX);
    assert_eq!(ELEVATION.evaluate(77, 33), before);
}

#[test]
fn a_fresh_handle_answers_exactly_as_a_warmed_one() {
    static FRESH: BilinearSurface<3, 3> = inset(BoundaryPolicy::new());
    for y in 100..=400u16 {
        for x in 10..=40u16 {
            let _ = INSET.evaluate(x, y);
        }
    }
    for y in [100u16, 150, 250, 400] {
        for x in [10u16, 15, 25, 40] {
            assert_eq!(FRESH.evaluate(x, y), INSET.evaluate(x, y));
        }
    }
    assert_eq!(FRESH, INSET);
}

#[test]
fn a_copy_of_the_handle_evaluates_identically_and_the_original_is_unchanged() {
    let snapshot = CORRECTION;
    let copy: BilinearSurface<4, 5> = CORRECTION;
    for y in [0u16, 5, 25, 71, 120] {
        for x in [40u16, 41, 55, 89, 200] {
            assert_eq!(copy.evaluate(x, y), CORRECTION.evaluate(x, y));
        }
    }
    assert_eq!(CORRECTION, snapshot);
    assert_eq!(copy, snapshot);
    assert_eq!(CORRECTION.values(), snapshot.values());
    assert_eq!(CORRECTION.policy(), snapshot.policy());
    assert_eq!(CORRECTION.x_axis(), snapshot.x_axis());
    assert_eq!(CORRECTION.y_axis(), snapshot.y_axis());
}

#[test]
fn errors_are_as_repeatable_as_values() {
    let first = INSET.evaluate(0, u16::MAX);
    assert!(first.is_err());
    for _ in 0..64 {
        assert_eq!(INSET.evaluate(0, u16::MAX), first);
        let _ = INSET.evaluate(25, 250);
    }
}
