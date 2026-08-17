//! Deterministic no-std, no-alloc integer surface mappings for embedded Rust.
//!
//! # Status
//!
//! **Lifecycle:** Incubating. **Distribution:** unpublished (`publish = false`),
//! version `0.1.0-incubating.1`. This crate exposes the validated static
//! representation [`BilinearSurface`], its evaluator
//! [`BilinearSurface::evaluate`], the boundary policy vocabulary
//! ([`Boundary`], [`BoundaryPolicy`]), and the out-of-domain outcome type
//! ([`SurfaceError`]). Axis lookup and scalar interpolation remain private
//! internals.
//!
//! The accepted v0.1 destination is a static rectilinear `u16 × u16 → i32`
//! bilinear surface with deterministic X-then-Y interpolation and four
//! independent Error/Clamp boundary sides.
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
//! # Runtime guarantees
//!
//! `#![no_std]` is unconditional. It is not relaxed by any feature; the crate
//! declares none. The implementation is core-only: no allocator, no `std`, and
//! no `unsafe`. The package has no runtime, development, or build dependency,
//! and in particular no dependency of any kind on `ph-curves`.
//!
//! Those are mechanically checked by the repository's local gate rather than
//! merely asserted: the runtime is built with a nightly `-Z build-std=core`
//! core-only sysroot on ARM (`thumbv7em-none-eabi`) and RISC-V
//! (`riscv32imac-unknown-none-elf`), so an allocator reference cannot link,
//! and the packaged artifact is compiled by a downstream `#![no_std]` consumer.
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

#[cfg(test)]
mod tests {
    #[test]
    fn crate_links_on_core_only_types() {
        let none: Option<u8> = None;
        assert!(none.is_none());
    }
}
