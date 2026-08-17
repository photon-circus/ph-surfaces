//! Fail-safe boundary sides on a static firmware correction map.
//!
//! Reject uncharacterized low operating codes (`Error`) and hold the last
//! characterized high edge (`Clamp`). All four sides are named. X is resolved
//! before Y, so an X error wins when both inputs are invalid, and a clamped X
//! still lets Y error. Clamp never extrapolates: it evaluates the endpoint
//! cell. See `docs/interpolation-walkthrough.md`.
//!
//! Host `main` is an assertion harness. Declarations are `static` and
//! `core`-compatible.

use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static X: [u16; 2] = [100, 200];
static Y: [u16; 2] = [10, 30];
static VALUES: [[i32; 2]; 2] = [
    [0, 100],  // Y = 10
    [40, 180], // Y = 30
];

static STRICT: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

static FAIL_SAFE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES).with_policy(
    BoundaryPolicy::new()
        .with_x_below(Boundary::Error)
        .with_x_above(Boundary::Clamp)
        .with_y_below(Boundary::Error)
        .with_y_above(Boundary::Clamp),
);

fn main() {
    assert_eq!(FAIL_SAFE.policy().x_below(), Boundary::Error);
    assert_eq!(FAIL_SAFE.policy().x_above(), Boundary::Clamp);
    assert_eq!(FAIL_SAFE.policy().y_below(), Boundary::Error);
    assert_eq!(FAIL_SAFE.policy().y_above(), Boundary::Clamp);

    // In-domain: policy is idle.
    assert_eq!(FAIL_SAFE.evaluate(125, 20), Ok(50));
    assert_eq!(FAIL_SAFE.evaluate(125, 20), STRICT.evaluate(125, 20));

    // Reject uncharacterized low codes.
    assert_eq!(
        FAIL_SAFE.evaluate(0, 20),
        Err(SurfaceError::XBelow {
            coordinate: 0,
            bound: 100
        })
    );
    assert_eq!(
        FAIL_SAFE.evaluate(125, 0),
        Err(SurfaceError::YBelow {
            coordinate: 0,
            bound: 10
        })
    );

    // Hold the last characterized high edge; nothing is extrapolated.
    assert_eq!(FAIL_SAFE.evaluate(4_000, 20), FAIL_SAFE.evaluate(200, 20));
    assert_eq!(FAIL_SAFE.evaluate(125, 4_000), FAIL_SAFE.evaluate(125, 30));
    assert_eq!(FAIL_SAFE.evaluate(4_000, 20), Ok(140));
    assert_eq!(FAIL_SAFE.evaluate(125, 4_000), Ok(75));

    // Both outside Error sides: X wins and Y is not reported.
    assert_eq!(
        STRICT.evaluate(0, 0),
        Err(SurfaceError::XBelow {
            coordinate: 0,
            bound: 100
        })
    );
    // X clamps, Y still errors.
    assert_eq!(
        FAIL_SAFE.evaluate(4_000, 0),
        Err(SurfaceError::YBelow {
            coordinate: 0,
            bound: 10
        })
    );
}
