//! Deterministic no-std, no-alloc integer surface mappings for embedded Rust.
//!
//! # Status
//!
//! **Lifecycle:** Incubating. **Distribution:** unpublished (`publish = false`),
//! version `0.1.0-incubating.1`. This crate currently exposes no public mapping
//! API. The accepted v0.1 destination is a static rectilinear
//! `u16 × u16 → i32` bilinear surface; that representation and evaluator are
//! later issues, not this scaffold.
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

#[cfg(test)]
mod tests {
    #[test]
    fn crate_links_on_core_only_types() {
        let none: Option<u8> = None;
        assert!(none.is_none());
    }
}
