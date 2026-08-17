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
- Black-box conformance suite in `tests/conformance/` for the public v0.1
  contract. It exercises `BilinearSurface::evaluate` and the public policy and
  error vocabulary only, with no dev-dependencies. Expected values come from an
  independent `i128` reference (linear axis scan, remainder-based
  ties-away-from-zero rounding, `i32::try_from` narrowing so every expected
  value is shown representable) or from hand computation in comments. It
  covers exact knots, a hand-computable 2x2 plane, nonuniform 3x3 grids,
  signed interiors, flat rows and columns, decreasing data, positive and
  negative half-way ties, sign reflection, all eight boundary side-by-policy
  cases, all four corner interactions with X-before-Y precedence, every one of
  the sixteen policies against every region, binary search against the linear
  oracle over nonuniform axes up to 1024 knots, axes containing `0` and
  `u16::MAX`, grids containing `i32::MIN` and `i32::MAX`, stateless
  determinism, and the retained locked X-then-Y fixture. Small declared
  domains are enumerated exhaustively; the full `u16 x u16` range is sampled
  under a stated rule with no exhaustive claim. Two device-neutral example maps
  (mixed-sign elevation, asymmetric process correction) show shape and
  rounding only. Four in-suite mutant oracles (Y-then-X, ties toward zero,
  Y-first precedence, extrapolation) are asserted to disagree with the
  accepted results on named points, and each mutation applied to `src/` fails
  the suite.
- The `integer only` check in `scripts/ci.sh` now also greps `tests/` for
  floating-point types and literals and for `ph-curves`, so the conformance
  suite cannot acquire a float or `ph-curves` oracle.
- Consolidated public v0.1 contract documentation in `README.md` and the
  crate-level rustdoc: representation and row-major `values[y][x]`
  orientation, compile-time constant validation, the four boundary policies
  and the four `SurfaceError` variants, X-before-Y error precedence,
  nearest/ties-away scalar rounding, the normative X-then-Y order with its
  locked fixture, the full-range `i64` no-overflow proof and why the public
  error surface has no overflow variant, statelessness, the prominent
  no-`ph-curves`-in-any-form statement, the supported verification targets,
  and the explicit v0.1 non-goals. Each contract item in the README is a
  doctest where it can be.
- Every code block in `README.md` is now compiled and run as a doctest via a
  `cfg(doctest)`-only module in `src/lib.rs`, so the packaged README cannot
  drift from the API. No runtime code and no public API item changed.
- Two unrelated device-neutral example maps (the conformance suite's
  `ELEVATION` and `CORRECTION` fixtures) documented as doctests in the README
  and crate rustdoc, with hand-computed points and a boundary policy each,
  and stated to prove generic mechanics only, not device or vendor accuracy.
- The `package build` check now also builds rustdoc with warnings denied and
  runs every doctest (README blocks included) from inside the unpacked
  `.crate`, and its downstream `#![no_std]` consumer now declares both example
  maps as statics, is tested on the host against the documented points, and
  is built for both embedded targets. `package list` additionally rejects
  `docs/` and `tests/` from the archive.
- `docs/v0.1-traceability.md`: the final traceability checklist mapping every
  acceptance claim of the v0.1 umbrella (#1) and of #9 to its implementation
  issue, tests, documentation section, and `scripts/ci.sh` gate, with the
  child-issue dispositions and the explicit not-proven / not-claimed list. It
  is repository material and is not packaged.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
