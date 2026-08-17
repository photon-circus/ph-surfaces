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
- Mechanical proofs of the runtime contract in `scripts/ci.sh`. Bare-metal
  targets are now built rather than checked, on ARM (`thumbv7em-none-eabi`)
  and RISC-V (`riscv32imac-unknown-none-elf`), and both are also built with a
  nightly `-Z build-std=core` core-only sysroot as the no-allocation proof. The
  `no ph-curves` guard rejects the name in the manifest text (normal, optional,
  target-specific, development, build, path, Git, `[patch]`, `[replace]`),
  in `Cargo.lock`, and in `cargo metadata --all-features`. The `no_std
  unconditional` guard additionally rejects a `[features]` table, a `cfg_attr`
  on `no_std`, and any feature-gated code in `src/`. A `package build` check
  builds the `.crate`, asserts its exact file set, and compiles a minimal
  downstream `#![no_std]` consumer against the unpacked artifact on the host
  and on both embedded targets. `CI_ONLY=<name>` runs a single check.
- `scripts/guard-selftest.sh` (run by `scripts/ci.sh` as `guards fire on
  mutation`): copies the tracked tree, applies a feature-conditional `no_std`,
  an allocator path, and a `ph-curves` dependency, and requires the matching
  guard — including the core-only build — to fail on the copy.
- Host tests asserting the documented storage figures: the referenced element
  payload equals `2*NX + 2*NY + 4*NX*NY` bytes for representative shapes, and
  the handle is three thin references plus the four-byte policy plus at most
  alignment padding, without assuming a pointer width or field order.
- Public resource and cost accounting in the crate docs and README: the exact
  payload formula stated separately from the target-dependent handle and never
  as total RAM, flash, binary, or linker cost; and the worst-case evaluation
  structure — two logarithmic axis searches of `2 + ceil(log2(len))`
  comparisons each and three scalar interpolations — stated as operation
  structure, not as a measured cycle or WCET figure.
- The manual hosted workflow now installs both embedded targets, carries a job
  timeout, and documents which checks it still skips.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
