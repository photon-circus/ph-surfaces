//! The four independent boundary sides under Error and Clamp, corner
//! interactions, X-before-Y precedence, and the absence of extrapolation.
//!
//! All cases use the INSET fixture: X in [10, 40], Y in [100, 400], so every
//! side has room to probe one unit out and every corner region is reachable.

use crate::fixtures::*;
use crate::reference::{self, policy_from_bits};
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

const X_MIN: u16 = 10;
const X_MAX: u16 = 40;
const Y_MIN: u16 = 100;
const Y_MAX: u16 = 400;

// The stored corner values, named for the corner tests.
const BOTTOM_LEFT: i32 = INSET_VALUES[0][0];
const BOTTOM_RIGHT: i32 = INSET_VALUES[0][2];
const TOP_LEFT: i32 = INSET_VALUES[2][0];
const TOP_RIGHT: i32 = INSET_VALUES[2][2];

// Coordinates one unit outside each side, and the far extremes 0 / u16::MAX.
const X_LOW: u16 = X_MIN - 1;
const X_HIGH: u16 = X_MAX + 1;
const Y_LOW: u16 = Y_MIN - 1;
const Y_HIGH: u16 = Y_MAX + 1;

const fn x_below(c: u16) -> SurfaceError {
    SurfaceError::XBelow {
        coordinate: c,
        bound: X_MIN,
    }
}
const fn x_above(c: u16) -> SurfaceError {
    SurfaceError::XAbove {
        coordinate: c,
        bound: X_MAX,
    }
}
const fn y_below(c: u16) -> SurfaceError {
    SurfaceError::YBelow {
        coordinate: c,
        bound: Y_MIN,
    }
}
const fn y_above(c: u16) -> SurfaceError {
    SurfaceError::YAbove {
        coordinate: c,
        bound: Y_MAX,
    }
}

// ---------------------------------------------------------------------------
// The eight side-by-policy cases. Each probes exactly one side with the other
// coordinate in domain, so no precedence question arises.
// ---------------------------------------------------------------------------

#[test]
fn x_below_under_error_reports_the_coordinate_and_the_first_x_knot() {
    assert_eq!(INSET.evaluate(X_LOW, 250), Err(x_below(X_LOW)));
    assert_eq!(INSET.evaluate(0, 250), Err(x_below(0)));
}

#[test]
fn x_below_under_clamp_evaluates_the_first_x_column() {
    static S: BilinearSurface<3, 3> = inset(BoundaryPolicy::new().with_x_below(Boundary::Clamp));
    for y in [Y_MIN, 150, 250, Y_MAX] {
        assert_eq!(S.evaluate(X_LOW, y), S.evaluate(X_MIN, y), "y = {y}");
        assert_eq!(S.evaluate(0, y), S.evaluate(X_MIN, y), "y = {y}");
        assert_eq!(S.evaluate(X_LOW, y), reference::evaluate(&S, X_LOW, y));
    }
}

#[test]
fn x_above_under_error_reports_the_coordinate_and_the_last_x_knot() {
    assert_eq!(INSET.evaluate(X_HIGH, 250), Err(x_above(X_HIGH)));
    assert_eq!(INSET.evaluate(u16::MAX, 250), Err(x_above(u16::MAX)));
}

#[test]
fn x_above_under_clamp_evaluates_the_last_x_column() {
    static S: BilinearSurface<3, 3> = inset(BoundaryPolicy::new().with_x_above(Boundary::Clamp));
    for y in [Y_MIN, 150, 250, Y_MAX] {
        assert_eq!(S.evaluate(X_HIGH, y), S.evaluate(X_MAX, y), "y = {y}");
        assert_eq!(S.evaluate(u16::MAX, y), S.evaluate(X_MAX, y), "y = {y}");
        assert_eq!(S.evaluate(X_HIGH, y), reference::evaluate(&S, X_HIGH, y));
    }
}

#[test]
fn y_below_under_error_reports_the_coordinate_and_the_first_y_knot() {
    assert_eq!(INSET.evaluate(25, Y_LOW), Err(y_below(Y_LOW)));
    assert_eq!(INSET.evaluate(25, 0), Err(y_below(0)));
}

#[test]
fn y_below_under_clamp_evaluates_the_first_y_row() {
    static S: BilinearSurface<3, 3> = inset(BoundaryPolicy::new().with_y_below(Boundary::Clamp));
    for x in [X_MIN, 15, 25, X_MAX] {
        assert_eq!(S.evaluate(x, Y_LOW), S.evaluate(x, Y_MIN), "x = {x}");
        assert_eq!(S.evaluate(x, 0), S.evaluate(x, Y_MIN), "x = {x}");
        assert_eq!(S.evaluate(x, Y_LOW), reference::evaluate(&S, x, Y_LOW));
    }
}

