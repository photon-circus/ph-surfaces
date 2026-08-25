//! Axis search, observed through the public evaluator and checked against the
//! reference's linear scan over representative nonuniform axes.
//!
//! Each lookup fixture's values are strictly convex in the knot index, so a
//! wrongly bracketed cell changes the interpolated value instead of hiding
//! behind collinear data. Every coordinate of every axis's declared range is
//! enumerated, including the 1024-knot axis, whose domain is `0..=4_085`.
//! Linear and Bucketed run the same axes as the default binary surface;
//! Uniform has its own even-spaced fixture.

use crate::fixtures::*;
use crate::reference;
use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, BucketedAxis, LinearAxis, UniformAxis, bucket_index,
};

fn check_located<const NX: usize, X, Y>(
    surface: &BilinearSurface<NX, 2, X, Y>,
    x_axis: &[u16; NX],
    name: &str,
) where
    X: AxisLookup<NX>,
    Y: AxisLookup<2>,
{
    assert!(
        x_axis.windows(2).all(|w| w[0] < w[1]),
        "{name}: fixture axis is not increasing"
    );

    let y_axis = &LOOKUP_Y;
    let values = surface.values();
    let policy = surface.policy();

    for x in x_axis[0]..=x_axis[NX - 1] {
        for y in [0u16, 1] {
            assert_eq!(
                surface.evaluate(x, y),
                reference::evaluate_tables(x_axis, y_axis, values, policy, x, y),
                "{name}: ({x}, {y})"
            );
        }
    }

    for (i, &knot) in x_axis.iter().enumerate() {
        assert_eq!(
            surface.evaluate(knot, 0),
            Ok(values[0][i]),
            "{name}: knot {i}"
        );
        if knot > x_axis[0] {
            assert_eq!(
                surface.evaluate(knot - 1, 0),
                reference::evaluate_tables(x_axis, y_axis, values, policy, knot - 1, 0),
                "{name}: one below knot {i}"
            );
        }
        if knot < x_axis[NX - 1] {
            assert_eq!(
                surface.evaluate(knot + 1, 0),
                reference::evaluate_tables(x_axis, y_axis, values, policy, knot + 1, 0),
                "{name}: one above knot {i}"
            );
        }
    }
}

fn check_axis<const NX: usize>(surface: &BilinearSurface<NX, 2>, name: &str) {
    check_located(surface, surface.x_axis(), name);
}

static LOOKUP_2_INDEX: [u16; 2] = bucket_index(&LOOKUP_2_X);
static LOOKUP_3_INDEX: [u16; 4] = bucket_index(&LOOKUP_3_X);
static LOOKUP_4_INDEX: [u16; 4] = bucket_index(&LOOKUP_4_X);
static LOOKUP_5_INDEX: [u16; 4] = bucket_index(&LOOKUP_5_X);
static LOOKUP_7_INDEX: [u16; 8] = bucket_index(&LOOKUP_7_X);
static LOOKUP_8_INDEX: [u16; 8] = bucket_index(&LOOKUP_8_X);
static LOOKUP_9_INDEX: [u16; 8] = bucket_index(&LOOKUP_9_X);
static LOOKUP_17_INDEX: [u16; 16] = bucket_index(&LOOKUP_17_X);
static LOOKUP_1024_INDEX: [u16; 32] = bucket_index(&LOOKUP_1024_X);

