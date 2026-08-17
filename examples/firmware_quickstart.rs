//! Minimal Binary/Binary static correction surface for firmware.
//!
//! Binary is the safe default: general irregular or unknown spacing, no extra
//! index bytes, exact `ceil(log2(N))` probes per axis. This table is tiny, so
//! Linear would store the same knots; choosing between them is a target-code
//! question, not a payload one. See `docs/choosing-a-strategy.md`.
//!
//! Host `main` is an assertion harness. The declarations are `static` and
//! `core`-compatible: no heap, no I/O, no cache, no runtime table construction.
//! Payload and comparison counts are not flash, cycles, or WCET; measure those
//! on a named target.

use ph_surfaces::{AxisLookup, BilinearSurface, BinaryAxis, SurfaceError};

static X: [u16; 2] = [100, 200];
static Y: [u16; 2] = [10, 30];
static VALUES: [[i32; 2]; 2] = [
    [0, 100],  // Y = 10
    [40, 180], // Y = 30
];

static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    // Declared knot: stored value, no interpolation.
    assert_eq!(SURFACE.evaluate(100, 10), Ok(0));
    // Interior operating point from docs/interpolation-walkthrough.md:
    // lower-X 25, upper-X 75, Y 50.
    assert_eq!(SURFACE.evaluate(125, 20), Ok(50));
    assert_eq!(
        SURFACE.evaluate(0, 20),
        Err(SurfaceError::XBelow {
            coordinate: 0,
            bound: 100
        })
    );

    assert_eq!(BilinearSurface::<2, 2>::VALUE_BYTES, 16);
    assert_eq!(BilinearSurface::<2, 2>::PAYLOAD_BYTES, 24);
    assert_eq!(BilinearSurface::<2, 2>::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(BilinearSurface::<2, 2>::SUCCESS_GRID_READS, 4);
    assert_eq!(<BinaryAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
    assert_eq!(
        BilinearSurface::<2, 2>::HANDLE_BYTES,
        core::mem::size_of::<BilinearSurface<2, 2>>()
    );
}
