//! Minimal downstream `no_std` consumer of the packaged crate: the two
//! documented device-neutral example maps, declared and evaluated.
#![no_std]
#![forbid(unsafe_code)]

use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static ELEVATION_X: [u16; 5] = [0, 25, 60, 100, 180];
static ELEVATION_Y: [u16; 4] = [0, 40, 90, 150];
static ELEVATION_VALUES: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
static ELEVATION: BilinearSurface<5, 4> =
    BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES)
        .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));

static CORRECTION_X: [u16; 4] = [40, 55, 90, 200];
static CORRECTION_Y: [u16; 5] = [0, 10, 25, 70, 120];
static CORRECTION_VALUES: [[i32; 4]; 5] = [
    [125, 80, -15, -140],
    [90, 41, -33, -170],
    [30, -7, -61, -205],
    [-48, -95, -150, -260],
    [-110, -142, -199, -333],
];
static CORRECTION: BilinearSurface<4, 5> =
    BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES)
        .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));

/// Evaluates the elevation map; the caller sees the crate's error type.
pub fn elevation(x: u16, y: u16) -> Result<i32, SurfaceError> {
    ELEVATION.evaluate(x, y)
}

/// Evaluates the correction map; the caller sees the crate's error type.
pub fn correction(x: u16, y: u16) -> Result<i32, SurfaceError> {
    CORRECTION.evaluate(x, y)
}

/// Firmware quickstart, Uniform compensation, and mixed Bucketed/Uniform
/// tables from the packaged examples. Declared as `static`s with no allocator
/// or runtime initialization.
pub mod firmware {
    use ph_surfaces::{BilinearSurface, BucketedAxis, SurfaceError, UniformAxis, bucket_index};

    static QUICKSTART_X: [u16; 2] = [100, 200];
    static QUICKSTART_Y: [u16; 2] = [10, 30];
    static QUICKSTART_VALUES: [[i32; 2]; 2] = [[0, 100], [40, 180]];
    static QUICKSTART: BilinearSurface<2, 2> =
        BilinearSurface::new(&QUICKSTART_X, &QUICKSTART_Y, &QUICKSTART_VALUES);

    pub fn quickstart(x: u16, y: u16) -> Result<i32, SurfaceError> {
        QUICKSTART.evaluate(x, y)
    }

    type Compensation = BilinearSurface<3, 3, UniformAxis<3, 0, 100>, UniformAxis<3, 0, 50>>;
    static COMP_VALUES: [[i32; 3]; 3] = [[0, 20, 40], [10, 30, 50], [20, 40, 60]];
    static COMPENSATION: Compensation =
        BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &COMP_VALUES);

    pub fn compensation(x: u16, y: u16) -> Result<i32, SurfaceError> {
        COMPENSATION.evaluate(x, y)
    }

    static MIXED_X: [u16; 17] = [
        0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300, 1_410, 1_500,
        1_600,
    ];
    static MIXED_X_INDEX: [u16; 8] = bucket_index(&MIXED_X);
    static MIXED_VALUES: [[i32; 17]; 9] = [[0; 17]; 9];
    type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
    static MIXED: Mixed = BilinearSurface::from_axes(
        BucketedAxis::new(&MIXED_X, &MIXED_X_INDEX),
        UniformAxis::new(),
        &MIXED_VALUES,
    );

    pub fn mixed(x: u16, y: u16) -> Result<i32, SurfaceError> {
        MIXED.evaluate(x, y)
    }
}

/// Every X/Y pairing of the four compile-time lookup strategies, declared as
/// statics over one equivalent axis pair.
///
/// A firmware selects strategies in the type, so a pairing exists only where it
/// is named. Declaring all sixteen here is what makes them compile — and, when
/// this consumer is built for the bare-metal targets with a core-only sysroot,
/// link without an allocator.
pub mod strategies {
    use ph_surfaces::{
        BilinearSurface, BinaryAxis, BucketedAxis, LinearAxis, SurfaceError, UniformAxis,
        bucket_index,
    };

