//! Cross-strategy black-box evidence against the independent linear `i128`
//! oracle.
//!
//! Every pairing is compared to `reference::evaluate_tables` over the fixture
//! knot arrays. Production locators are never the expected-value source, and
//! the crate's own `src/axis/` unit tests are not repeated here.
//!
//! Applicable pairings:
//! - all 16 on even-spaced fixtures (`ORDER`, `PLANE`, `TIES`, `SHAPES`,
//!   `EXTREME`);
//! - the stored-knot 9-way (Linear/Binary/Bucketed) on irregular X (`GRID`,
//!   `SIGNED`, `EDGES`, `ELEVATION`);
//! - mixed Uniform+stored where exactly one axis is uniform (`GRID` Y,
//!   `EDGES` Y). `INSET` is irregular on both axes, so Uniform is skipped
//!   there and a stored mixed pairing carries the sixteen-policy sweep.

use crate::fixtures::*;
use crate::reference::{self, policy_from_bits};
use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, Boundary, BoundaryPolicy, BucketedAxis, LinearAxis,
    SurfaceError, UniformAxis, bucket_index, max_local_comparisons,
};

fn agrees<const NX: usize, const NY: usize, X, Y>(
    surface: &BilinearSurface<NX, NY, X, Y>,
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    x: u16,
    y: u16,
    name: &str,
) where
    X: AxisLookup<NX>,
    Y: AxisLookup<NY>,
{
    assert_eq!(
        surface.evaluate(x, y),
        reference::evaluate_tables(x_axis, y_axis, surface.values(), surface.policy(), x, y),
        "{name} at ({x}, {y})"
    );
}

fn knots_exact<const NX: usize, const NY: usize, X, Y>(
    surface: &BilinearSurface<NX, NY, X, Y>,
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    name: &str,
) where
    X: AxisLookup<NX>,
    Y: AxisLookup<NY>,
{
    reference::assert_every_knot_exact_tables(
        |x, y| surface.evaluate(x, y),
        x_axis,
        y_axis,
        surface.values(),
        name,
    );
}

fn exhaustive<const NX: usize, const NY: usize, X, Y>(
    surface: &BilinearSurface<NX, NY, X, Y>,
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    name: &str,
) -> u64
where
    X: AxisLookup<NX>,
    Y: AxisLookup<NY>,
{
    reference::assert_exhaustive_domain_tables(
        |x, y| surface.evaluate(x, y),
        x_axis,
        y_axis,
        surface.values(),
        surface.policy(),
        name,
    )
}

