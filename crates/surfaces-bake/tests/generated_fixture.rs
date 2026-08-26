//! Compile the checked-in generated fixture against the runtime crate.
//!
//! The emit tests assert emitted text by substring; this test is the only
//! place the emitted source is actually compiled, proving it builds against
//! the public `ph_surfaces` API and evaluates like the runtime. It stays out
//! of the packaged crate on purpose: `include` in Cargo.toml omits `tests/`,
//! and the generated fixture does not ship, so packaged builds never
//! reference it.

// The included source provides `use ph_surfaces::BilinearSurface;` itself.
include!("../generated/rounding.rs");

#[test]
fn the_checked_in_fixture_compiles_and_matches_the_runtime() {
    assert_eq!(SURFACE.evaluate(0, 0), Ok(0));
    assert_eq!(SURFACE.evaluate(2, 0), Ok(1));
    assert_eq!(SURFACE.evaluate(0, 2), Ok(1));
    assert_eq!(SURFACE.evaluate(2, 2), Ok(2));
    // Runtime X-then-Y rounding at the off-knot sample; the S4 example.
    assert_eq!(SURFACE.evaluate(1, 1), Ok(2));
    assert_eq!(PAYLOAD_BYTES, BilinearSurface::<2, 2>::PAYLOAD_BYTES);
    assert_eq!(MAX_ERR_LSB, 1);
}
