//! Exact firmware resource-budget comparisons from the public const cost API.
//!
//! Three declared shapes, each split into referenced static payload, handle
//! bytes, bounded operation structure, and target-dependent measurement.
//! Comparison counts are not cycles or WCET. Payload is not total RAM, flash,
//! or linked binary size. `HANDLE_BYTES` is `size_of` on this host and must be
//! re-read on the firmware target. See `docs/usage-guide.md` §10–11 and
//! `docs/choosing-a-strategy.md`.
//!
//! Host `main` is an assertion harness. Declarations are `static` and
//! `core`-compatible.

use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, BucketedAxis, LinearAxis, UniformAxis, bucket_index,
    max_local_comparisons,
};

fn tiny_linear_linear() {
    // 3×2 firmware table. Linear is the "tiny axis" starting point; Binary
    // stores the same knots and, on this shape, the same comparison bound.
    static X: [u16; 3] = [0, 10, 20];
    static Y: [u16; 2] = [0, 100];
    static VALUES: [[i32; 3]; 2] = [[0, 1, 2], [10, 11, 12]];

    type TinyLinear = BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>>;
    type TinyBinary = BilinearSurface<3, 2>;

    static LINEAR: TinyLinear =
        BilinearSurface::from_axes(LinearAxis::new(&X), LinearAxis::new(&Y), &VALUES);
    static BINARY: TinyBinary = BilinearSurface::new(&X, &Y, &VALUES);

    assert_eq!(LINEAR.evaluate(10, 100), Ok(11));
    assert_eq!(LINEAR.evaluate(10, 100), BINARY.evaluate(10, 100));

    // Referenced payload: 4*3*2 values + 2*3 + 2*2 knots = 24 + 10 = 34.
    assert_eq!(TinyLinear::VALUE_BYTES, 24);
    assert_eq!(TinyLinear::PAYLOAD_BYTES, 34);
    assert_eq!(TinyBinary::PAYLOAD_BYTES, 34);
    assert_eq!(TinyLinear::HANDLE_BYTES, core::mem::size_of::<TinyLinear>());

    // Work: four successful endpoint comparisons plus (3-1)+(2-1) = 3 search
    // comparisons = 7 knot comparisons. Binary is ceil(log2(3))+ceil(log2(2))
    // = 2+1 = 3 search comparisons as well. Choosing Linear vs Binary on this
    // shape requires target code/timing evidence, not a universal threshold.
    assert_eq!(<LinearAxis<3>>::MAX_SEARCH_COMPARISONS, 2);
    assert_eq!(<LinearAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
    assert_eq!(<BinaryAxis<3>>::MAX_SEARCH_COMPARISONS, 2);
    assert_eq!(<BinaryAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
    assert_eq!(TinyLinear::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(TinyLinear::SUCCESS_GRID_READS, 4);
}

fn uniform_uniform_17x9() {
    // Both axes are exact arithmetic progressions, so Uniform stores no knots.
    // Location is a subtraction and a division by a compile-time STEP — zero
    // knot comparisons, not zero cycles.
    type UniformPair = BilinearSurface<17, 9, UniformAxis<17, 0, 100>, UniformAxis<9, 0, 200>>;
    type AllBinary = BilinearSurface<17, 9>;

    static VALUES: [[i32; 17]; 9] = [[0; 17]; 9];
    static SURFACE: UniformPair =
        BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &VALUES);

    assert_eq!(SURFACE.evaluate(100, 200), Ok(0));
    assert_eq!(SURFACE.x_knot(16), 1_600);
    assert_eq!(SURFACE.y_knot(8), 1_600);

    assert_eq!(UniformPair::VALUE_BYTES, 612);
    assert_eq!(UniformPair::PAYLOAD_BYTES, 612);
    assert_eq!(AllBinary::PAYLOAD_BYTES, 664);
    assert_eq!(UniformPair::PAYLOAD_BYTES + 52, AllBinary::PAYLOAD_BYTES);
    assert_eq!(<UniformAxis<17, 0, 100>>::KNOT_BYTES, 0);
    assert_eq!(<UniformAxis<17, 0, 100>>::MAX_SEARCH_COMPARISONS, 0);
    assert_eq!(<UniformAxis<9, 0, 200>>::MAX_SEARCH_COMPARISONS, 0);
    assert_eq!(UniformPair::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(UniformPair::SUCCESS_GRID_READS, 4);
    assert_eq!(
        UniformPair::HANDLE_BYTES,
        core::mem::size_of::<UniformPair>()
    );
}

fn mixed_bucketed_uniform_17x9() {
    static X: [u16; 17] = [
        0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300, 1_410, 1_500,
        1_600,
    ];
    static X_INDEX: [u16; 8] = bucket_index(&X);
    static VALUES: [[i32; 17]; 9] = [[0; 17]; 9];

    type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
    type AllBinary = BilinearSurface<17, 9>;

    static MIXED: Mixed =
        BilinearSurface::from_axes(BucketedAxis::new(&X, &X_INDEX), UniformAxis::new(), &VALUES);

    assert_eq!(MIXED.evaluate(1_600, 1_600), Ok(0));

    // Payload: 612 grid + 34 X knots + 16 X index + 0 Y = 662. Binary is 664.
    assert_eq!(Mixed::VALUE_BYTES, 612);
    assert_eq!(<BucketedAxis<17, 8>>::KNOT_BYTES, 34);
    assert_eq!(<BucketedAxis<17, 8>>::INDEX_BYTES, 16);
    assert_eq!(<UniformAxis<9, 0, 200>>::KNOT_BYTES, 0);
    assert_eq!(Mixed::PAYLOAD_BYTES, 662);
    assert_eq!(AllBinary::PAYLOAD_BYTES, 664);
    assert_eq!(max_local_comparisons(&X, &X_INDEX), 3);

    // Work: four endpoint comparisons, plus at most 3 X local comparisons and
    // 0 Y comparisons = 7 knot comparisons, versus 4 + 5 + 4 = 13 for
    // Binary/Binary. Each Bucketed search also reads one bucket and maps the
    // coordinate arithmetically; that is not included in the comparison count
    // and is not a cycle count.
    assert_eq!(<BinaryAxis<17>>::MAX_SEARCH_COMPARISONS, 5);
    assert_eq!(<BinaryAxis<9>>::MAX_SEARCH_COMPARISONS, 4);
    assert_eq!(<UniformAxis<9, 0, 200>>::MAX_SEARCH_COMPARISONS, 0);
    assert_eq!(Mixed::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(Mixed::SUCCESS_GRID_READS, 4);
    assert_eq!(Mixed::HANDLE_BYTES, core::mem::size_of::<Mixed>());
}

fn main() {
    tiny_linear_linear();
    uniform_uniform_17x9();
    mixed_bucketed_uniform_17x9();
}