macro_rules! each_of_16 {
    (
        nx = $nx:literal, ny = $ny:literal,
        x = $x:expr, y = $y:expr,
        xi = $xi:expr, yi = $yi:expr,
        xb = $xb:literal, yb = $yb:literal,
        ux = $ux:ty, uy = $uy:ty,
        values = $values:expr,
        policy = $policy:expr,
        |$s:ident, $label:ident| $body:block
    ) => {{
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, LinearAxis<$ny>>::from_axes(
                LinearAxis::new($x),
                LinearAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Linear";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, BinaryAxis<$ny>>::from_axes(
                LinearAxis::new($x),
                BinaryAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Binary";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, $uy>::from_axes(
                LinearAxis::new($x),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Uniform";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, LinearAxis<$nx>, BucketedAxis<$ny, $yb>>::from_axes(
                    LinearAxis::new($x),
                    BucketedAxis::new($y, $yi),
                    $values,
                )
                .with_policy($policy);
            let $label = "Linear×Bucketed";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, LinearAxis<$ny>>::from_axes(
                BinaryAxis::new($x),
                LinearAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Linear";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, BinaryAxis<$ny>>::from_axes(
                BinaryAxis::new($x),
                BinaryAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Binary";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, $uy>::from_axes(
                BinaryAxis::new($x),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Uniform";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, BucketedAxis<$ny, $yb>>::from_axes(
                    BinaryAxis::new($x),
                    BucketedAxis::new($y, $yi),
                    $values,
                )
                .with_policy($policy);
            let $label = "Binary×Bucketed";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, $ux, LinearAxis<$ny>>::from_axes(
                UniformAxis::new(),
                LinearAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Uniform×Linear";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, $ux, BinaryAxis<$ny>>::from_axes(
                UniformAxis::new(),
                BinaryAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Uniform×Binary";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, $ux, $uy>::from_axes(
                UniformAxis::new(),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Uniform×Uniform";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, $ux, BucketedAxis<$ny, $yb>>::from_axes(
                UniformAxis::new(),
                BucketedAxis::new($y, $yi),
                $values,
            )
            .with_policy($policy);
            let $label = "Uniform×Bucketed";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, LinearAxis<$ny>>::from_axes(
                    BucketedAxis::new($x, $xi),
                    LinearAxis::new($y),
                    $values,
                )
                .with_policy($policy);
            let $label = "Bucketed×Linear";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, BinaryAxis<$ny>>::from_axes(
                    BucketedAxis::new($x, $xi),
                    BinaryAxis::new($y),
                    $values,
                )
                .with_policy($policy);
            let $label = "Bucketed×Binary";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, $uy>::from_axes(
                BucketedAxis::new($x, $xi),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Bucketed×Uniform";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, BucketedAxis<$ny, $yb>>::from_axes(
                BucketedAxis::new($x, $xi),
                BucketedAxis::new($y, $yi),
                $values,
            )
            .with_policy($policy);
            let $label = "Bucketed×Bucketed";
            $body
        }
    }};
}

macro_rules! each_stored_9 {
    (
        nx = $nx:literal, ny = $ny:literal,
        x = $x:expr, y = $y:expr,
        xi = $xi:expr, yi = $yi:expr,
        xb = $xb:literal, yb = $yb:literal,
        values = $values:expr,
        policy = $policy:expr,
        |$s:ident, $label:ident| $body:block
    ) => {{
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, LinearAxis<$ny>>::from_axes(
                LinearAxis::new($x),
                LinearAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Linear";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, BinaryAxis<$ny>>::from_axes(
                LinearAxis::new($x),
                BinaryAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Binary";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, LinearAxis<$nx>, BucketedAxis<$ny, $yb>>::from_axes(
                    LinearAxis::new($x),
                    BucketedAxis::new($y, $yi),
                    $values,
                )
                .with_policy($policy);
            let $label = "Linear×Bucketed";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, LinearAxis<$ny>>::from_axes(
                BinaryAxis::new($x),
                LinearAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Linear";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, BinaryAxis<$ny>>::from_axes(
                BinaryAxis::new($x),
                BinaryAxis::new($y),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Binary";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, BucketedAxis<$ny, $yb>>::from_axes(
                    BinaryAxis::new($x),
                    BucketedAxis::new($y, $yi),
                    $values,
                )
                .with_policy($policy);
            let $label = "Binary×Bucketed";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, LinearAxis<$ny>>::from_axes(
                    BucketedAxis::new($x, $xi),
                    LinearAxis::new($y),
                    $values,
                )
                .with_policy($policy);
            let $label = "Bucketed×Linear";
            $body
        }
        {
            let $s =
                BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, BinaryAxis<$ny>>::from_axes(
                    BucketedAxis::new($x, $xi),
                    BinaryAxis::new($y),
                    $values,
                )
                .with_policy($policy);
            let $label = "Bucketed×Binary";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, BucketedAxis<$ny, $yb>>::from_axes(
                BucketedAxis::new($x, $xi),
                BucketedAxis::new($y, $yi),
                $values,
            )
            .with_policy($policy);
            let $label = "Bucketed×Bucketed";
            $body
        }
    }};
}

