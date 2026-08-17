//! Uniform/Uniform compensation over two evenly coded operating inputs.
//!
//! Both axes are exact arithmetic progressions, so Uniform stores no knot
//! arrays: location is one subtraction and one division per axis, not a search.
//! That is not zero work in cycles; measure the division on the target if
//! timing matters. See `docs/choosing-a-strategy.md`.
//!
//! Host `main` is an assertion harness. Declarations are `static` and
//! `core`-compatible. Payload figures are referenced element bytes, not total
//! flash or RAM.

use ph_surfaces::{AxisLookup, BilinearSurface, UniformAxis};

// X codes 0, 100, 200 and Y codes 0, 50, 100 — described, not stored.
type Compensation = BilinearSurface<3, 3, UniformAxis<3, 0, 100>, UniformAxis<3, 0, 50>>;

static VALUES: [[i32; 3]; 3] = [[0, 20, 40], [10, 30, 50], [20, 40, 60]];

static SURFACE: Compensation =
    BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &VALUES);

// Equivalent default surface over the same described knots, for equality.
static X: [u16; 3] = [0, 100, 200];
static Y: [u16; 3] = [0, 50, 100];
static DEFAULT: BilinearSurface<3, 3> = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    // Endpoint accessors reconstruct the arithmetic progression.
    assert_eq!(SURFACE.x_knot(0), 0);
    assert_eq!(SURFACE.x_knot(2), 200);
    assert_eq!(SURFACE.y_knot(1), 50);

    // Interior: lower-X 10, upper-X 20, Y 15. Hand-computable plane.
    assert_eq!(SURFACE.evaluate(50, 25), Ok(15));
    assert_eq!(SURFACE.evaluate(200, 100), Ok(60));
    for x in [0u16, 50, 100, 199, 200] {
        for y in [0u16, 25, 50, 99, 100] {
            assert_eq!(SURFACE.evaluate(x, y), DEFAULT.evaluate(x, y));
        }
    }

    assert_eq!(<UniformAxis<3, 0, 100>>::KNOT_BYTES, 0);
    assert_eq!(<UniformAxis<3, 0, 50>>::KNOT_BYTES, 0);
    assert_eq!(<UniformAxis<3, 0, 100>>::MAX_SEARCH_COMPARISONS, 0);
    assert_eq!(Compensation::VALUE_BYTES, 36);
    assert_eq!(Compensation::PAYLOAD_BYTES, 36);
    assert_eq!(BilinearSurface::<3, 3>::PAYLOAD_BYTES, 48);
    assert_eq!(
        Compensation::HANDLE_BYTES,
        core::mem::size_of::<Compensation>()
    );
}
