//! Every static surface the suite evaluates. Each fixture states why it exists.
//!
//! All tables are `static` so the surfaces are the same `&'static` handles a
//! firmware consumer would define; nothing here is built at test time.

use ph_surfaces::{BilinearSurface, BoundaryPolicy};

// ---------------------------------------------------------------------------
// The locked order fixture from issue #6.
//
// Axes [0, 2], rows [[0, 0], [1, 3]], input (1, 1). X-then-Y: rows -> 0 and
// (1 + 3) / 2 = 2, then (0 + 2) / 2 = 1. Y-then-X: columns -> (0 + 1) / 2 = 1
// (0.5 rounds away) and (0 + 3) / 2 = 2 (1.5 rounds away), then (1 + 2) / 2 =
// 2 (1.5 rounds away). Reversing the order therefore necessarily fails.
// ---------------------------------------------------------------------------
pub static ORDER_AXIS: [u16; 2] = [0, 2];
pub static ORDER_VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
pub static ORDER: BilinearSurface<2, 2> =
    BilinearSurface::new(&ORDER_AXIS, &ORDER_AXIS, &ORDER_VALUES);

// A hand-computable plane: value = 10 * x + 100 * y. Bilinear interpolation of
// a plane is exact, so every interior value is a closed form.
pub static PLANE_X: [u16; 2] = [0, 10];
pub static PLANE_Y: [u16; 2] = [0, 10];
pub static PLANE_VALUES: [[i32; 2]; 2] = [[0, 100], [1_000, 1_100]];
pub static PLANE: BilinearSurface<2, 2> = BilinearSurface::new(&PLANE_X, &PLANE_Y, &PLANE_VALUES);

// A nonuniform 3x3 grid: a 10-wide then a 100-wide X cell, uniform Y, and a
// negative bottom-left so the interior crosses zero.
pub static GRID_X: [u16; 3] = [0, 10, 110];
pub static GRID_Y: [u16; 3] = [0, 4, 8];
pub static GRID_VALUES: [[i32; 3]; 3] = [[0, 100, 300], [10, 110, 310], [-40, 60, 260]];
pub static GRID: BilinearSurface<3, 3> = BilinearSurface::new(&GRID_X, &GRID_Y, &GRID_VALUES);

// Rows chosen for their shapes: positive increasing, negative decreasing, flat,
// zero-crossing. The middle column is flat along Y between rows 2 and 3 only
// through the value 7 -> 0, so a flat *column* is provided separately below.
pub static SHAPE_X: [u16; 3] = [0, 100, 200];
pub static SHAPE_Y: [u16; 4] = [0, 10, 20, 30];
pub static SHAPE_VALUES: [[i32; 3]; 4] = [
    [10, 20, 30],    // positive, increasing
    [-10, -20, -30], // negative, decreasing
    [7, 7, 7],       // flat row
    [-100, 0, 100],  // zero-crossing
];
pub static SHAPES: BilinearSurface<3, 4> = BilinearSurface::new(&SHAPE_X, &SHAPE_Y, &SHAPE_VALUES);

// A flat column (x = 5) next to sloped ones, and decreasing data along Y.
pub static COLUMN_X: [u16; 3] = [0, 5, 9];
pub static COLUMN_Y: [u16; 3] = [0, 3, 9];
pub static COLUMN_VALUES: [[i32; 3]; 3] = [[30, -4, 20], [10, -4, 0], [-30, -4, -60]];
pub static COLUMNS: BilinearSurface<3, 3> =
    BilinearSurface::new(&COLUMN_X, &COLUMN_Y, &COLUMN_VALUES);

// Half-way ties on both rows: X midpoints are +0.5 and -0.5 before rounding.
pub static TIE_AXIS: [u16; 2] = [0, 2];
pub static TIE_VALUES: [[i32; 2]; 2] = [[0, 1], [0, -1]];
pub static TIES: BilinearSurface<2, 2> = BilinearSurface::new(&TIE_AXIS, &TIE_AXIS, &TIE_VALUES);