macro_rules! each_stored_x_uniform_y {
    (
        nx = $nx:literal, ny = $ny:literal,
        x = $x:expr,
        xi = $xi:expr,
        xb = $xb:literal,
        uy = $uy:ty,
        values = $values:expr,
        policy = $policy:expr,
        |$s:ident, $label:ident| $body:block
    ) => {{
        {
            let $s = BilinearSurface::<$nx, $ny, LinearAxis<$nx>, $uy>::from_axes(
                LinearAxis::new($x),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Linear×Uniform";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BinaryAxis<$nx>, $uy>::from_axes(
                BinaryAxis::new($x),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Binary×Uniform";
            $body
        }
        {
            let $s = BilinearSurface::<$nx, $ny, BucketedAxis<$nx, $xb>, $uy>::from_axes(
                BucketedAxis::new($x, $xi),
                UniformAxis::new(),
                $values,
            )
            .with_policy($policy);
            let $label = "Bucketed×Uniform";
            $body
        }
    }};
}

static ORDER_INDEX: [u16; 2] = bucket_index(&ORDER_AXIS);
static PLANE_X_INDEX: [u16; 2] = bucket_index(&PLANE_X);
static PLANE_Y_INDEX: [u16; 2] = bucket_index(&PLANE_Y);
static TIE_INDEX: [u16; 2] = bucket_index(&TIE_AXIS);
static SHAPE_X_INDEX: [u16; 4] = bucket_index(&SHAPE_X);
static SHAPE_Y_INDEX: [u16; 4] = bucket_index(&SHAPE_Y);
static FULL_INDEX: [u16; 2] = bucket_index(&FULL_AXIS);
static GRID_X_INDEX: [u16; 4] = bucket_index(&GRID_X);
static GRID_Y_INDEX: [u16; 4] = bucket_index(&GRID_Y);
static SIGN_X_INDEX: [u16; 4] = bucket_index(&SIGN_X);
static SIGN_Y_INDEX: [u16; 4] = bucket_index(&SIGN_Y);
static EDGE_X_INDEX: [u16; 8] = bucket_index(&EDGE_X);
static EDGE_Y_INDEX: [u16; 4] = bucket_index(&EDGE_Y);
static ELEVATION_X_INDEX: [u16; 8] = bucket_index(&ELEVATION_X);
static ELEVATION_Y_INDEX: [u16; 4] = bucket_index(&ELEVATION_Y);
static INSET_X_INDEX: [u16; 4] = bucket_index(&INSET_X);
static INSET_Y_INDEX: [u16; 4] = bucket_index(&INSET_Y);

#[test]
fn every_pairing_recovers_stored_knots_on_uniform_fixtures() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 2, ny = 2,
        x = &ORDER_AXIS, y = &ORDER_AXIS,
        xi = &ORDER_INDEX, yi = &ORDER_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 2>, uy = UniformAxis<2, 0, 2>,
        values = &ORDER_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &ORDER_AXIS, &ORDER_AXIS, label); }
    );
    each_of_16!(
        nx = 2, ny = 2,
        x = &PLANE_X, y = &PLANE_Y,
        xi = &PLANE_X_INDEX, yi = &PLANE_Y_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 10>, uy = UniformAxis<2, 0, 10>,
        values = &PLANE_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &PLANE_X, &PLANE_Y, label); }
    );
    each_of_16!(
        nx = 2, ny = 2,
        x = &TIE_AXIS, y = &TIE_AXIS,
        xi = &TIE_INDEX, yi = &TIE_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 2>, uy = UniformAxis<2, 0, 2>,
        values = &TIE_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &TIE_AXIS, &TIE_AXIS, label); }
    );
    each_of_16!(
        nx = 3, ny = 4,
        x = &SHAPE_X, y = &SHAPE_Y,
        xi = &SHAPE_X_INDEX, yi = &SHAPE_Y_INDEX,
        xb = 4, yb = 4,
        ux = UniformAxis<3, 0, 100>, uy = UniformAxis<4, 0, 10>,
        values = &SHAPE_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &SHAPE_X, &SHAPE_Y, label); }
    );
    each_of_16!(
        nx = 2, ny = 2,
        x = &FULL_AXIS, y = &FULL_AXIS,
        xi = &FULL_INDEX, yi = &FULL_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 65535>, uy = UniformAxis<2, 0, 65535>,
        values = &EXTREME_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &FULL_AXIS, &FULL_AXIS, label); }
    );
}

