//! Deterministic no-std, no-alloc integer surface mappings for embedded Rust.
//!
//! # Status
//!
//! **Lifecycle:** Incubating. **Distribution:** unpublished (`publish = false`),
//! version `0.1.0-incubating.1`. This crate exposes the validated static
//! representation [`BilinearSurface`], its evaluator
//! [`BilinearSurface::evaluate`], the boundary policy vocabulary
//! ([`Boundary`], [`BoundaryPolicy`]), and the out-of-domain outcome type
//! ([`SurfaceError`]). Scalar interpolation remains private. This build exposes
//! the source-compatible binary lookup baseline; compile-time per-axis
//! strategies (#18), their proof (#19), and the final documentation/package
//! gate (#9) remain pre-release work.
//!
//! The accepted v0.1 destination is a static rectilinear `u16 × u16 → i32`
//! bilinear surface with deterministic X-then-Y interpolation and four
//! independent Error/Clamp boundary sides, with each axis selecting its lookup
//! strategy at compile time.
//!
//! # Evaluation contract
//!
//! [`BilinearSurface::evaluate`] resolves X before Y, so the X-side error wins
//! when both coordinates leave the domain on Error sides, and a clamped X is
//! still followed by a Y resolved under its own selections. It then
//! interpolates along X on the lower-Y row, along X on the upper-Y row, and
//! finally interpolates those two already-rounded results along Y.
//!
//! That order is normative rather than incidental: every step rounds to nearest
//! with exact half-way values away from zero, so a Y-then-X composition returns
//! different values. Evaluation is stateless, allocation-free, and integer
//! only.
//!
//! ```
//! use ph_surfaces::BilinearSurface;
//!
//! static AXIS: [u16; 2] = [0, 2];
//! static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
//! static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
//!
//! assert_eq!(SURFACE.evaluate(1, 1), Ok(1));
//! ```
//!
//! # Contract
//!
//! **Representation.** [`BilinearSurface<NX, NY>`](BilinearSurface) references
//! `&'static [u16; NX]` X knots, `&'static [u16; NY]` Y knots, and a row-major
//! `&'static [[i32; NX]; NY]` value grid addressed as `values[y][x]`. Swapping
//! unequal X/Y dimensions is a compile-time type error; a square transpose
//! preserves the type, so callers remain responsible for row-major orientation.
//! [`BilinearSurface::new`] is a
//! `const fn` that asserts at least two knots per axis and strict increase of
//! both axes, so an invalid `static` definition fails to compile. The handle
//! carries no units, provenance, or other metadata.
//!
//! **Boundaries.** [`Boundary`] is `Error` or `Clamp`. [`BoundaryPolicy`]
//! selects one of those independently for X-below, X-above, Y-below, and
//! Y-above; every side defaults to `Error`. [`SurfaceError`] has exactly four
//! variants, one per side, each carrying the supplied coordinate and the
//! applicable bound. `Clamp` substitutes the nearest endpoint knot; nothing is
//! ever extrapolated.
//!
//! **Precedence.** X is resolved before Y. When both coordinates leave the
//! domain on `Error` sides the X-side error is reported; when X clamps, Y is
//! still resolved under its own selections.
//!
//! **Rounding.** Each scalar segment computes the exact rational
//! `(y0 * (span - offset) + y1 * offset) / span` in `i64` and rounds to
//! nearest, with exact half-way values rounded away from zero. One private
//! helper implements that rule and every interpolated value passes through it.
//!
//! **Order.** Bilinear evaluation interpolates along X on the lower-Y row,
//! along X on the upper-Y row, and then along Y between those two
//! already-rounded values. Because every step rounds, that order is observable
//! and normative; see the locked fixture under [Evaluation
//! contract](#evaluation-contract) above.
//!
//! # No arithmetic-overflow variant
//!
//! [`SurfaceError`] has no overflow variant because none is reachable. Both
//! segment weights are nonnegative and sum to `span <= 65_535`, so the `i64`
//! numerator has magnitude below `2^31 * 65_535 < 2^47`. The rounded quotient
//! lies in the closed hull of the two endpoints, so it fits `i32`; the Y step
//! receives two such values and returns one from the hull of the four corners.
//! This holds for knots at `0` and `u16::MAX` and for grids containing
//! `i32::MIN` and `i32::MAX`, and the conformance suite asserts it on those
//! extremes against an `i128` reference.
//!
//! # Statelessness
//!
//! Evaluation is a pure function of the handle and the two coordinates. There
//! is no reset, warm-up, cache, clock, I/O, persistence, hardware, or
//! lifecycle behaviour, and evaluating mutates and allocates nothing.
//!
//! # Runtime guarantees and independence
//!
//! `#![no_std]` is unconditional. It is not relaxed by any feature; the crate
//! declares none. The implementation is core-only: no allocator, no `std`, and
//! no `unsafe`. The package has no runtime, development, or build dependency.
//!
//! In particular, **this crate has no dependency of any kind on `ph-curves`**:
//! not direct, transitive, optional, feature-gated, target-specific,
//! development, build, path, or Git. Its scalar arithmetic is a private helper
//! specified and verified in this crate. Shared arithmetic is a separate
//! post-v0.1 decision.
//!
//! Those are mechanically checked by the repository's local gate rather than
//! merely asserted: the runtime is built with a nightly `-Z build-std=core`
//! core-only sysroot on ARM (`thumbv7em-none-eabi`) and RISC-V
//! (`riscv32imac-unknown-none-elf`), so an allocator reference cannot link;
//! the manifest, lockfile, and `cargo metadata` are checked for the banned
//! name; and the packaged artifact's own doctests and a downstream `#![no_std]`
//! consumer are compiled from the unpacked package. Every other Rust target,
//! Xtensa included, is unproven and unclaimed.
//!
//! # Examples
//!
//! Two unrelated, device-neutral example maps. They demonstrate generic
//! mechanics only — nonuniform axes, mixed-sign values, a boundary policy, and
//! the rounding rule on hand-computable points — and make no claim about any
//! device, vendor, sensor, calibration, or measurement accuracy. The same
//! tables are the `ELEVATION` and `CORRECTION` fixtures of the conformance
//! suite.
//!
//! A mixed-sign elevation map holding its last column past the far X edge:
//!
//! ```
//! use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};
//!
//! static ELEVATION_X: [u16; 5] = [0, 25, 60, 100, 180];
//! static ELEVATION_Y: [u16; 4] = [0, 40, 90, 150];
//! static ELEVATION_VALUES: [[i32; 5]; 4] = [
//!     [-120, -35, 40, 15, -60],
//!     [-80, 10, 95, 60, -20],
//!     [-15, 55, 130, 88, 5],
//!     [-40, 20, 70, 110, 45],
//! ];
//! static ELEVATION: BilinearSurface<5, 4> =
//!     BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES)
//!         .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));
//!
//! assert_eq!(ELEVATION.evaluate(60, 90), Ok(130)); // a declared knot
//! assert_eq!(ELEVATION.evaluate(10, 20), Ok(-65)); // rows -86, -44; midway
//! assert_eq!(ELEVATION.evaluate(75, 100), Ok(109)); // rows 114, 85; 114 - 29*10/60
//! assert_eq!(ELEVATION.evaluate(140, 60), Ok(31)); // rows 20, 47; 20 + 27*20/50
//! assert_eq!(ELEVATION.evaluate(u16::MAX, 0), Ok(-60)); // X clamps to 180
//! assert_eq!(
//!     ELEVATION.evaluate(500, 151),
//!     Err(SurfaceError::YAbove { coordinate: 151, bound: 150 })
//! );
//! ```
//!
//! An asymmetric process-correction map holding its last load row above its
//! range:
//!
//! ```
//! use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};
//!
//! static CORRECTION_X: [u16; 4] = [40, 55, 90, 200];
//! static CORRECTION_Y: [u16; 5] = [0, 10, 25, 70, 120];
//! static CORRECTION_VALUES: [[i32; 4]; 5] = [
//!     [125, 80, -15, -140],
//!     [90, 41, -33, -170],
//!     [30, -7, -61, -205],
//!     [-48, -95, -150, -260],
//!     [-110, -142, -199, -333],
//! ];
//! static CORRECTION: BilinearSurface<4, 5> =
//!     BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES)
//!         .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));
//!
//! assert_eq!(CORRECTION.evaluate(47, 5), Ok(86)); // rows 104, 67; 85.5 -> 86
//! assert_eq!(CORRECTION.evaluate(145, 100), Ok(-242));
//! assert_eq!(CORRECTION.evaluate(60, 40), Ok(-44));
//! assert_eq!(CORRECTION.evaluate(90, u16::MAX), Ok(-199)); // Y clamps to 120
//! assert_eq!(
//!     CORRECTION.evaluate(39, 500),
//!     Err(SurfaceError::XBelow { coordinate: 39, bound: 40 })
//! );
//! ```
//!
//! # Resource accounting
//!
//! A [`BilinearSurface<NX, NY>`](BilinearSurface) references three static
//! tables whose element payload is exactly
//!
//! ```text
//! 2*NX + 2*NY + 4*NX*NY bytes
//! ```
//!
//! (`NX` X knots of `u16`, `NY` Y knots of `u16`, and `NX*NY` values of
//! `i32`). That figure is exact and target-independent, and it is **only** the
//! referenced element payload. It is not total RAM, flash, binary, or linker
//! cost: alignment, section placement, code, and stack are outside it.
//!
//! The handle itself is separate and target-dependent: three thin references
//! (one pointer width each), four one-byte boundary selections, and whatever
//! padding the target's alignment requires. It does not grow with `NX` or
//! `NY`.
//!
//! # Evaluation cost
//!
//! [`BilinearSurface::evaluate`] performs, in the worst case, two axis
//! searches and three scalar interpolations. Each in-domain axis search is two
//! endpoint comparisons plus exactly `ceil(log2(len))` probes of that axis; a
//! clamped coordinate costs one or two comparisons and no probes; a rejected
//! coordinate returns before any interpolation, and a rejected X also skips
//! the Y search. Exactly four value-grid elements are read on success. The
//! grid is never scanned.
//!
//! That is a statement of operation structure, derived from the
//! implementation and asserted by its tests. It is not a cycle count or a
//! WCET figure: no timing has been measured, and none is claimed.
//!
//! # Scope
//!
//! This crate owns static multidimensional mapping mechanics: shape and
//! invariant validation, axis location, explicit domain policies, deterministic
//! integer interpolation, and truthful resource accounting.
//!
//! It does not own hardware access, sensor configuration, sampling, clocks,
//! persistence, calibration discovery, fault or application policy, device
//! lifecycle, vendor catalogs, or total measurement accuracy.
//!
//! # Not in v0.1
//!
//! Explicitly outside this version: a dependency on `ph-curves` or extraction
//! of a shared arithmetic crate; inverse lookup or solving for either axis;
//! other dimensions, axis widths, or output types; scattered points, irregular
//! meshes, bicubic interpolation, extrapolation, or fitting; dynamic or
//! runtime-loaded grids, mutation, caching, allocation, `unsafe`, or floating
//! point; runtime metadata, units, or provenance; host generation or CLI
//! tooling; runtime-selectable strategies or runtime-generated indexes; and a
//! direct coordinate-to-cell LUT before a concrete consumer supplies its
//! coordinate domain and latency bound, measurements showing Bucketed misses
//! that bound on a named target/profile, an adequate static-data budget, and a
//! reproducible generation and validation plan.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::correctness)]
#![deny(
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core
)]

mod boundary;
mod error;
mod evaluate;
mod interp;
mod lookup;
mod surface;

pub use boundary::{Boundary, BoundaryPolicy};
pub use error::SurfaceError;
pub use surface::BilinearSurface;

/// Compiles every code block in the packaged `README.md` as a doctest, so the
/// README cannot drift from the API it documents. Present only under
/// `cfg(doctest)`; it adds nothing to the built crate or its rustdoc.
#[cfg(doctest)]
mod readme_doctests {
    #![doc = include_str!("../README.md")]
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_links_on_core_only_types() {
        let none: Option<u8> = None;
        assert!(none.is_none());
    }
}
