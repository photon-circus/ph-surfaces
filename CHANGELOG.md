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
- Private allocation-free binary axis lookup in `src/lookup.rs`. One algorithm
  serves both axes: it brackets a coordinate to the adjacent lower and upper
  knots, and accepts both inclusive endpoints and every exact interior knot
  without changing the coordinate. The X-below, X-above, Y-below, and Y-above
  selections apply independently; an Error side reports the matching
  `SurfaceError` variant with the coordinate as supplied and the applicable
  bound, and a Clamp side normalizes to the nearest endpoint cell and never
  extrapolates. An in-domain lookup costs two endpoint comparisons plus exactly
  `ceil(log2(len))` probes, independent of the stored knots and of the
  coordinate; an out-of-domain coordinate costs one or two comparisons and never
  searches. The lookup is crate private and does not change the public API.
- Public `BilinearSurface::evaluate(&self, x: u16, y: u16) -> Result<i32,
  SurfaceError>` in `src/evaluate.rs`, composing the axis lookup and the scalar
  interpolation helper into the first public runtime capability. X is resolved
  before Y, so the X-side error is reported when both coordinates leave the
  domain on Error sides and a clamped X is still followed by a Y resolved under
  its own selections. The value is composed by interpolating along X on the
  lower-Y row, along X on the upper-Y row, and then interpolating those two
  already-rounded results along Y; that order is normative and observable,
  because a Y-then-X composition returns different values. Evaluation is
  deterministic, integer only, allocation free, and stateless. A successful
  evaluation performs exactly three scalar interpolations and four reads of the
  value grid; in-domain axis lookups are logarithmic, clamped lookups perform no
  search probes, and rejected coordinates return before interpolation or grid
  access, with an X rejection also skipping Y lookup. Evaluation never
  extrapolates or overflows for any surface this crate can define.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
