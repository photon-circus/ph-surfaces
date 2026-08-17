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
  axis/grid shape mismatch is a type error, not a runtime error. For square
  grids, transposition preserves the type, so callers must still supply
  row-major `values[y][x]` orientation correctly.
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
  the default handle is three thin references plus the four-byte policy plus at
  most alignment padding. Strategy-specific tests account for zero axis
  references from Uniform, one from Linear/Binary, and two from Bucketed,
  without assuming a pointer width or field order.
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
- Consolidated documentation for the implemented binary-lookup baseline in
  `README.md` and the crate-level rustdoc: representation and row-major
  `values[y][x]`
  orientation, compile-time constant validation, the four boundary policies
  and the four `SurfaceError` variants, X-before-Y error precedence,
  nearest/ties-away scalar rounding, the normative X-then-Y order with its
  locked fixture, the full-range `i64` no-overflow proof and why the public
  error surface has no overflow variant, statelessness, the prominent
  no-`ph-curves`-in-any-form statement, the supported verification targets,
  and the explicit v0.1 non-goals. Each contract item in the README is a
  doctest where it can be.
- Every Rust code block in `README.md` is now compiled and run as a doctest via a
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
  is built for both embedded targets when they are installed; the check reports
  `SKIP`, not `PASS`, if either target is unavailable. `package list`
  additionally rejects `docs/` and `tests/` from the archive.
- `docs/v0.1-traceability.md`: an interim traceability checklist for the
  implemented binary baseline and the remaining #22, #26, and #27 acceptance
  work. It records #18 and #19 as implemented and #9 as closed. It remains
  repository material and is not packaged.
- Public compile-time per-axis lookup strategies in `src/axis/`: `LinearAxis<N>`
  (stored knots, bounded scan, at most `N - 1` comparisons), `BinaryAxis<N>`
  (stored knots, exactly `ceil(log2(N))` comparisons, and still the default),
  `UniformAxis<N, ORIGIN, STEP>` (a zero-sized descriptor, no stored knots, and
  no knot comparison at all), and `BucketedAxis<N, B>` (stored knots plus a
  `2*B`-byte static index that bounds a local scan). They implement the sealed
  `AxisLookup<N>` trait, and the three stored-knot strategies also implement the
  sealed `KnotArray<N>`. `bucket_index` builds a bucket table at compile time
  and `max_local_comparisons` states the exact local bound for a given axis;
  raising the bucket count to a multiple of itself splits buckets rather than
  moving their boundaries, so that bound never increases.
- `BilinearSurface` now carries two defaulted strategy type parameters,
  `BilinearSurface<NX, NY, X = BinaryAxis<NX>, Y = BinaryAxis<NY>>`, and a
  `const fn from_axes` that builds a surface from two already-validated axes.
  `BilinearSurface<NX, NY>` and `BilinearSurface::new` are unchanged in meaning,
  spelling, results, errors, and handle size; the existing conformance suite
  passes untouched. Selection is type-level, so no runtime discriminant or
  strategy branch exists. Each axis validates itself in its own `const fn`
  constructor, so an invalid uniform descriptor, a bucket index that disagrees
  with its knots, an axis of fewer than two knots, or a non-increasing axis
  fails to compile. New accessors `x_knot` and `y_knot` read a knot from any
  strategy; `x_axis` and `y_axis` remain available for the strategies that store
  one, and the endpoint accessors are no longer `const fn` because they dispatch
  through the axis type.
- Cross-strategy unit evidence in `src/axis/`: every strategy locates the same
  cell and evaluates the same value on equivalent axes, the four `SurfaceError`
  variants and X-before-Y precedence are invariant across pairings, clamping
  never extrapolates under any pairing, the locked X-then-Y fixture still
  distinguishes the two orders under all sixteen pairings, each strategy stays
  inside its declared search bound and declared stored bytes, and a finer nested
  bucket index never worsens the local bound.
- The `package build` consumer now also declares all sixteen X/Y strategy
  pairings as statics, asserts on the host that each agrees with the default
  binary surface, and is built for both embedded targets against a nightly
  core-only sysroot; the check reports `SKIP` if nightly with `rust-src` is
  unavailable. `package list` and the exact packaged file set now include
  `src/axis/`.