#[test]
fn y_above_under_error_reports_the_coordinate_and_the_last_y_knot() {
    assert_eq!(INSET.evaluate(25, Y_HIGH), Err(y_above(Y_HIGH)));
    assert_eq!(INSET.evaluate(25, u16::MAX), Err(y_above(u16::MAX)));
}

#[test]
fn y_above_under_clamp_evaluates_the_last_y_row() {
    static S: BilinearSurface<3, 3> = inset(BoundaryPolicy::new().with_y_above(Boundary::Clamp));
    for x in [X_MIN, 15, 25, X_MAX] {
        assert_eq!(S.evaluate(x, Y_HIGH), S.evaluate(x, Y_MAX), "x = {x}");
        assert_eq!(S.evaluate(x, u16::MAX), S.evaluate(x, Y_MAX), "x = {x}");
        assert_eq!(S.evaluate(x, Y_HIGH), reference::evaluate(&S, x, Y_HIGH));
    }
}

// ---------------------------------------------------------------------------
// Corners: both coordinates out of domain at once, under each of the four
// X-policy / Y-policy interactions. "Error/Clamp" means the X side errors and
// the Y side clamps, and so on. Every corner appears under every interaction.
// ---------------------------------------------------------------------------

static ALL_ERROR: BilinearSurface<3, 3> = inset(policy_from_bits(0b0000));
static X_ERROR_Y_CLAMP: BilinearSurface<3, 3> = inset(policy_from_bits(0b1100));
static X_CLAMP_Y_ERROR: BilinearSurface<3, 3> = inset(policy_from_bits(0b0011));
static ALL_CLAMP: BilinearSurface<3, 3> = inset(policy_from_bits(0b1111));

#[test]
fn error_error_corners_report_the_x_side() {
    // X is resolved first, so with both sides erroring the X error is the one
    // reported at every corner, and the Y coordinate does not appear.
    assert_eq!(ALL_ERROR.evaluate(X_LOW, Y_LOW), Err(x_below(X_LOW)));
    assert_eq!(ALL_ERROR.evaluate(X_LOW, Y_HIGH), Err(x_below(X_LOW)));
    assert_eq!(ALL_ERROR.evaluate(X_HIGH, Y_LOW), Err(x_above(X_HIGH)));
    assert_eq!(ALL_ERROR.evaluate(X_HIGH, Y_HIGH), Err(x_above(X_HIGH)));
    // The same at the extremes of the coordinate space.
    assert_eq!(ALL_ERROR.evaluate(0, 0), Err(x_below(0)));
    assert_eq!(ALL_ERROR.evaluate(0, u16::MAX), Err(x_below(0)));
    assert_eq!(ALL_ERROR.evaluate(u16::MAX, 0), Err(x_above(u16::MAX)));
    assert_eq!(
        ALL_ERROR.evaluate(u16::MAX, u16::MAX),
        Err(x_above(u16::MAX))
    );
}

#[test]
fn error_clamp_corners_still_report_the_x_side() {
    // X errors, Y would clamp: the X error is returned and Y is never resolved.
    assert_eq!(X_ERROR_Y_CLAMP.evaluate(X_LOW, Y_LOW), Err(x_below(X_LOW)));
    assert_eq!(X_ERROR_Y_CLAMP.evaluate(X_LOW, Y_HIGH), Err(x_below(X_LOW)));
    assert_eq!(
        X_ERROR_Y_CLAMP.evaluate(X_HIGH, Y_LOW),
        Err(x_above(X_HIGH))
    );
    assert_eq!(
        X_ERROR_Y_CLAMP.evaluate(X_HIGH, Y_HIGH),
        Err(x_above(X_HIGH))
    );
}

#[test]
fn clamp_error_corners_report_the_y_side() {
    // X clamps, so it does not suppress the Y error: Y is resolved under its
    // own selection and its error is returned.
    assert_eq!(X_CLAMP_Y_ERROR.evaluate(X_LOW, Y_LOW), Err(y_below(Y_LOW)));
    assert_eq!(
        X_CLAMP_Y_ERROR.evaluate(X_LOW, Y_HIGH),
        Err(y_above(Y_HIGH))
    );
    assert_eq!(X_CLAMP_Y_ERROR.evaluate(X_HIGH, Y_LOW), Err(y_below(Y_LOW)));
    assert_eq!(
        X_CLAMP_Y_ERROR.evaluate(X_HIGH, Y_HIGH),
        Err(y_above(Y_HIGH))
    );
}