#[test]
fn stored_knot_pairings_recover_knots_on_irregular_fixtures() {
    let policy = BoundaryPolicy::new();
    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &GRID_X,
        y = &GRID_Y,
        xi = &GRID_X_INDEX,
        yi = &GRID_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &GRID_VALUES,
        policy = policy,
        |surface, label| {
            knots_exact(&surface, &GRID_X, &GRID_Y, label);
        }
    );
    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &SIGN_X,
        y = &SIGN_Y,
        xi = &SIGN_X_INDEX,
        yi = &SIGN_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &SIGN_VALUES,
        policy = policy,
        |surface, label| {
            knots_exact(&surface, &SIGN_X, &SIGN_Y, label);
        }
    );
    each_stored_9!(
        nx = 5,
        ny = 3,
        x = &EDGE_X,
        y = &EDGE_Y,
        xi = &EDGE_X_INDEX,
        yi = &EDGE_Y_INDEX,
        xb = 8,
        yb = 4,
        values = &EDGE_VALUES,
        policy = policy,
        |surface, label| {
            knots_exact(&surface, &EDGE_X, &EDGE_Y, label);
        }
    );
    each_stored_9!(
        nx = 5,
        ny = 4,
        x = &ELEVATION_X,
        y = &ELEVATION_Y,
        xi = &ELEVATION_X_INDEX,
        yi = &ELEVATION_Y_INDEX,
        xb = 8,
        yb = 4,
        values = &ELEVATION_VALUES,
        policy = policy,
        |surface, label| {
            knots_exact(&surface, &ELEVATION_X, &ELEVATION_Y, label);
        }
    );
}

#[test]
fn mixed_uniform_y_recovers_knots_where_y_is_evenly_spaced() {
    let policy = BoundaryPolicy::new();
    each_stored_x_uniform_y!(
        nx = 3, ny = 3,
        x = &GRID_X, xi = &GRID_X_INDEX, xb = 4,
        uy = UniformAxis<3, 0, 4>,
        values = &GRID_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &GRID_X, &GRID_Y, label); }
    );
    each_stored_x_uniform_y!(
        nx = 5, ny = 3,
        x = &EDGE_X, xi = &EDGE_X_INDEX, xb = 8,
        uy = UniformAxis<3, 0, 2>,
        values = &EDGE_VALUES, policy = policy,
        |surface, label| { knots_exact(&surface, &EDGE_X, &EDGE_Y, label); }
    );
}

