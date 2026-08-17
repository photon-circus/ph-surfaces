//! Deterministic no-std, no-alloc integer surface mappings for embedded Rust.
//!
//! # Status
//!
//! **Lifecycle:** Incubating. **Distribution:** unpublished (`publish = false`),
//! version `0.1.0-incubating.1`. This crate exposes the validated static
//! representation [`BilinearSurface`], its evaluator
//! [`BilinearSurface::evaluate`], the boundary policy vocabulary
//! ([`Boundary`], [`BoundaryPolicy`]), the out-of-domain outcome type
//! ([`SurfaceError`]), and the four compile-time axis lookup strategies
//! ([`LinearAxis`], [`BinaryAxis`], [`UniformAxis`], [`BucketedAxis`]) behind
//! the sealed [`AxisLookup`] and [`KnotArray`] traits. Scalar interpolation
//! remains private. Cross-strategy conformance, the const cost API, the
//! selection matrix, and a labelled code-size snapshot (#19) have landed. The
//! final documentation/package gate (#9) remains pre-release work.
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
//! **Lookup strategies.** Each axis selects how it locates a coordinate, in the
//! type, and the two axes select independently. [`BinaryAxis`] is the default,
//! so `BilinearSurface<NX, NY>` and [`BilinearSurface::new`] mean the
//! binary-knotted surface they have always meant. [`LinearAxis`] scans a small
//! axis, [`UniformAxis`] describes evenly spaced knots by origin, step, and
//! count instead of storing them, and [`BucketedAxis`] adds a static bucket
//! index — built at compile time by [`bucket_index`] — that bounds the local
//! scan of a long irregular axis. A surface that names strategies is built with
//! [`BilinearSurface::from_axes`].
//!
//! [`AxisLookup`] and [`KnotArray`] are sealed: the four strategies above are
//! the only implementations, each validating its own invariants in a `const fn`
//! constructor. Selection is type-level, so there is no runtime discriminant
//! and no branch among strategies; a firmware that names one combination
//! compiles that one. Choose [`LinearAxis`] for a tiny axis when the minimum
//! auxiliary structure is what matters; [`BinaryAxis`] as the general default;
//! [`UniformAxis`] when knots are evenly spaced, so the knot arrays can be
//! dropped and location is constant work; [`BucketedAxis`] for a long
//! irregular axis when `2*B` extra index bytes buy a smaller local bound.
//! Every strategy locates the same cell, evaluates the same value, and reports
//! the same errors — only stored bytes and search work differ.
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
//! A surface whose two axes choose different lookup strategies. The X axis is
//! irregular, so it keeps its knots and buys a bounded local scan with an
//! eight-entry bucket index; the Y axis is evenly spaced, so it describes its
//! knots by origin and step and stores none of them. Naming strategies changes
//! stored bytes and search work and nothing else — the default all-binary
//! surface over the same tables answers identically:
//!
//! ```
//! use ph_surfaces::{BilinearSurface, BucketedAxis, UniformAxis, bucket_index};
//!
//! static X: [u16; 5] = [0, 1, 2, 40, 1_000];
//! static X_INDEX: [u16; 8] = bucket_index(&X);
//! static Y: [u16; 3] = [0, 50, 100];
//! static VALUES: [[i32; 5]; 3] = [
//!     [0, 10, 20, 400, 10_000],
//!     [-5, 5, 15, 395, 9_995],
//!     [-10, 0, 10, 390, 9_990],
//! ];
//!
//! static MIXED: BilinearSurface<5, 3, BucketedAxis<5, 8>, UniformAxis<3, 0, 50>> =
//!     BilinearSurface::from_axes(
//!         BucketedAxis::new(&X, &X_INDEX),
//!         UniformAxis::new(),
//!         &VALUES,
//!     );
//! static DEFAULT: BilinearSurface<5, 3> = BilinearSurface::new(&X, &Y, &VALUES);
//!
//! assert_eq!(MIXED.evaluate(2, 50), Ok(15)); // a declared knot
//! // Rows 5_200 and 5_195 at x = 520; midway on Y is 5_197.5, rounded away.
//! assert_eq!(MIXED.evaluate(520, 25), Ok(5_198));
//! assert_eq!(MIXED.evaluate(520, 25), DEFAULT.evaluate(520, 25));
//! assert_eq!(MIXED.y_knot(2), 100); // described, not stored
//! ```
//!
//! # Resource accounting
//!
//! A [`BilinearSurface<NX, NY>`](BilinearSurface) references static tables
//! whose element payload is exactly [`BilinearSurface::PAYLOAD_BYTES`]:
//!
//! ```text
//! X::KNOT_BYTES + X::INDEX_BYTES + Y::KNOT_BYTES + Y::INDEX_BYTES + VALUE_BYTES
//! ```
//!
//! with [`BilinearSurface::VALUE_BYTES`] equal to `4*NX*NY`. For the default
//! binary pairing that is `2*NX + 2*NY + 4*NX*NY` bytes. That figure is exact
//! and target-independent, and it is **only** the referenced element payload.
//! It is not total RAM, flash, binary, or linker cost: alignment, section
//! placement, code, and stack are outside it.
//!
//! Naming a strategy changes the two axis terms and nothing else. Each axis
//! term is stated exactly by its strategy: `2*N` knot bytes and no index for
//! [`LinearAxis`] and [`BinaryAxis`], nothing at all for [`UniformAxis`], and
//! `2*N` knot bytes plus `2*B` index bytes for
//! [`BucketedAxis<N, B>`](BucketedAxis). The same exclusions apply: these are
//! referenced element bytes, not a total memory cost.
//!
//! The handle itself is separate and target-dependent:
//! [`BilinearSurface::HANDLE_BYTES`] is `size_of` of the handle on the current
//! target. It always has the value-grid reference and four one-byte boundary
//! selections. A Uniform axis adds no reference, a Linear or Binary axis adds
//! one knot-array reference, and a Bucketed axis adds knot-array and
//! index-array references. The default binary/binary handle is therefore three
//! thin references plus the policy and alignment padding. For a fixed strategy
//! pairing it does not grow with `NX` or `NY`.
//!
//! Default binary `ELEVATION` 5×4: payload `10 + 8 + 80 = 98`, three
//! interpolations and four grid reads on success, in-domain searches
//! `2 + ceil(log2(5))` and `2 + ceil(log2(4))` comparisons:
//!
//! ```
//! use ph_surfaces::{AxisLookup, BilinearSurface, BinaryAxis};
//!
//! assert_eq!(BilinearSurface::<5, 4>::VALUE_BYTES, 80);
//! assert_eq!(BilinearSurface::<5, 4>::PAYLOAD_BYTES, 98);
//! assert_eq!(BilinearSurface::<5, 4>::SUCCESS_INTERPOLATIONS, 3);
//! assert_eq!(BilinearSurface::<5, 4>::SUCCESS_GRID_READS, 4);
//! assert_eq!(<BinaryAxis<5>>::MAX_SEARCH_COMPARISONS, 3);
//! assert_eq!(<BinaryAxis<4>>::MAX_SEARCH_COMPARISONS, 2);
//! assert_eq!(
//!     BilinearSurface::<5, 4>::HANDLE_BYTES,
//!     core::mem::size_of::<BilinearSurface<5, 4>>()
//! );
//! ```
//!
//! Tiny Linear×Linear 3×2: six X knot bytes, four Y knot bytes, 24 value
//! bytes, payload 34; searches at most `N - 1` knot comparisons per axis:
//!
//! ```
//! use ph_surfaces::{AxisLookup, BilinearSurface, LinearAxis};
//!
//! type Tiny = BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>>;
//! assert_eq!(Tiny::VALUE_BYTES, 24);
//! assert_eq!(Tiny::PAYLOAD_BYTES, 34);
//! assert_eq!(<LinearAxis<3>>::MAX_SEARCH_COMPARISONS, 2);
//! assert_eq!(<LinearAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
//! assert_eq!(Tiny::SUCCESS_INTERPOLATIONS, 3);
//! assert_eq!(Tiny::SUCCESS_GRID_READS, 4);
//! ```
//!
//! Mixed [`BucketedAxis<5, 8>`](BucketedAxis) × [`UniformAxis<3, 0, 50>`](UniformAxis):
//! X knots+index `10 + 16`, Y knots 0, grid 60, payload 86; Uniform search
//! comparisons 0:
//!
//! ```
//! use ph_surfaces::{AxisLookup, BilinearSurface, BucketedAxis, UniformAxis};
//!
//! type Mixed = BilinearSurface<5, 3, BucketedAxis<5, 8>, UniformAxis<3, 0, 50>>;
//! assert_eq!(<BucketedAxis<5, 8>>::KNOT_BYTES, 10);
//! assert_eq!(<BucketedAxis<5, 8>>::INDEX_BYTES, 16);
//! assert_eq!(<UniformAxis<3, 0, 50>>::KNOT_BYTES, 0);
//! assert_eq!(Mixed::VALUE_BYTES, 60);
//! assert_eq!(Mixed::PAYLOAD_BYTES, 86);
//! assert_eq!(<UniformAxis<3, 0, 50>>::MAX_SEARCH_COMPARISONS, 0);
//! assert_eq!(Mixed::SUCCESS_INTERPOLATIONS, 3);
//! assert_eq!(Mixed::SUCCESS_GRID_READS, 4);
//! ```
//!
//! # Evaluation cost
//!
//! [`BilinearSurface::evaluate`] performs, in the worst case, two axis
//! searches and [`BilinearSurface::SUCCESS_INTERPOLATIONS`] scalar
//! interpolations. Each in-domain axis search is two endpoint comparisons plus
//! the search work of that axis's strategy; a clamped coordinate costs one or
//! two comparisons and no probes (the endpoint path, not a search); a rejected
//! evaluation returns before any interpolation or
//! [`BilinearSurface::SUCCESS_GRID_READS`] grid reads, and a rejected X also
//! skips the Y search. Exactly four value-grid elements are read on success.
//! The grid is never scanned.
//!
//! The per-strategy search work, in knot comparisons, is
//! [`AxisLookup::MAX_SEARCH_COMPARISONS`]: exactly `ceil(log2(N))` for the
//! default [`BinaryAxis`], at most `N - 1` for [`LinearAxis`], none at all for
//! [`UniformAxis`] — which locates by one subtraction and one division — and,
//! for [`BucketedAxis`], one bucket read plus a local scan bounded by
//! [`max_local_comparisons`] for that axis's knots and index. Raising a bucket
//! count to a multiple of itself splits buckets rather than moving their
//! boundaries, so that figure never increases.
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

mod axis;
mod boundary;
mod error;
mod evaluate;
mod interp;
mod lookup;
mod surface;

pub use axis::{
    AxisLookup, BinaryAxis, BucketedAxis, KnotArray, LinearAxis, UniformAxis, bucket_index,
    max_local_comparisons,
};
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
