//! Black-box conformance suite for the public v0.1 contract (issue #7).
//!
//! Every test here goes through `ph_surfaces::{BilinearSurface, Boundary,
//! BoundaryPolicy, SurfaceError}` and the public accessors only. Nothing
//! imports a private module, and the crate's own unit tests are not repeated
//! here: this suite exists so the contract has evidence that does not share
//! control flow with the implementation.
//!
//! Expected values come from `reference.rs`, an independent `i128`
//! implementation with a linear axis scan of fixture knot arrays and
//! remainder-based rounding, or from hand computation shown in comments.
//! `ph-curves` is not an oracle and the `integer only` and `no ph-curves`
//! checks in `cargo xtask ci` fail if this directory names it or a float.
//! `strategies.rs` extends that evidence across every
//! applicable Linear/Binary/Uniform/Bucketed pairing.
//!
//! # Exhaustive vs sampled
//!
//! Where a fixture's declared Cartesian domain is small (tens of thousands of
//! points), `reference::assert_exhaustive_domain` evaluates every point. Where
//! the domain is the full `u16 x u16` range, tests use `reference::sample_axis`
//! and say so; no exhaustive claim is made for those.
//!
//! # Mutation evidence
//!
//! `reference.rs` also carries four deliberately wrong "mutant" oracles —
//! Y-then-X composition, ties toward zero, Y-before-X precedence, and
//! extrapolation. `order.rs` and `boundary.rs` assert that named fixture points
//! *disagree* with each mutant, so the suite is shown to detect each mistake
//! rather than merely to agree with the implementation.

mod boundary;
mod determinism;
mod examples;
mod fixtures;
mod lookup;
mod order;
mod range;
mod reference;
mod scalar;
mod strategies;
