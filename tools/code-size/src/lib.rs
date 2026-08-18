//! Throwaway measurement consumer. Not packaged. The wrappers retain normal
//! Rust symbol mangling; `llvm-nm --demangle` identifies them by item name.
#![no_std]

use ph_surfaces::BilinearSurface;
#[cfg(feature = "pairing-linear")]
use ph_surfaces::LinearAxis;
#[cfg(any(feature = "pairing-mixed", feature = "pairing-uniform"))]
use ph_surfaces::UniformAxis;
#[cfg(feature = "pairing-mixed")]
use ph_surfaces::{BucketedAxis, bucket_index};

#[cfg(feature = "pairing-binary")]
static BINARY_X: [u16; 5] = [0, 25, 60, 100, 180];
#[cfg(feature = "pairing-binary")]
static BINARY_Y: [u16; 4] = [0, 40, 90, 150];
#[cfg(feature = "pairing-binary")]
static BINARY_V: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
#[cfg(feature = "pairing-binary")]
static BINARY: BilinearSurface<5, 4> = BilinearSurface::new(&BINARY_X, &BINARY_Y, &BINARY_V);

#[cfg(feature = "pairing-linear")]
static LINEAR_X: [u16; 3] = [0, 5, 100];
#[cfg(feature = "pairing-linear")]
static LINEAR_Y: [u16; 2] = [0, 20];
#[cfg(feature = "pairing-linear")]
static LINEAR_V: [[i32; 3]; 2] = [[0, 1, 2], [3, 4, 5]];
#[cfg(feature = "pairing-linear")]
static LINEAR: BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>> = BilinearSurface::from_axes(
    LinearAxis::new(&LINEAR_X),
    LinearAxis::new(&LINEAR_Y),
    &LINEAR_V,
);

#[cfg(feature = "pairing-uniform")]
static UNIFORM_V: [[i32; 2]; 2] = [[0, 100], [1_000, 1_100]];
#[cfg(feature = "pairing-uniform")]
static UNIFORM: BilinearSurface<2, 2, UniformAxis<2, 0, 10>, UniformAxis<2, 0, 10>> =
    BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &UNIFORM_V);

#[cfg(feature = "pairing-mixed")]
static MIXED_X: [u16; 17] = [
    0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300, 1_410, 1_500, 1_600,
];
#[cfg(feature = "pairing-mixed")]
static MIXED_X_INDEX: [u16; 8] = bucket_index(&MIXED_X);
#[cfg(feature = "pairing-mixed")]
static MIXED_V: [[i32; 17]; 9] = mixed_values();
#[cfg(feature = "pairing-mixed")]
static MIXED: BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>> =
    BilinearSurface::from_axes(
        BucketedAxis::new(&MIXED_X, &MIXED_X_INDEX),
        UniformAxis::new(),
        &MIXED_V,
    );

#[cfg(feature = "pairing-mixed")]
const fn mixed_values() -> [[i32; 17]; 9] {
    let mut values = [[0; 17]; 9];
    let mut y = 0;
    while y < 9 {
        let mut x = 0;
        while x < 17 {
            values[y][x] = (x as i32) * 100 - (y as i32) * 37;
            x += 1;
        }
        y += 1;
    }
    values
}

fn value(result: Result<i32, ph_surfaces::SurfaceError>) -> i32 {
    match result {
        Ok(v) => v,
        Err(_) => 0,
    }
}

#[cfg(feature = "pairing-binary")]
#[inline(never)]
pub extern "C" fn ph_eval_binary(x: u16, y: u16) -> i32 {
    value(BINARY.evaluate(x, y))
}

#[cfg(feature = "pairing-linear")]
#[inline(never)]
pub extern "C" fn ph_eval_linear(x: u16, y: u16) -> i32 {
    value(LINEAR.evaluate(x, y))
}

#[cfg(feature = "pairing-uniform")]
#[inline(never)]
pub extern "C" fn ph_eval_uniform(x: u16, y: u16) -> i32 {
    value(UNIFORM.evaluate(x, y))
}

#[cfg(feature = "pairing-mixed")]
#[inline(never)]
pub extern "C" fn ph_eval_mixed(x: u16, y: u16) -> i32 {
    value(MIXED.evaluate(x, y))
}