    static X: [u16; 5] = [0, 25, 50, 75, 100];
    static X_INDEX: [u16; 4] = bucket_index(&X);
    static Y: [u16; 3] = [0, 10, 20];
    static Y_INDEX: [u16; 2] = bucket_index(&Y);
    static VALUES: [[i32; 5]; 3] = [
        [0, 25, 50, 75, 100],
        [10, 35, 60, 85, 110],
        [-20, 5, 30, 55, 80],
    ];

    type Lx = LinearAxis<5>;
    type Bx = BinaryAxis<5>;
    type Ux = UniformAxis<5, 0, 25>;
    type Kx = BucketedAxis<5, 4>;

    type Ly = LinearAxis<3>;
    type By = BinaryAxis<3>;
    type Uy = UniformAxis<3, 0, 10>;
    type Ky = BucketedAxis<3, 2>;

    macro_rules! pairing {
        ($name:ident, $xt:ty, $yt:ty, $x:expr, $y:expr) => {
            static $name: BilinearSurface<5, 3, $xt, $yt> =
                BilinearSurface::from_axes($x, $y, &VALUES);
        };
    }

    pairing!(LL, Lx, Ly, LinearAxis::new(&X), LinearAxis::new(&Y));
    pairing!(LB, Lx, By, LinearAxis::new(&X), BinaryAxis::new(&Y));
    pairing!(LU, Lx, Uy, LinearAxis::new(&X), UniformAxis::new());
    pairing!(
        LK,
        Lx,
        Ky,
        LinearAxis::new(&X),
        BucketedAxis::new(&Y, &Y_INDEX)
    );
    pairing!(BL, Bx, Ly, BinaryAxis::new(&X), LinearAxis::new(&Y));
    pairing!(BB, Bx, By, BinaryAxis::new(&X), BinaryAxis::new(&Y));
    pairing!(BU, Bx, Uy, BinaryAxis::new(&X), UniformAxis::new());
    pairing!(
        BK,
        Bx,
        Ky,
        BinaryAxis::new(&X),
        BucketedAxis::new(&Y, &Y_INDEX)
    );
    pairing!(UL, Ux, Ly, UniformAxis::new(), LinearAxis::new(&Y));
    pairing!(UB, Ux, By, UniformAxis::new(), BinaryAxis::new(&Y));
    pairing!(UU, Ux, Uy, UniformAxis::new(), UniformAxis::new());
    pairing!(
        UK,
        Ux,
        Ky,
        UniformAxis::new(),
        BucketedAxis::new(&Y, &Y_INDEX)
    );
    pairing!(
        KL,
        Kx,
        Ly,
        BucketedAxis::new(&X, &X_INDEX),
        LinearAxis::new(&Y)
    );
    pairing!(
        KB,
        Kx,
        By,
        BucketedAxis::new(&X, &X_INDEX),
        BinaryAxis::new(&Y)
    );
    pairing!(
        KU,
        Kx,
        Uy,
        BucketedAxis::new(&X, &X_INDEX),
        UniformAxis::new()
    );
    pairing!(
        KK,
        Kx,
        Ky,
        BucketedAxis::new(&X, &X_INDEX),
        BucketedAxis::new(&Y, &Y_INDEX)
    );

    /// Evaluates all sixteen pairings at the same point, in a fixed order.
    pub fn every_pairing(x: u16, y: u16) -> [Result<i32, SurfaceError>; 16] {
        [
            LL.evaluate(x, y),
            LB.evaluate(x, y),
            LU.evaluate(x, y),
            LK.evaluate(x, y),
            BL.evaluate(x, y),
            BB.evaluate(x, y),
            BU.evaluate(x, y),
            BK.evaluate(x, y),
            UL.evaluate(x, y),
            UB.evaluate(x, y),
            UU.evaluate(x, y),
            UK.evaluate(x, y),
            KL.evaluate(x, y),
            KB.evaluate(x, y),
            KU.evaluate(x, y),
            KK.evaluate(x, y),
        ]
    }

