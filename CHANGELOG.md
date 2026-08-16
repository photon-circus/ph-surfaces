# Changelog

## Unreleased

### Added

- Independent core-only `ph-surfaces` crate and repository floor at
  `0.1.0-incubating.1` (`publish = false`).
- Private `u16`-domain, `i32`-range scalar segment interpolation in
  `src/interp.rs`, using 64-bit integer arithmetic and rounding to nearest with
  exact half-way values away from zero. One local division helper is the sole
  implementation of that rounding policy. The helper is crate private and does
  not change the public API.
- An `integer only` check in `scripts/ci.sh` that fails if runtime code
  acquires a floating-point, allocator, `std`, or `ph-curves` path, and that
  asserts the crate-level `#![forbid(unsafe_code)]` is still in place.
- Public static representation `BilinearSurface<NX, NY>`, referencing
  `&'static` axes and a row-major `&'static [[i32; NX]; NY]` value grid. Its
  `const fn new` rejects fewer than two knots on either axis and any axis that
  is not strictly increasing, so an invalid definition fails to compile. A
  transposed grid is a type error, not a runtime error.
- Public `Boundary` and `BoundaryPolicy`, selecting Error or Clamp
  independently on the X-below, X-above, Y-below, and Y-above domain sides.
  Every side defaults to Error.
- Public `SurfaceError`, distinguishing the four domain sides and carrying the
  rejected coordinate and the applicable bound. Implements `Display` and
  `core::error::Error`.
- Evaluation is not implemented yet: there is no axis lookup and no
  `BilinearSurface::evaluate`.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