// A signed grid and its exact negation, for the sign-reflection property:
// ties-away-from-zero is odd-symmetric, so evaluate(-V) == -evaluate(V) at
// every point. Even spans (2, 8 on X; 4, 2 on Y) make exact ties frequent.
pub static SIGN_X: [u16; 3] = [0, 2, 10];
pub static SIGN_Y: [u16; 3] = [0, 4, 6];
pub static SIGN_VALUES: [[i32; 3]; 3] = [[1, -2, 5], [-7, 4, -3], [9, 0, -11]];
pub static SIGN_NEGATED: [[i32; 3]; 3] = [[-1, 2, -5], [7, -4, 3], [-9, 0, 11]];
pub static SIGNED: BilinearSurface<3, 3> = BilinearSurface::new(&SIGN_X, &SIGN_Y, &SIGN_VALUES);
pub static SIGNED_NEGATED: BilinearSurface<3, 3> =
    BilinearSurface::new(&SIGN_X, &SIGN_Y, &SIGN_NEGATED);

// A domain with room on every side, so each side can be probed one unit out
// and every corner region is reachable. 31 x 301 = 9_331 declared points, so
// it is enumerated exhaustively.
pub static INSET_X: [u16; 3] = [10, 20, 40];
pub static INSET_Y: [u16; 3] = [100, 200, 400];
pub static INSET_VALUES: [[i32; 3]; 3] = [[0, 1, 2], [10, 11, 12], [20, 21, 22]];

pub const fn inset(policy: BoundaryPolicy) -> BilinearSurface<3, 3> {
    BilinearSurface::new(&INSET_X, &INSET_Y, &INSET_VALUES).with_policy(policy)
}

pub static INSET: BilinearSurface<3, 3> = inset(BoundaryPolicy::new());

// The full u16 span on both axes with the extreme i32 corners: the widest
// operands a v0.1 surface can present. Its 2^32-point domain is sampled, never
// enumerated.
pub static FULL_AXIS: [u16; 2] = [0, u16::MAX];
pub static EXTREME_VALUES: [[i32; 2]; 2] = [[i32::MIN, i32::MAX], [i32::MAX, i32::MIN]];
pub static EXTREME: BilinearSurface<2, 2> =
    BilinearSurface::new(&FULL_AXIS, &FULL_AXIS, &EXTREME_VALUES);

// Axes that *contain* 0 and u16::MAX next to their neighbours, with tiny
// interior cells, and grids holding i32::MIN / i32::MAX beside small values.
// The X domain is the full range so it is sampled; the tiny cells at each end
// are enumerated exhaustively by the tests that use them.
pub static EDGE_X: [u16; 5] = [0, 1, 32_768, 65_534, 65_535];
pub static EDGE_Y: [u16; 3] = [0, 2, 4];
pub static EDGE_VALUES: [[i32; 5]; 3] = [
    [i32::MIN, 0, 1, 0, i32::MAX],
    [i32::MAX, -1, 0, 1, i32::MIN],
    [i32::MIN + 1, i32::MAX - 1, 0, i32::MIN + 1, i32::MAX - 1],
];
pub static EDGES: BilinearSurface<5, 3> = BilinearSurface::new(&EDGE_X, &EDGE_Y, &EDGE_VALUES);

// ---------------------------------------------------------------------------
// Lookup axes. Each pairs with a two-row grid whose values are strictly convex
// in the knot index (i * i and a shifted copy), so a wrongly located cell
// changes the interpolated value rather than hiding behind collinear data.
// ---------------------------------------------------------------------------
pub static LOOKUP_Y: [u16; 2] = [0, 1];

macro_rules! lookup_fixture {
    ($surface:ident, $axis:ident, $values:ident, $n:literal, [$($knot:expr),* $(,)?]) => {
        pub static $axis: [u16; $n] = [$($knot),*];
        pub static $values: [[i32; $n]; 2] = convex::<$n>();
        pub static $surface: BilinearSurface<$n, 2> =
            BilinearSurface::new(&$axis, &LOOKUP_Y, &$values);
    };
}