- Associated cost constants on `BilinearSurface`: `VALUE_BYTES`,
  `PAYLOAD_BYTES`, `HANDLE_BYTES` (target-dependent), `SUCCESS_INTERPOLATIONS`
  (3), and `SUCCESS_GRID_READS` (4). Default binary `PAYLOAD_BYTES` equals
  `2*NX + 2*NY + 4*NX*NY`. Host tests match `size_of` of the declared tables;
  Uniform/Uniform payload is only the grid.
- Embedded firmware usage guides (`docs/usage-guide.md`), an interpolation
  walkthrough of the production X-then-Y order
  (`docs/interpolation-walkthrough.md`), and a prescriptive strategy cookbook
  (`docs/choosing-a-strategy.md`). Five assertion-harness Cargo examples
  (`firmware_quickstart`, `uniform_sensor_compensation`,
  `mixed_calibration_map`, `fail_safe_boundaries`, `firmware_cost_budget`)
  mechanically assert the documented results and the three exact
  payload/work comparisons. Local CI runs every example, packages them, runs
  them from the unpacked `.crate`, and reuses the default, Uniform, and mixed
  fixtures in the downstream `#![no_std]` consumer.
- Black-box cross-strategy suite in `tests/conformance/strategies.rs`. Every
  applicable Linear/Binary/Uniform/Bucketed pairing is compared to a
  table-based `i128` linear-scan oracle (`evaluate_tables`); production
  locators are never the expected-value source. Lookup fixtures cover Linear
  and Bucketed on the existing axes and Uniform on an even-spaced axis.
  Locator mutants (off-by-one cell, Uniform origin/step off-by-one, Bucketed
  cluster vs tail) disagree on named points. Nested `bucket_index` /
  `max_local_comparisons` never increases the local bound when `B` is
  multiplied.
- Selection matrix and three exact worked storage/work doctests (default
  binary `ELEVATION` 5×4, tiny Linear×Linear 3×2, and a mixed
  Bucketed×Uniform 17×9 example whose index reduces the documented search
  bound) in the README and crate rustdoc, wired through the new constants.
- `scripts/measure-code-size.sh` (not packaged) records compiler-object
  `.text` totals for four named single-pairing consumers on ARM and RISC-V
  with the pinned 1.94.0 toolchain. It identifies normally mangled safe Rust
  symbols with `llvm-nm --demangle`; no exported unsafe attributes are
  generated. Output is committed as `docs/code-size-snapshot.txt` and labelled
  non-normative. The `code size snapshot` CI check diffs it and returns SKIP
  if a target or `llvm-tools-preview` is missing.
- The `package build` consumer asserts `VALUE_BYTES` / `PAYLOAD_BYTES` /
  `SUCCESS_*` on the default binary surface and one mixed pairing.
- Repository-specific contribution, conduct, and release procedures, including
  the fail-closed clean-artifact matrix and never-retarget/yank-and-replace
  policy; grouped monthly Cargo and GitHub Actions Dependabot configuration.

### Changed

- The canonical gate now includes release-profile tests and accepts a dated
  `NIGHTLY_TOOLCHAIN` override. `REQUIRE_NO_SKIPS=1` converts every skip to a
  failure, requires clean package provenance matching `HEAD`, and prints a
  twice-verified archive SHA-256. `SKIP_EMBEDDED=1` no longer suppresses either
  core-only proof.
- Removed Cargo's deprecated `authors` field so a future package does not copy
  a personal email into registry metadata. Historical Git identities are a
  separate public-exposure decision.
- Regenerated the non-normative single-pairing code-size snapshot after the
  checked public-search refactor. Binary and Uniform are unchanged; the named
  ARM/RISC-V Linear objects increase by 16/18 bytes and the mixed objects
  decrease by 30/62 bytes under the pinned recipe. These remain compiler-object
  `.text` observations, not total flash or a guarantee.

### Fixed

- `max_local_comparisons` now validates the complete supplied bucket index and
  its dimensions before subtraction, so malformed public input is rejected
  consistently instead of panicking only in debug and wrapping in release.
- Public `AxisLookup::search` now rejects out-of-domain coordinates in every
  build profile. Surface evaluation enters a private sealed in-domain search
  after its existing boundary checks, preserving exactly two endpoint
  comparisons plus each strategy's declared probe bound.
- Added a repository LF policy and a transition-safe code-size snapshot
  comparison so the documented Git Bash gate is portable across Windows
  `core.autocrlf` settings.

### Documentation

- Corrected the four-bucket coordinate example to `0, 251, 501, 751` and made
  the square-grid transposition warning explicit.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
