# Changelog

## Unreleased

Evaluate a static rectilinear `u16 × u16 → i32` surface on embedded firmware
with deterministic X-then-Y bilinear interpolation, four independent
Error/Clamp boundary sides, and a compile-time lookup strategy per axis. The
runtime is unconditional `no_std`, allocation-free, integer-only, and has no
dependency on `ph-curves`. There is no 1.0 compatibility promise.

### Added

- `BilinearSurface<NX, NY>`: a `const fn` handle over `&'static` X knots, Y
  knots, and a row-major `values[y][x]` grid. Fewer than two knots per axis or
  a non-increasing axis fails to compile. Unequal X/Y transposition is a type
  error; square grids still require the documented orientation.
- Four sealed per-axis lookup strategies, chosen independently in the type:
  `LinearAxis` (bounded scan), `BinaryAxis` (default, `ceil(log2(N))`
  probes), `UniformAxis` (origin, step, and count; no stored knots), and
  `BucketedAxis` (static index plus a bounded local scan).
  `BilinearSurface::from_axes` builds a mixed pairing. Every pairing locates
  the same cell, returns the same value, and reports the same error.
- `Boundary` / `BoundaryPolicy`: Error or Clamp on each of the four domain
  sides, every side defaulting to Error. `SurfaceError` names the side and
  carries the supplied coordinate and the applicable bound. Clamp holds the
  nearest endpoint; nothing is extrapolated.
- `BilinearSurface::evaluate`: X before Y, then X on each Y row, then Y
  between those already-rounded results. Rounding is nearest, with exact
  half-way values away from zero. The X-side error wins when both coordinates
  leave Error sides. Evaluation is stateless and cannot overflow for any
  surface this crate can define.
- Exact cost constants on the handle: `VALUE_BYTES`, `PAYLOAD_BYTES`,
  `HANDLE_BYTES`, `SUCCESS_INTERPOLATIONS` (3), and `SUCCESS_GRID_READS` (4).
  Default binary payload is `2*NX + 2*NY + 4*NX*NY` bytes. Those figures are
  referenced element payload and operation structure, not total RAM, flash,
  or a cycle/WCET claim.
- Firmware examples `firmware_quickstart`, `uniform_sensor_compensation`,
  `mixed_calibration_map`, `fail_safe_boundaries`, and
  `firmware_cost_budget`.
- Black-box conformance against an independent `i128` reference, including
  every applicable strategy pairing, the locked X-then-Y fixture, and the
  packaged `ELEVATION` and `CORRECTION` maps.
- Canonical verification entry point `cargo xtask ci`, reporting `PASS`,
  `FAIL`, and `SKIP` distinctly. Release evidence is
  `--profile release --nightly nightly-YYYY-MM-DD`. A skipped check is not a
  pass. The gate proves unconditional `no_std`, integer-only core runtime,
  absence of `ph-curves`, packaged file set, doctests from the unpacked
  `.crate`, and a downstream `#![no_std]` consumer of all sixteen strategy
  pairings on host and both representative embedded targets.

### Fixed

- `max_local_comparisons` validates the complete supplied bucket index before
  subtraction, so a malformed table is rejected in every build profile
  instead of panicking only in debug or wrapping in release.
- Public `AxisLookup::search` rejects out-of-domain coordinates in every
  build profile. Surface evaluation still performs exactly two endpoint
  comparisons plus each strategy's declared probe bound.

### Documentation

- The four-bucket coordinate example uses `0, 251, 501, 751`. Square-grid
  callers are warned that transposition preserves the type.