#[test]
fn interpolation_agrees_with_the_linear_oracle_on_named_points() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 2, ny = 2,
        x = &PLANE_X, y = &PLANE_Y,
        xi = &PLANE_X_INDEX, yi = &PLANE_Y_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 10>, uy = UniformAxis<2, 0, 10>,
        values = &PLANE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(4, 3), Ok(340), "{label}");
            agrees(&surface, &PLANE_X, &PLANE_Y, 4, 3, label);
        }
    );
    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &GRID_X,
        y = &GRID_Y,
        xi = &GRID_X_INDEX,
        yi = &GRID_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &GRID_VALUES,
        policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(5, 2), Ok(55), "{label}");
            assert_eq!(surface.evaluate(60, 6), Ok(185), "{label}");
            assert_eq!(surface.evaluate(7, 1), Ok(73), "{label}");
            agrees(&surface, &GRID_X, &GRID_Y, 7, 5, label);
        }
    );
    each_stored_x_uniform_y!(
        nx = 3, ny = 3,
        x = &GRID_X, xi = &GRID_X_INDEX, xb = 4,
        uy = UniformAxis<3, 0, 4>,
        values = &GRID_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(5, 2), Ok(55), "{label}");
            agrees(&surface, &GRID_X, &GRID_Y, 60, 6, label);
        }
    );
    each_of_16!(
        nx = 3, ny = 4,
        x = &SHAPE_X, y = &SHAPE_Y,
        xi = &SHAPE_X_INDEX, yi = &SHAPE_Y_INDEX,
        xb = 4, yb = 4,
        ux = UniformAxis<3, 0, 100>, uy = UniformAxis<4, 0, 10>,
        values = &SHAPE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(50, 0), Ok(15), "{label}");
            assert_eq!(surface.evaluate(50, 10), Ok(-15), "{label}");
            agrees(&surface, &SHAPE_X, &SHAPE_Y, 150, 30, label);
        }
    );
    each_stored_9!(
        nx = 5,
        ny = 4,
        x = &ELEVATION_X,
        y = &ELEVATION_Y,
        xi = &ELEVATION_X_INDEX,
        yi = &ELEVATION_Y_INDEX,
        xb = 8,
        yb = 4,
        values = &ELEVATION_VALUES,
        policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(60, 90), Ok(130), "{label}");
            assert_eq!(surface.evaluate(10, 20), Ok(-65), "{label}");
            agrees(&surface, &ELEVATION_X, &ELEVATION_Y, 75, 100, label);
        }
    );
}

#[test]
fn ties_away_and_the_toward_zero_mutant_disagree_on_named_points() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 2, ny = 2,
        x = &TIE_AXIS, y = &TIE_AXIS,
        xi = &TIE_INDEX, yi = &TIE_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 2>, uy = UniformAxis<2, 0, 2>,
        values = &TIE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(1, 0), Ok(1), "{label}");
            assert_eq!(surface.evaluate(1, 2), Ok(-1), "{label}");
            assert_eq!(
                reference::mutant_evaluate_ties_toward_zero_tables(
                    &TIE_AXIS, &TIE_AXIS, &TIE_VALUES, policy, 1, 0
                ),
                Ok(0),
                "{label}"
            );
            assert_ne!(
                surface.evaluate(1, 0),
                reference::mutant_evaluate_ties_toward_zero_tables(
                    &TIE_AXIS, &TIE_AXIS, &TIE_VALUES, policy, 1, 0
                ),
                "{label}"
            );
            agrees(&surface, &TIE_AXIS, &TIE_AXIS, 1, 1, label);
        }
    );
    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &GRID_X,
        y = &GRID_Y,
        xi = &GRID_X_INDEX,
        yi = &GRID_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &GRID_VALUES,
        policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(7, 1), Ok(73), "{label}");
            assert_eq!(
                reference::mutant_evaluate_ties_toward_zero_tables(
                    &GRID_X,
                    &GRID_Y,
                    &GRID_VALUES,
                    policy,
                    7,
                    1
                ),
                Ok(72),
                "{label}"
            );
        }
    );
}

#[test]
fn order_fixture_is_one_under_every_pairing_and_two_under_the_y_then_x_mutant() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 2, ny = 2,
        x = &ORDER_AXIS, y = &ORDER_AXIS,
        xi = &ORDER_INDEX, yi = &ORDER_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 2>, uy = UniformAxis<2, 0, 2>,
        values = &ORDER_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(surface.evaluate(1, 1), Ok(1), "{label}");
            assert_eq!(
                reference::mutant_evaluate_y_then_x_tables(
                    &ORDER_AXIS, &ORDER_AXIS, &ORDER_VALUES, policy, 1, 1
                ),
                Ok(2),
                "{label}"
            );
            assert_ne!(surface.evaluate(1, 1), Ok(2), "{label}");
            let count = exhaustive(&surface, &ORDER_AXIS, &ORDER_AXIS, label);
            assert_eq!(count, 9, "{label}");
        }
    );
}

