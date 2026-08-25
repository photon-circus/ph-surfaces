# Changelog

## Unreleased

### Added

- Host-side baker Rust emission in `ph-surfaces-bake`: `--emit-rust` writes
  deterministic static knot arrays, a row-major `values[y][x]` grid,
  `PAYLOAD_BYTES` for the selected pairing, and `MAX_ERR_LSB` to stdout.
  `--x-bucketed` / `--y-bucketed` reject a bucket count outside the runtime
  `1..=65_536` bound so the emitter does not print uncompilable
  `BucketedAxis` source; `emit_rust_with` returns the same closed
  `BakeError::InvalidBucketCount`. Packaged baker tests do not
  `include!` the unpackaged generated fixture.
  `cargo xtask generate` writes the baker-owned checked-in fixture; the
  `generated source` check re-renders it in memory and fails with
  `run cargo xtask generate` when it drifts. The bound is an i32 value LSB
  of sample deviation, not a device or accuracy claim. This is not a
  runtime API change.
- Host-side baker quantization in `ph-surfaces-bake`: fill each declared
  grid node from on-knot samples, apply the caller-stated scale with
  round-to-nearest (exact half-way away from zero; values just below a
  half stay on the nearer integer), measure deviation of
  the quantized table from every supplied sample in i32 value LSBs, and
  emit the maximum as `pub const MAX_ERR_LSB: i32`. For samples whose
  coordinates are exact `u16` values, the bound includes the runtime's
  rounded X-then-Y path, not only unrounded host bilinear. `MAX_ERR_LSB`
  is `ceil` of the exact rational residual (IEEE `f64` bit-patterns as
  dyadics, bilinear as an exact ratio of the `i32` grid on the host, not a
  host-`f64` lerp with ULP padding). `ceil` applies only to the finished
  residual. A finite residual whose `ceil` does not fit in `i32` is
  `BakeError::BoundOverflow`, not `NonFiniteDeviation`. Missing or
  ambiguous nodes, a non-invertible scale, an NX×NY product above
  1_048_576 cells, and `i32` overflow are closed `BakeError`s. On-knot
  lookup is binary search on the ordered knot lists. The operator RMS
  statistic scales before squaring so a representable tiny residual does
  not underflow to 0, and converting the residual to `f64` applies the
  binary exponent without first underflowing `2^exp` and applies any
  remaining negative exponent while the significand is still normal, so a
  significand just above `2^-1075` still becomes the smallest subnormal
  with only one subnormal rounding. The bound is an
  upper bound on deviation from the supplied samples, not a device or
  accuracy claim. This is not a runtime API change.
- Reviewed host crates on `ph-surfaces-bake` (`num-bigint`, `num-rational`,
  `num-traits`) for exact residual arithmetic. They stay off the runtime
  graph. This is not a runtime API change.
- Host-side baker ingest in `ph-surfaces-bake`: delimited sample points
  (X, Y, value as host `f64`), an explicit per-axis grid (knot list or
  uniform origin/step/count), and a caller-stated output scale stored at
  ingest. Grid validation matches the runtime constructors in the same
  vocabulary; samples outside the declared domain are reported;
  non-finite sample fields and scales are rejected; failures are a closed
  `BakeError` enum. No third-party parser. This is not a runtime API change.
- Host-side baker crate floor `ph-surfaces-bake` at `crates/surfaces-bake`:
  `[lib]` plus a thin CLI, reviewed host crates for exact residual
  arithmetic, a mechanically checked implementation-line budget, and a
  packaged-file allowlist
  checked independently of the runtime `package *` family. This is not a
  runtime API change. The runtime crate cannot reach the baker through any
  dependency kind, feature, or `cfg`.

### Changed

- Adopt a virtual Cargo workspace: the runtime crate lives in
  `crates/surfaces`, the verification gate in `xtask/`, and `publish_lock`
  classifies every workspace member. The packaged `ph-surfaces` file set is
  unchanged.

## 0.1.0 - 2026-08-23

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
  `AxisLookup::search` rejects an out-of-domain coordinate and
  `max_local_comparisons` rejects a malformed bucket table in every build
  profile, not only in debug.
- `Boundary` / `BoundaryPolicy`: Error or Clamp on each of the four domain
  sides, every side defaulting to Error. `SurfaceError` names the side and
  carries the supplied coordinate and the applicable bound. Clamp holds the
  nearest endpoint; nothing is extrapolated.
- `BilinearSurface::evaluate`: X before Y, then X on each Y row, then Y
  between those already-rounded results. Rounding is nearest, with exact
  half-way values away from zero. The X-side error wins when both coordinates
  leave Error sides. Evaluation is stateless and cannot overflow for any
  surface this crate can define.
- Public axis access: `BilinearSurface::x()` / `y()` return each axis with
  its strategy, so generic code bounded on `AxisLookup` (or `KnotArray` for
  the stored strategies) can read domain bounds, knots, comparison counts,
  and cost constants from any surface. The strategy-specific
  `x_knot`/`x_min`/`x_max` accessors remain the constant-context
  conveniences, and `UniformAxis` gains const `knot`/`last` so the
  descriptor arithmetic has one home.
- A documented panics-and-determinism contract: `evaluate` cannot panic for
  any constructible surface (a structural argument; compiler-retained bounds
  checks are visible in the committed instruction snapshots), results are
  bit-identical across host, ARM, and RISC-V, and floating point is excluded
  by disclosed policy — any future FPU fast path must be an off-by-default
  feature gate that leaves default-build results untouched.
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
  pass. The gate proves unconditional `no_std`, integer-only core runtime —
  including wide-integer confinement (64-bit arithmetic only inside the
  `src/interp.rs` kernel, 128-bit only in test oracles) — absence of
  `ph-curves`, a full-history secret scan, per-target clippy, packaged file
  set, doctests from the unpacked `.crate`, and a downstream `#![no_std]`
  consumer of all sixteen strategy pairings on host and both representative
  embedded targets.
- Committed per-architecture measurement artifacts: the code-size snapshot
  now includes the shared `ph_interp_kernel` size, and
  `cargo xtask asm --write` records the emitted instruction streams
  (`docs/asm-snapshot-<target>.txt`) for both embedded targets, relocations
  named, as informational review material.
