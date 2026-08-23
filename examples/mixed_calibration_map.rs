//! Mixed Bucketed-X / Uniform-Y calibration map.
//!
//! X is irregular characterized codes, so it keeps its knots and buys a
//! smaller local bound with a compile-time bucket index. Y is an exact
//! arithmetic progression, so Uniform drops its knot array. Changing these
//! strategies cannot change a value or an error. The nested-index tuning that
//! selected `B = 8` is in the strategy guide (not packaged):
//! <https://github.com/photon-circus/ph-surfaces/blob/v0.1.0/docs/choosing-a-strategy.md>.
//!
//! Host `main` is an assertion harness. Declarations are `static` and
//! `core`-compatible. Comparison counts are operation structure, not cycles.

use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, BucketedAxis, UniformAxis, bucket_index,
    max_local_comparisons,
};

static X: [u16; 17] = [
    0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300, 1_410, 1_500, 1_600,
];
static X_INDEX_2: [u16; 2] = bucket_index(&X);
static X_INDEX_4: [u16; 4] = bucket_index(&X);
static X_INDEX_8: [u16; 8] = bucket_index(&X);
static X_INDEX_16: [u16; 16] = bucket_index(&X);
static Y: [u16; 9] = [0, 200, 400, 600, 800, 1_000, 1_200, 1_400, 1_600];
static VALUES: [[i32; 17]; 9] = [[0; 17]; 9];

type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
type AllBinary = BilinearSurface<17, 9>;

static MIXED: Mixed = BilinearSurface::from_axes(
    BucketedAxis::new(&X, &X_INDEX_8),
    UniformAxis::new(),
    &VALUES,
);
static DEFAULT: AllBinary = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    // Nested tuning: discard indexes that do not beat Binary's bound of 5.
    assert_eq!(max_local_comparisons(&X, &X_INDEX_2), 9); // 4 index bytes; worse
    assert_eq!(max_local_comparisons(&X, &X_INDEX_4), 5); // 8 bytes; no improvement
    assert_eq!(max_local_comparisons(&X, &X_INDEX_8), 3); // 16 bytes; meets bound 3
    assert_eq!(max_local_comparisons(&X, &X_INDEX_16), 2); // 32 bytes; only if 3 is not enough
    assert_eq!(<BinaryAxis<17>>::MAX_SEARCH_COMPARISONS, 5);

    assert_eq!(MIXED.evaluate(610, 400), DEFAULT.evaluate(610, 400));
    assert_eq!(MIXED.evaluate(610, 400), Ok(0));
    assert_eq!(MIXED.y_knot(8), 1_600); // described, not stored
    for x in [0u16, 100, 610, 1_205, 1_600] {
        for y in [0u16, 200, 800, 1_600] {
            assert_eq!(MIXED.evaluate(x, y), DEFAULT.evaluate(x, y));
        }
    }

    assert_eq!(<BucketedAxis<17, 8>>::KNOT_BYTES, 34);
    assert_eq!(<BucketedAxis<17, 8>>::INDEX_BYTES, 16);
    assert_eq!(<UniformAxis<9, 0, 200>>::KNOT_BYTES, 0);
    assert_eq!(Mixed::PAYLOAD_BYTES, 662);
    assert_eq!(AllBinary::PAYLOAD_BYTES, 664);
}