#[test]
fn plane_and_ties_domains_are_exhaustive_under_every_pairing() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 2, ny = 2,
        x = &PLANE_X, y = &PLANE_Y,
        xi = &PLANE_X_INDEX, yi = &PLANE_Y_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 10>, uy = UniformAxis<2, 0, 10>,
        values = &PLANE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(exhaustive(&surface, &PLANE_X, &PLANE_Y, label), 121);
        }
    );
    each_of_16!(
        nx = 2, ny = 2,
        x = &TIE_AXIS, y = &TIE_AXIS,
        xi = &TIE_INDEX, yi = &TIE_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 2>, uy = UniformAxis<2, 0, 2>,
        values = &TIE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(exhaustive(&surface, &TIE_AXIS, &TIE_AXIS, label), 9);
        }
    );
}

#[test]
fn shapes_domain_is_exhaustive_under_every_pairing() {
    let policy = BoundaryPolicy::new();
    each_of_16!(
        nx = 3, ny = 4,
        x = &SHAPE_X, y = &SHAPE_Y,
        xi = &SHAPE_X_INDEX, yi = &SHAPE_Y_INDEX,
        xb = 4, yb = 4,
        ux = UniformAxis<3, 0, 100>, uy = UniformAxis<4, 0, 10>,
        values = &SHAPE_VALUES, policy = policy,
        |surface, label| {
            assert_eq!(exhaustive(&surface, &SHAPE_X, &SHAPE_Y, label), 201 * 31);
        }
    );
}

#[test]
fn inset_error_clamp_precedence_and_clamp_then_y_error_hold_on_stored_pairings() {
    let all_error = BoundaryPolicy::new();
    let all_clamp = policy_from_bits(0b1111);
    let clamp_x = BoundaryPolicy::new()
        .with_x_below(Boundary::Clamp)
        .with_x_above(Boundary::Clamp);

    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &INSET_X,
        y = &INSET_Y,
        xi = &INSET_X_INDEX,
        yi = &INSET_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &INSET_VALUES,
        policy = all_error,
        |surface, label| {
            assert_eq!(
                surface.evaluate(9, 250),
                Err(SurfaceError::XBelow {
                    coordinate: 9,
                    bound: 10
                }),
                "{label}"
            );
            assert_eq!(
                surface.evaluate(9, 99),
                Err(SurfaceError::XBelow {
                    coordinate: 9,
                    bound: 10
                }),
                "{label} X-before-Y"
            );
            assert_ne!(
                surface.evaluate(9, 99),
                reference::mutant_evaluate_y_precedence_tables(
                    &INSET_X,
                    &INSET_Y,
                    &INSET_VALUES,
                    all_error,
                    9,
                    99
                ),
                "{label}"
            );
            agrees(&surface, &INSET_X, &INSET_Y, 20, 200, label);
        }
    );

    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &INSET_X,
        y = &INSET_Y,
        xi = &INSET_X_INDEX,
        yi = &INSET_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &INSET_VALUES,
        policy = all_clamp,
        |surface, label| {
            assert_eq!(
                surface.evaluate(0, 250),
                surface.evaluate(10, 250),
                "{label}"
            );
            assert_eq!(
                surface.evaluate(0, 250),
                reference::evaluate_tables(&INSET_X, &INSET_Y, &INSET_VALUES, all_clamp, 0, 250),
                "{label}"
            );
            let (lo, hi) = reference::value_hull_tables(&INSET_VALUES);
            let value = surface
                .evaluate(0, 0)
                .unwrap_or_else(|_| panic!("{label} clamps"));
            assert!((lo..=hi).contains(&value), "{label} extrapolated");
        }
    );

    each_stored_9!(
        nx = 3,
        ny = 3,
        x = &INSET_X,
        y = &INSET_Y,
        xi = &INSET_X_INDEX,
        yi = &INSET_Y_INDEX,
        xb = 4,
        yb = 4,
        values = &INSET_VALUES,
        policy = clamp_x,
        |surface, label| {
            assert_eq!(
                surface.evaluate(9, 99),
                Err(SurfaceError::YBelow {
                    coordinate: 99,
                    bound: 100
                }),
                "{label} clamp-then-Y-error"
            );
            agrees(&surface, &INSET_X, &INSET_Y, 9, 250, label);
        }
    );
}

