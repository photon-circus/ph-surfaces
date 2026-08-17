//! The binary axis search, observed through the public evaluator and checked
//! against the reference's linear scan over representative nonuniform axes.
//!
//! Each lookup fixture's values are strictly convex in the knot index, so a
//! wrongly bracketed cell changes the interpolated value instead of hiding
//! behind collinear data. Every coordinate of every axis's declared range is
//! enumerated, including the 1024-knot axis, whose domain is `0..=4_085`.

use crate::fixtures::*;
use crate::reference;
use ph_surfaces::BilinearSurface;

fn check_axis<const NX: usize>(surface: &BilinearSurface<NX, 2>, name: &str) {
    let axis = surface.x_axis();
    assert!(
        axis.windows(2).all(|w| w[0] < w[1]),
        "{name}: fixture axis is not increasing"
    );

    // Every coordinate of the domain, on both Y rows and midway between them.
    for x in surface.x_min()..=surface.x_max() {
        for y in [0u16, 1] {
            assert_eq!(
                surface.evaluate(x, y),
                reference::evaluate(surface, x, y),
                "{name}: ({x}, {y})"
            );
        }
    }

    // Exact knots and their immediate neighbours, named explicitly so a
    // one-off cell boundary shows up as such.
    for (i, &knot) in axis.iter().enumerate() {
        assert_eq!(
            surface.evaluate(knot, 0),
            Ok(surface.values()[0][i]),
            "{name}: knot {i}"
        );
        if knot > surface.x_min() {
            assert_eq!(
                surface.evaluate(knot - 1, 0),
                reference::evaluate(surface, knot - 1, 0),
                "{name}: one below knot {i}"
            );
        }
        if knot < surface.x_max() {
            assert_eq!(
                surface.evaluate(knot + 1, 0),
                reference::evaluate(surface, knot + 1, 0),
                "{name}: one above knot {i}"
            );
        }
    }
}

#[test]
fn a_two_knot_axis_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_2, "LOOKUP_2");
}

#[test]
fn a_three_knot_axis_with_a_unit_cell_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_3, "LOOKUP_3");
}

#[test]
fn a_four_knot_axis_with_clustered_knots_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_4, "LOOKUP_4");
}

#[test]
fn a_five_knot_axis_with_two_clusters_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_5, "LOOKUP_5");
}

#[test]
fn a_seven_knot_geometric_axis_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_7, "LOOKUP_7");
}

#[test]
fn an_eight_knot_power_of_two_axis_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_8, "LOOKUP_8");
}

#[test]
fn a_nine_knot_axis_with_a_long_tail_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_9, "LOOKUP_9");
}

#[test]
fn a_seventeen_knot_axis_with_a_far_last_knot_locates_against_the_linear_oracle() {
    check_axis(&LOOKUP_17, "LOOKUP_17");
}

#[test]
fn a_1024_knot_nonuniform_axis_locates_against_the_linear_oracle_exhaustively() {
    assert_eq!(LOOKUP_1024.x_max(), 4_085);
    check_axis(&LOOKUP_1024, "LOOKUP_1024");
}

#[test]
fn a_wrongly_bracketed_cell_would_change_the_value() {
    // Evidence that the convex fixtures are not lookup-blind: on the 1024-knot
    // axis, evaluating an interior coordinate with the reference forced onto
    // the *neighbouring* cell gives a different value. Done through the public
    // grid so no private index type is involved.
    let axis = LOOKUP_1024.x_axis();
    let values = LOOKUP_1024.values();
    let x = axis[500] + 1;
    let right = reference::segment(x, axis[500], axis[501], values[0][500], values[0][501]);
    assert_eq!(LOOKUP_1024.evaluate(x, 0), Ok(right));
    // The neighbouring cell's segment extended to x is a different number.
    let (num, span) = (
        i64::from(values[0][499]) * i64::from(axis[500] - axis[499])
            + i64::from(values[0][500] - values[0][499]) * i64::from(x - axis[499]),
        i64::from(axis[500] - axis[499]),
    );
    let wrong = (num + span / 2) / span;
    assert_ne!(i64::from(right), wrong);
}
