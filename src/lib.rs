//! Deterministic no-std, no-alloc integer surface mappings for embedded Rust.
//!
//! # Status
//!
//! **Lifecycle:** Incubating. **Distribution:** unpublished (`publish = false`),
//! version `0.1.0-incubating.1`. This crate exposes the validated static
//! representation [`BilinearSurface`] together with its boundary policy
//! vocabulary ([`Boundary`], [`BoundaryPolicy`]) and its out-of-domain outcome
//! type ([`SurfaceError`]). It does not yet expose an evaluator: axis lookup is
//! a private internal, and evaluation is a later issue.
//!
//! The accepted v0.1 destination is a static rectilinear `u16 × u16 → i32`
//! bilinear surface with deterministic X-then-Y interpolation and four
//! independent Error/Clamp boundary sides.
//!
//! # Runtime guarantees
//!
//! `#![no_std]` is unconditional. It is not relaxed by any feature. The
//! implementation is core-only: no allocator, no `std`, and no `unsafe`.
//!
//! # Scope
//!
//! This crate will own static multidimensional mapping mechanics: shape and
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