#[test]
fn all_sixteen_policies_on_a_mixed_inset_pairing() {
    const X_PROBES: [u16; 11] = [0, 1, 9, 10, 11, 15, 20, 39, 40, 41, u16::MAX];
    const Y_PROBES: [u16; 11] = [0, 1, 99, 100, 101, 150, 200, 399, 400, 401, u16::MAX];

    for bits in 0..16 {
        let policy = policy_from_bits(bits);
        let surface = BilinearSurface::<3, 3, LinearAxis<3>, BucketedAxis<3, 4>>::from_axes(
            LinearAxis::new(&INSET_X),
            BucketedAxis::new(&INSET_Y, &INSET_Y_INDEX),
            &INSET_VALUES,
        )
        .with_policy(policy);
        for &y in &Y_PROBES {
            for &x in &X_PROBES {
                agrees(
                    &surface,
                    &INSET_X,
                    &INSET_Y,
                    x,
                    y,
                    &format!("Linear×Bucketed bits {bits:04b}"),
                );
            }
        }
    }
}

#[test]
fn sampled_extreme_stays_inside_the_hull_under_every_pairing() {
    let policy = BoundaryPolicy::new();
    let xs = reference::sample_axis(0, u16::MAX, 64);
    let ys = reference::sample_axis(0, u16::MAX, 64);
    let (lo, hi) = reference::value_hull_tables(&EXTREME_VALUES);

    each_of_16!(
        nx = 2, ny = 2,
        x = &FULL_AXIS, y = &FULL_AXIS,
        xi = &FULL_INDEX, yi = &FULL_INDEX,
        xb = 2, yb = 2,
        ux = UniformAxis<2, 0, 65535>, uy = UniformAxis<2, 0, 65535>,
        values = &EXTREME_VALUES, policy = policy,
        |surface, label| {
            for &y in &ys {
                for &x in &xs {
                    let value = surface.evaluate(x, y);
                    assert_eq!(
                        value,
                        reference::evaluate_tables(
                            &FULL_AXIS, &FULL_AXIS, &EXTREME_VALUES, policy, x, y
                        ),
                        "{label} at ({x}, {y})"
                    );
                    let value = value.expect("in domain");
                    assert!(
                        (lo..=hi).contains(&value),
                        "{label} at ({x}, {y}) left the hull"
                    );
                }
            }
        }
    );
}

fn check_edge_ends<X: AxisLookup<5>, Y: AxisLookup<3>>(
    surface: &BilinearSurface<5, 3, X, Y>,
    label: &str,
) {
    for x in [0u16, 1, 65_534, 65_535] {
        for y in 0..=4u16 {
            agrees(surface, &EDGE_X, &EDGE_Y, x, y, label);
        }
    }
    assert_eq!(surface.evaluate(0, 0), Ok(i32::MIN), "{label}");
    assert_eq!(surface.evaluate(65_535, 0), Ok(i32::MAX), "{label}");
    assert_eq!(surface.evaluate(0, 1), Ok(-1), "{label}");
}

#[test]
fn edges_end_cells_are_exhaustive_for_stored_and_mixed_pairings() {
    let policy = BoundaryPolicy::new();

    each_stored_9!(
        nx = 5,
        ny = 3,
        x = &EDGE_X,
        y = &EDGE_Y,
        xi = &EDGE_X_INDEX,
        yi = &EDGE_Y_INDEX,
        xb = 8,
        yb = 4,
        values = &EDGE_VALUES,
        policy = policy,
        |surface, label| {
            check_edge_ends(&surface, label);
        }
    );
    each_stored_x_uniform_y!(
        nx = 5, ny = 3,
        x = &EDGE_X, xi = &EDGE_X_INDEX, xb = 8,
        uy = UniformAxis<3, 0, 2>,
        values = &EDGE_VALUES, policy = policy,
        |surface, label| { check_edge_ends(&surface, label); }
    );
}