#[test]
fn clamp_clamp_corners_return_the_stored_corner_value() {
    assert_eq!(ALL_CLAMP.evaluate(X_LOW, Y_LOW), Ok(BOTTOM_LEFT));
    assert_eq!(ALL_CLAMP.evaluate(X_HIGH, Y_LOW), Ok(BOTTOM_RIGHT));
    assert_eq!(ALL_CLAMP.evaluate(X_LOW, Y_HIGH), Ok(TOP_LEFT));
    assert_eq!(ALL_CLAMP.evaluate(X_HIGH, Y_HIGH), Ok(TOP_RIGHT));
    assert_eq!(ALL_CLAMP.evaluate(0, 0), Ok(BOTTOM_LEFT));
    assert_eq!(ALL_CLAMP.evaluate(u16::MAX, 0), Ok(BOTTOM_RIGHT));
    assert_eq!(ALL_CLAMP.evaluate(0, u16::MAX), Ok(TOP_LEFT));
    assert_eq!(ALL_CLAMP.evaluate(u16::MAX, u16::MAX), Ok(TOP_RIGHT));
}

#[test]
fn a_y_first_precedence_would_fail_on_every_error_error_corner() {
    // The reversed-precedence mutant reports the Y error at each corner. The
    // accepted result differs at all four, so a linearized or reversed
    // precedence cannot pass the corner tests above.
    for (x, y) in [
        (X_LOW, Y_LOW),
        (X_LOW, Y_HIGH),
        (X_HIGH, Y_LOW),
        (X_HIGH, Y_HIGH),
    ] {
        let accepted = ALL_ERROR.evaluate(x, y);
        let mutant = reference::mutant_evaluate_y_precedence(&ALL_ERROR, x, y);
        assert_ne!(accepted, mutant, "corner ({x}, {y})");
        assert!(matches!(
            mutant,
            Err(SurfaceError::YBelow { .. } | SurfaceError::YAbove { .. })
        ));
    }
    // And on Error/Clamp corners the mutant would clamp Y and then hit the X
    // error, which happens to agree; on Clamp/Error corners it agrees too. So
    // the Error/Error corners are the discriminating evidence, and they are
    // asserted explicitly above.
}

// ---------------------------------------------------------------------------
// The full interaction sweep: every one of the sixteen policies against every
// region of the coordinate plane, compared with the reference.
// ---------------------------------------------------------------------------

/// Representative coordinates for the three regions of one axis: below the
/// domain, inside it (including both endpoints and interior knots), above it.
const X_PROBES: [u16; 11] = [0, 1, X_LOW, X_MIN, 11, 15, 20, 39, X_MAX, X_HIGH, u16::MAX];
const Y_PROBES: [u16; 11] = [
    0,
    1,
    Y_LOW,
    Y_MIN,
    101,
    150,
    200,
    399,
    Y_MAX,
    Y_HIGH,
    u16::MAX,
];