macro_rules! linear_and_bucketed {
    ($test:ident, $axis:expr, $values:expr, $index:expr, $n:literal, $b:literal, $name:expr) => {
        #[test]
        fn $test() {
            let linear = BilinearSurface::<$n, 2, LinearAxis<$n>, BinaryAxis<2>>::from_axes(
                LinearAxis::new($axis),
                BinaryAxis::new(&LOOKUP_Y),
                $values,
            );
            check_located(&linear, $axis, concat!($name, " Linear"));

            let bucketed = BilinearSurface::<$n, 2, BucketedAxis<$n, $b>, BinaryAxis<2>>::from_axes(
                BucketedAxis::new($axis, $index),
                BinaryAxis::new(&LOOKUP_Y),
                $values,
            );
            check_located(&bucketed, $axis, concat!($name, " Bucketed"));
        }
    };
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

linear_and_bucketed!(
    linear_and_bucketed_locate_a_two_knot_axis,
    &LOOKUP_2_X,
    &LOOKUP_2_V,
    &LOOKUP_2_INDEX,
    2,
    2,
    "LOOKUP_2"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_three_knot_axis,
    &LOOKUP_3_X,
    &LOOKUP_3_V,
    &LOOKUP_3_INDEX,
    3,
    4,
    "LOOKUP_3"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_four_knot_clustered_axis,
    &LOOKUP_4_X,
    &LOOKUP_4_V,
    &LOOKUP_4_INDEX,
    4,
    4,
    "LOOKUP_4"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_five_knot_axis,
    &LOOKUP_5_X,
    &LOOKUP_5_V,
    &LOOKUP_5_INDEX,
    5,
    4,
    "LOOKUP_5"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_seven_knot_axis,
    &LOOKUP_7_X,
    &LOOKUP_7_V,
    &LOOKUP_7_INDEX,
    7,
    8,
    "LOOKUP_7"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_an_eight_knot_axis,
    &LOOKUP_8_X,
    &LOOKUP_8_V,
    &LOOKUP_8_INDEX,
    8,
    8,
    "LOOKUP_8"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_nine_knot_axis,
    &LOOKUP_9_X,
    &LOOKUP_9_V,
    &LOOKUP_9_INDEX,
    9,
    8,
    "LOOKUP_9"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_seventeen_knot_axis,
    &LOOKUP_17_X,
    &LOOKUP_17_V,
    &LOOKUP_17_INDEX,
    17,
    16,
    "LOOKUP_17"
);
linear_and_bucketed!(
    linear_and_bucketed_locate_a_1024_knot_axis_exhaustively,
    &LOOKUP_1024_X,
    &LOOKUP_1024_V,
    &LOOKUP_1024_INDEX,
    1024,
    32,
    "LOOKUP_1024"
);

#[test]
fn a_uniform_even_spaced_axis_locates_against_the_linear_oracle() {
    let surface = BilinearSurface::<5, 2, UniformAxis<5, 0, 10>, BinaryAxis<2>>::from_axes(
        UniformAxis::new(),
        BinaryAxis::new(&LOOKUP_Y),
        &LOOKUP_UNIFORM_V,
    );
    check_located(&surface, &LOOKUP_UNIFORM_X, "LOOKUP_UNIFORM Uniform");
    check_axis(&LOOKUP_UNIFORM, "LOOKUP_UNIFORM Binary");
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

    let policy = LOOKUP_1024.policy();
    assert_ne!(
        Ok(right),
        reference::mutant_evaluate_off_by_one_cell(axis, &LOOKUP_Y, values, policy, x, 0)
    );
}

#[test]
fn uniform_origin_and_step_off_by_one_disagree_on_an_interior_point() {
    let policy = LOOKUP_UNIFORM.policy();
    assert_eq!(LOOKUP_UNIFORM.evaluate(15, 0), Ok(3));
    assert_ne!(
        Ok(3),
        reference::mutant_evaluate_uniform_origin_off_by_one(
            0,
            10,
            &LOOKUP_Y,
            &LOOKUP_UNIFORM_V,
            policy,
            15,
            0
        )
    );
    assert_ne!(
        Ok(3),
        reference::mutant_evaluate_uniform_step_off_by_one(
            0,
            10,
            &LOOKUP_Y,
            &LOOKUP_UNIFORM_V,
            policy,
            15,
            0
        )
    );
}

#[test]
fn bucketed_cluster_vs_tail_disagrees_on_a_tail_coordinate() {
    let policy = LOOKUP_4.policy();
    let accepted = LOOKUP_4.evaluate(100, 0);
    assert_ne!(
        accepted,
        reference::mutant_evaluate_bucketed_cluster_vs_tail(
            &LOOKUP_4_X,
            &LOOKUP_Y,
            &LOOKUP_4_V,
            policy,
            100,
            0
        )
    );
}