#[test]
fn mixed_grid_handle_is_stateless_across_repeat_copy_and_interleave() {
    static MIXED: BilinearSurface<3, 3, LinearAxis<3>, UniformAxis<3, 0, 4>> =
        BilinearSurface::from_axes(LinearAxis::new(&GRID_X), UniformAxis::new(), &GRID_VALUES);

    let probes = [(5u16, 2u16), (60, 6), (7, 1), (0, 0), (110, 8), (111, 0)];
    let first: Vec<_> = probes.iter().map(|&(x, y)| MIXED.evaluate(x, y)).collect();
    for _ in 0..64 {
        for (i, &(x, y)) in probes.iter().enumerate() {
            assert_eq!(MIXED.evaluate(x, y), first[i], "({x}, {y})");
        }
    }

    let copy = MIXED;
    let snapshot = MIXED;
    for &(x, y) in &probes {
        assert_eq!(copy.evaluate(x, y), MIXED.evaluate(x, y));
    }
    assert_eq!(MIXED, snapshot);
    assert_eq!(copy, snapshot);

    let before = MIXED.evaluate(7, 5);
    for y in 0..=8u16 {
        for x in 0..=110u16 {
            let _ = MIXED.evaluate(x, y);
        }
    }
    let _ = MIXED.evaluate(u16::MAX, u16::MAX);
    assert_eq!(MIXED.evaluate(7, 5), before);
}

#[test]
fn nested_buckets_never_increase_the_local_bound_on_the_documented_cluster() {
    // Same clustered axis as the `max_local_comparisons` rustdoc example.
    static KNOTS: [u16; 5] = [0, 1, 2, 3, 1_000];
    static COARSE: [u16; 2] = bucket_index(&KNOTS);
    static MID: [u16; 4] = bucket_index(&KNOTS);
    static FINE: [u16; 8] = bucket_index(&KNOTS);

    let coarse = max_local_comparisons(&KNOTS, &COARSE);
    let mid = max_local_comparisons(&KNOTS, &MID);
    let fine = max_local_comparisons(&KNOTS, &FINE);
    assert!(mid <= coarse);
    assert!(fine <= mid);
    assert!(fine <= coarse);
}

#[test]
fn locator_mutants_disagree_on_named_conformance_points() {
    let policy = BoundaryPolicy::new();
    // LOOKUP_4: clustered [5,6,7] then tail [7,500]. x=100 is in the tail cell.
    let x = 100u16;
    let accepted = reference::evaluate_tables(&LOOKUP_4_X, &LOOKUP_Y, &LOOKUP_4_V, policy, x, 0);
    assert_eq!(LOOKUP_4.evaluate(x, 0), accepted);
    assert_ne!(
        accepted,
        reference::mutant_evaluate_off_by_one_cell(
            &LOOKUP_4_X,
            &LOOKUP_Y,
            &LOOKUP_4_V,
            policy,
            x,
            0
        )
    );
    assert_ne!(
        accepted,
        reference::mutant_evaluate_bucketed_cluster_vs_tail(
            &LOOKUP_4_X,
            &LOOKUP_Y,
            &LOOKUP_4_V,
            policy,
            x,
            0
        )
    );

    // Even-spaced lookup: x=15 is interior to [10, 20]. Origin+1 or step+1
    // moves the interpolation endpoints and changes the value.
    let uniform_accepted = reference::evaluate_tables(
        &LOOKUP_UNIFORM_X,
        &LOOKUP_Y,
        &LOOKUP_UNIFORM_V,
        policy,
        15,
        0,
    );
    assert_eq!(LOOKUP_UNIFORM.evaluate(15, 0), uniform_accepted);
    assert_ne!(
        uniform_accepted,
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
        uniform_accepted,
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