#[test]
fn all_sixteen_policies_agree_with_the_reference_on_every_region() {
    for bits in 0..16 {
        let surface = inset(policy_from_bits(bits));
        for &y in &Y_PROBES {
            for &x in &X_PROBES {
                assert_eq!(
                    surface.evaluate(x, y),
                    reference::evaluate(&surface, x, y),
                    "policy bits {bits:04b} at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn a_policy_selects_only_what_happens_outside_the_domain() {
    // Every one of the sixteen policies returns the same in-domain values,
    // over the whole 31 x 301 domain of the fixture.
    for bits in 1..16 {
        let surface = inset(policy_from_bits(bits));
        for y in Y_MIN..=Y_MAX {
            for x in X_MIN..=X_MAX {
                assert_eq!(
                    surface.evaluate(x, y),
                    INSET.evaluate(x, y),
                    "bits {bits:04b}"
                );
            }
        }
    }
}

#[test]
fn an_error_side_never_returns_a_value_and_a_clamp_side_never_returns_an_error() {
    for bits in 0..16 {
        let policy = policy_from_bits(bits);
        let surface = inset(policy);
        for &y in &Y_PROBES {
            for &x in &X_PROBES {
                let result = surface.evaluate(x, y);
                let x_side = if x < X_MIN {
                    Some(policy.x_below())
                } else if x > X_MAX {
                    Some(policy.x_above())
                } else {
                    None
                };
                let y_side = if y < Y_MIN {
                    Some(policy.y_below())
                } else if y > Y_MAX {
                    Some(policy.y_above())
                } else {
                    None
                };
                let expect_error =
                    x_side == Some(Boundary::Error) || y_side == Some(Boundary::Error);
                assert_eq!(
                    result.is_err(),
                    expect_error,
                    "bits {bits:04b} at ({x}, {y})"
                );
                if let Err(error) = result {
                    // The reported side is the X side whenever X errored.
                    let is_x = matches!(
                        error,
                        SurfaceError::XBelow { .. } | SurfaceError::XAbove { .. }
                    );
                    assert_eq!(
                        is_x,
                        x_side == Some(Boundary::Error),
                        "bits {bits:04b} at ({x}, {y})"
                    );
                    // The coordinate is echoed unchanged and the bound is a knot.
                    let expected_coordinate = if is_x { x } else { y };
                    assert_eq!(error.coordinate(), expected_coordinate);
                    assert!([X_MIN, X_MAX, Y_MIN, Y_MAX].contains(&error.bound()));
                }
            }
        }
    }
}

#[test]
fn clamping_never_extrapolates() {
    let (lo, hi) = reference::value_hull(&ALL_CLAMP);
    for &y in &Y_PROBES {
        for &x in &X_PROBES {
            let value = ALL_CLAMP.evaluate(x, y).expect("every side clamps");
            assert!(
                (lo..=hi).contains(&value),
                "({x}, {y}) returned {value}, outside [{lo}, {hi}]"
            );
        }
    }
    // The extrapolating mutant leaves the hull on every out-of-domain probe of
    // this fixture, so a Clamp implemented as extrapolation would fail here.
    let mut escaped = 0;
    for &y in &Y_PROBES {
        for &x in &X_PROBES {
            let in_domain = (X_MIN..=X_MAX).contains(&x) && (Y_MIN..=Y_MAX).contains(&y);
            let mutant = reference::mutant_evaluate_extrapolating(&ALL_CLAMP, x, y);
            if in_domain {
                assert_eq!(Ok(mutant), ALL_CLAMP.evaluate(x, y));
            } else if !(lo..=hi).contains(&mutant) {
                escaped += 1;
            }
        }
    }
    assert!(escaped > 0);
    // Named points: X one below under Clamp gives the first column, whereas
    // extrapolating along the sloped INSET rows would step past it.
    assert_eq!(ALL_CLAMP.evaluate(X_LOW, 100), Ok(0));
    assert_eq!(
        reference::mutant_evaluate_extrapolating(&ALL_CLAMP, 0, 100),
        -1
    );
    assert_eq!(ALL_CLAMP.evaluate(u16::MAX, 100), Ok(2));
    assert!(reference::mutant_evaluate_extrapolating(&ALL_CLAMP, u16::MAX, 100) > 2);
}

#[test]
fn a_clamped_x_is_followed_by_a_y_resolved_under_its_own_selection() {
    // X clamps on both sides; the four Y outcomes then follow Y's own policy.
    static Y_ERROR: BilinearSurface<3, 3> = inset(policy_from_bits(0b0011));
    static Y_CLAMP: BilinearSurface<3, 3> = inset(policy_from_bits(0b1111));
    assert_eq!(Y_ERROR.evaluate(0, Y_LOW), Err(y_below(Y_LOW)));
    assert_eq!(Y_ERROR.evaluate(u16::MAX, Y_HIGH), Err(y_above(Y_HIGH)));
    assert_eq!(Y_ERROR.evaluate(0, 250), Y_ERROR.evaluate(X_MIN, 250));
    assert_eq!(Y_CLAMP.evaluate(0, Y_LOW), Ok(BOTTOM_LEFT));
    assert_eq!(Y_CLAMP.evaluate(u16::MAX, Y_HIGH), Ok(TOP_RIGHT));
}

#[test]
fn a_clamped_y_never_masks_an_x_error() {
    static S: BilinearSurface<3, 3> = inset(policy_from_bits(0b1100));
    assert_eq!(S.evaluate(X_LOW, 0), Err(x_below(X_LOW)));
    assert_eq!(S.evaluate(X_HIGH, u16::MAX), Err(x_above(X_HIGH)));
    assert_eq!(S.evaluate(X_LOW, 250), Err(x_below(X_LOW)));
}

#[test]
fn the_domain_endpoints_are_inclusive_on_every_side() {
    // Under all-Error, the endpoints themselves are in domain.
    assert!(ALL_ERROR.evaluate(X_MIN, Y_MIN).is_ok());
    assert!(ALL_ERROR.evaluate(X_MAX, Y_MIN).is_ok());
    assert!(ALL_ERROR.evaluate(X_MIN, Y_MAX).is_ok());
    assert!(ALL_ERROR.evaluate(X_MAX, Y_MAX).is_ok());
    // And an endpoint under Clamp returns the same value as under Error.
    for (x, y) in [
        (X_MIN, Y_MIN),
        (X_MAX, Y_MIN),
        (X_MIN, Y_MAX),
        (X_MAX, Y_MAX),
    ] {
        assert_eq!(ALL_CLAMP.evaluate(x, y), ALL_ERROR.evaluate(x, y));
    }
}