pub const fn convex<const N: usize>() -> [[i32; N]; 2] {
    let mut out = [[0i32; N]; 2];
    let mut i = 0;
    while i < N {
        let index = i as i32;
        out[0][i] = index * index;
        out[1][i] = 1_000 - index * index * 3;
        i += 1;
    }
    out
}

lookup_fixture!(LOOKUP_2, LOOKUP_2_X, LOOKUP_2_V, 2, [3, 40]);
lookup_fixture!(LOOKUP_3, LOOKUP_3_X, LOOKUP_3_V, 3, [0, 1, 300]);
lookup_fixture!(LOOKUP_4, LOOKUP_4_X, LOOKUP_4_V, 4, [5, 6, 7, 500]);
lookup_fixture!(LOOKUP_5, LOOKUP_5_X, LOOKUP_5_V, 5, [10, 11, 200, 201, 202]);
lookup_fixture!(
    LOOKUP_7,
    LOOKUP_7_X,
    LOOKUP_7_V,
    7,
    [1, 2, 4, 8, 16, 32, 64]
);
lookup_fixture!(
    LOOKUP_8,
    LOOKUP_8_X,
    LOOKUP_8_V,
    8,
    [0, 100, 101, 102, 103, 900, 901, 1000]
);
lookup_fixture!(
    LOOKUP_9,
    LOOKUP_9_X,
    LOOKUP_9_V,
    9,
    [7, 9, 10, 11, 12, 13, 14, 15, 2000]
);
lookup_fixture!(
    LOOKUP_17,
    LOOKUP_17_X,
    LOOKUP_17_V,
    17,
    [
        0, 1, 3, 4, 6, 7, 9, 10, 12, 13, 15, 16, 18, 19, 21, 22, 60_000
    ]
);

// 1024 knots: a nonuniform stride pattern (mostly 3, every eighth gap 11) so
// no arithmetic shortcut could stand in for a search. Last knot is well below
// u16::MAX so the domain (0..=4_085) is small enough to enumerate.
pub static LOOKUP_1024_X: [u16; 1024] = {
    let mut axis = [0u16; 1024];
    let mut i = 0;
    let mut knot = 0u16;
    while i < 1024 {
        axis[i] = knot;
        i += 1;
        if i < 1024 {
            knot += if i % 8 == 0 { 11 } else { 3 };
        }
    }
    axis
};
pub static LOOKUP_1024_V: [[i32; 1024]; 2] = convex::<1024>();
pub static LOOKUP_1024: BilinearSurface<1024, 2> =
    BilinearSurface::new(&LOOKUP_1024_X, &LOOKUP_Y, &LOOKUP_1024_V);

// ---------------------------------------------------------------------------
// Two unrelated device-neutral examples. They demonstrate shape and rounding
// behaviour only. Neither claims anything about a sensor, a vendor, or total
// measurement accuracy.
// ---------------------------------------------------------------------------

// A mixed-sign nonuniform elevation map: X and Y are plan-view positions in
// arbitrary distance units, values are height relative to a datum in the same
// units (negative below datum). Rows are non-monotone; the axes are unevenly
// spaced. 181 x 151 = 27_331 declared points, enumerated exhaustively.
pub static ELEVATION_X: [u16; 5] = [0, 25, 60, 100, 180];
pub static ELEVATION_Y: [u16; 4] = [0, 40, 90, 150];
pub static ELEVATION_VALUES: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
pub static ELEVATION: BilinearSurface<5, 4> =
    BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES);

// An asymmetric process-correction map: X is a setpoint code, Y a load code,
// values are a signed correction in milli-units. Neither axis is symmetric,
// the correction changes sign, and no row or column mirrors another. 161 x 121
// = 19_481 declared points, enumerated exhaustively.
pub static CORRECTION_X: [u16; 4] = [40, 55, 90, 200];
pub static CORRECTION_Y: [u16; 5] = [0, 10, 25, 70, 120];
pub static CORRECTION_VALUES: [[i32; 4]; 5] = [
    [125, 80, -15, -140],
    [90, 41, -33, -170],
    [30, -7, -61, -205],
    [-48, -95, -150, -260],
    [-110, -142, -199, -333],
];
pub static CORRECTION: BilinearSurface<4, 5> =
    BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES);