    /// Evaluates the default binary surface over the same tables.
    pub fn default_surface(x: u16, y: u16) -> Result<i32, SurfaceError> {
        static DEFAULT: BilinearSurface<5, 3> = BilinearSurface::new(&X, &Y, &VALUES);

        DEFAULT.evaluate(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::{correction, elevation};
    use ph_surfaces::SurfaceError;

    #[test]
    fn elevation_matches_the_documented_points() {
        assert_eq!(elevation(60, 90), Ok(130));
        assert_eq!(elevation(10, 20), Ok(-65));
        assert_eq!(elevation(75, 100), Ok(109));
        assert_eq!(elevation(140, 60), Ok(31));
        assert_eq!(elevation(u16::MAX, 0), Ok(-60));
        assert_eq!(
            elevation(500, 151),
            Err(SurfaceError::YAbove {
                coordinate: 151,
                bound: 150
            })
        );
    }

    #[test]
    fn every_strategy_pairing_agrees_with_the_default_surface() {
        use super::strategies;

        for x in [0u16, 1, 24, 25, 60, 99, 100] {
            for y in [0u16, 5, 10, 19, 20] {
                let expected = strategies::default_surface(x, y);
                for (index, actual) in strategies::every_pairing(x, y).iter().enumerate() {
                    assert_eq!(*actual, expected, "pairing {index} at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn cost_constants_on_the_default_binary_and_a_mixed_pairing() {
        use ph_surfaces::{BilinearSurface, BucketedAxis, UniformAxis};

        assert_eq!(BilinearSurface::<5, 4>::VALUE_BYTES, 80);
        assert_eq!(BilinearSurface::<5, 4>::PAYLOAD_BYTES, 98);
        assert_eq!(BilinearSurface::<5, 4>::SUCCESS_INTERPOLATIONS, 3);
        assert_eq!(BilinearSurface::<5, 4>::SUCCESS_GRID_READS, 4);

        type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
        assert_eq!(Mixed::VALUE_BYTES, 612);
        assert_eq!(Mixed::PAYLOAD_BYTES, 662);
        assert_eq!(Mixed::SUCCESS_INTERPOLATIONS, 3);
        assert_eq!(Mixed::SUCCESS_GRID_READS, 4);
    }

    #[test]
    fn correction_matches_the_documented_points() {
        assert_eq!(correction(47, 5), Ok(86));
        assert_eq!(correction(145, 100), Ok(-242));
        assert_eq!(correction(60, 40), Ok(-44));
        assert_eq!(correction(90, u16::MAX), Ok(-199));
        assert_eq!(
            correction(39, 500),
            Err(SurfaceError::XBelow {
                coordinate: 39,
                bound: 40
            })
        );
    }

    #[test]
    fn firmware_examples_match_the_documented_points_and_cost_figures() {
        use super::firmware;
        use ph_surfaces::{
            BilinearSurface, BucketedAxis, SurfaceError, UniformAxis, bucket_index,
            max_local_comparisons,
        };

        assert_eq!(firmware::quickstart(100, 10), Ok(0));
        assert_eq!(firmware::quickstart(125, 20), Ok(50));
        assert_eq!(
            firmware::quickstart(0, 20),
            Err(SurfaceError::XBelow {
                coordinate: 0,
                bound: 100
            })
        );

        assert_eq!(firmware::compensation(50, 25), Ok(15));
        assert_eq!(firmware::mixed(610, 400), Ok(0));

        type UniformPair = BilinearSurface<17, 9, UniformAxis<17, 0, 100>, UniformAxis<9, 0, 200>>;
        assert_eq!(UniformPair::VALUE_BYTES, 612);
        assert_eq!(UniformPair::PAYLOAD_BYTES, 612);

        type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
        assert_eq!(Mixed::VALUE_BYTES, 612);
        assert_eq!(Mixed::PAYLOAD_BYTES, 662);
        static X: [u16; 17] = [
            0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300, 1_410,
            1_500, 1_600,
        ];
        static X_INDEX: [u16; 8] = bucket_index(&X);
        assert_eq!(max_local_comparisons(&X, &X_INDEX), 3);
    }
}
