# ph-surfaces

Deterministic `no_std`, no-alloc integer surface mappings for embedded Rust.

[![Lifecycle: Incubating](https://img.shields.io/badge/lifecycle-incubating-orange.svg)](https://github.com/photon-circus/.github/blob/main/REPOSITORY_STANDARDS.md#31-lifecycle-values)
[![MSRV](https://img.shields.io/badge/MSRV-1.94.0-blue.svg)](Cargo.toml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **Lifecycle:** Incubating — the responsibility is bounded and intended to
> become supported. Compatibility follows the documented version and release
> policy, not lifecycle alone.
> **Distribution:** Unpublished. `publish = false`. Version `0.1.0-incubating.1`.
> **Domain:** Libraries.

This repository is a private Incubating Libraries project. It currently exposes
the validated surface representation, its deterministic X-then-Y evaluator, its
boundary and error vocabulary, the four compile-time per-axis lookup
strategies, and the const cost API. Cross-strategy conformance, the selection
matrix, and a labelled code-size snapshot (#19) have landed. The final
documentation/package gate (#9) is closed, and the embedded usage guides,
strategy cookbook, and runnable firmware examples (#22) have landed. There is
no crates.io publication and no docs.rs page. Publishing, tagging, and a
stable 1.0 promise remain separate maintainer decisions.

## What this is

A reusable math crate for evaluating static rectilinear two-dimensional integer
surfaces on embedded firmware. The accepted v0.1 destination is:

> Evaluate a static rectilinear two-dimensional `u16 × u16 → i32` surface with
> deterministic X-then-Y bilinear interpolation, four independent Error/Clamp
> boundary sides, no allocation, no floating point at runtime, and an explicit
> compile-time choice of lookup strategy for each axis.

`BilinearSurface::evaluate` implements that contract, and binary lookup remains
the default on both axes. A firmware compensation table is three `static`
arrays and one `static` handle — no allocator, no warm-up, no cache. Coordinates
are already quantized to `u16` and values to `i32` by the application; the
surface stores neither units nor provenance.

```rust
use ph_surfaces::{BilinearSurface, SurfaceError};

// Operating codes and a signed correction. Invented, device-neutral numbers.
static X: [u16; 2] = [100, 200];
static Y: [u16; 2] = [10, 30];
static VALUES: [[i32; 2]; 2] = [
    [0, 100],  // Y = 10
    [40, 180], // Y = 30
];

static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    assert_eq!(SURFACE.evaluate(100, 10), Ok(0)); // a declared knot
    assert_eq!(SURFACE.evaluate(125, 20), Ok(50)); // interior point; see the walkthrough
    assert_eq!(
        SURFACE.evaluate(0, 20),
        Err(SurfaceError::XBelow {
            coordinate: 0,
            bound: 100
        })
    );
}
```

Every Rust code block in this README is compiled and run as a doctest of the
packaged crate, so the README cannot drift from the API it describes.

## Start here

Task-oriented firmware guidance lives next to this README, not inside the
normative contract below.

1. **[Usage guide](docs/usage-guide.md)** — lay out axes as `values[y][x]`,
   declare a static Binary surface, name all four boundary sides, and place
   payload / handle / work figures in the right budget.
2. **One evaluation.** The query `(125, 20)` above sits in the cell
   `X ∈ [100, 200]`, `Y ∈ [10, 30]`. X interpolates on each Y row, each step
   rounds to nearest with ties away from zero, then Y interpolates those two
   already-rounded results: `25`, then `75`, then `50`. An X-side `Error`
   short-circuits before Y. The arithmetic is walked in
   **[the interpolation walkthrough](docs/interpolation-walkthrough.md)**.
3. **Choose a strategy** independently on each axis. Changing a strategy cannot
   change a value, an error, rounding, order, or boundary behaviour.

   | Situation | Starting choice | Then verify |
   | --- | --- | --- |
   | Unsure, or a general irregular axis | `BinaryAxis` (default) | its exact comparison bound is acceptable |
   | Knots are an exact arithmetic progression | `UniformAxis` | dropped knot storage is valuable; measure division on the target if timing matters |
   | Axis is very small | compare `LinearAxis` with `BinaryAxis` | generated target code and measured timing — no universal knot-count threshold |
   | Irregular axis needs a smaller proven local bound | `BucketedAxis` | `max_local_comparisons` improves enough to justify `2*B` index bytes |

   The cookbook, including Bucketed index tuning, is
   **[choosing a strategy](docs/choosing-a-strategy.md)**.
4. **Runnable examples** (host `main` is an assertion harness; the tables are
   `static` and `core`-only):
   `firmware_quickstart`, `uniform_sensor_compensation`,
   `mixed_calibration_map`, `fail_safe_boundaries`, `firmware_cost_budget`.

   ```sh
   cargo run --example firmware_quickstart
   ```

## Independence from `ph-curves`

**This crate has no dependency on `ph-curves` in any form** — not direct,
transitive, optional, feature-gated, target-specific, development, build,
path, or Git. Its `[dependencies]`, `[dev-dependencies]`, and
`[build-dependencies]` tables are empty. The scalar arithmetic it needs (one
signed segment interpolation with one rounding rule) is a private helper in
`src/interp.rs`, specified in this repository and verified locally against an
independent integer reference. That is a v0.1 decision, not an accident: shared
arithmetic can be reconsidered only after shipped duplication provides evidence
for a neutral common crate, in a separate post-v0.1 proposal.

The gate proves the absence rather than asserting it. `scripts/ci.sh` rejects
the name in the manifest text (every dependency kind, `[patch]`, `[replace]`),
in `Cargo.lock`, and in `cargo metadata --all-features`; `deny.toml` bans it as
a fourth layer; the downstream consumer's fresh lockfile may name only two
packages; and `scripts/guard-selftest.sh` shows the guard fires when a
`ph-curves` dependency is injected into a copy of the tree.

## Contract

This section is the consumer-facing statement of the implemented contract. Each
item below is implemented and tested by the
black-box suite in
`tests/conformance/`, and mapped to its evidence in
[`docs/v0.1-traceability.md`](docs/v0.1-traceability.md).

### Representation

- The public concrete type is
  `BilinearSurface<const NX: usize, const NY: usize, X = BinaryAxis<NX>, Y = BinaryAxis<NY>>`.
  The two strategy parameters default to binary lookup, so `BilinearSurface<NX, NY>`
  is the binary-knotted surface it has always been.
- It references `&'static [u16; NX]` (X knots), `&'static [u16; NY]` (Y
  knots), and a row-major `&'static [[i32; NX]; NY]` value grid. Y selects the
  row and X selects the column: a value is addressed as **`values[y][x]`**.
- Because the grid type is `[[i32; NX]; NY]`, swapping unequal X/Y dimensions
  is a **compile-time type error**, not a runtime error. For a square surface,
  transposition preserves the type, so the caller must still supply the
  documented row-major `values[y][x]` orientation. There is no reachable
  runtime dimension-mismatch outcome.
- `BilinearSurface::new` is a `const fn`. It asserts at least two knots on each
  axis and strict increase of both axes. In a `static` or `const` definition
  those assertions run at compile time, so an invalid definition **fails to
  compile**. The rustdoc on `BilinearSurface::new` carries `compile_fail`
  doctests for each rejected shape.
- The handle stores no units, provenance, achieved-error claim, host report, or
  other generated metadata: for the default surface, three references and four
  one-byte boundary selections.

### Per-axis lookup strategies

Each axis chooses **in the type** how it locates a coordinate, and the two axes
choose independently. There is no runtime discriminant and no branch among
strategies: a firmware that names one combination compiles that one.

| Strategy | Stored per axis | Search work, in knot comparisons | Choose when |
| --- | --- | --- | --- |
| `LinearAxis<N>` | `2*N` knot bytes | bounded scan, at most `N - 1` | tiny axis; minimum auxiliary structure |
| `BinaryAxis<N>` (default) | `2*N` knot bytes | exactly `ceil(log2(N))` | the general default |
| `UniformAxis<N, ORIGIN, STEP>` | nothing | none: one subtraction, one division | even spacing; drop knot arrays; constant location |
| `BucketedAxis<N, B>` | `2*N` knot bytes plus `2*B` index bytes | one bucket read plus a local scan bounded by `max_local_comparisons` | irregular axis; extra index bytes for a smaller local bound |

- `AxisLookup` and `KnotArray` are **sealed**. Those four types are the only
  implementations, and each validates its own invariants in a `const fn`
  constructor, so an invalid axis fails to compile: fewer than two knots, a
  non-increasing knot array, a zero or unrepresentable uniform step, or a bucket
  index that does not match its knots.
- A `BucketedAxis` index is generated at compile time by `bucket_index` and
  re-derived by the constructor. Nothing is built, cached, or mutated at
  runtime.
- Every strategy locates the same cell, evaluates the same value, and reports
  the same error. Only stored bytes and search work differ; rounding,
  composition order, boundary semantics, and error variants are unchanged.

```rust
use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, BucketedAxis, UniformAxis,
    bucket_index, max_local_comparisons,
};

static X: [u16; 17] = [
    0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205,
    1_300, 1_410, 1_500, 1_600,
]; // irregular: keeps its knots
static X_INDEX: [u16; 8] = bucket_index(&X);
static Y: [u16; 9] = [0, 200, 400, 600, 800, 1_000, 1_200, 1_400, 1_600];
static VALUES: [[i32; 17]; 9] = [[0; 17]; 9];

static MIXED: BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>> =
    BilinearSurface::from_axes(BucketedAxis::new(&X, &X_INDEX), UniformAxis::new(), &VALUES);
static DEFAULT: BilinearSurface<17, 9> = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    assert_eq!(MIXED.evaluate(610, 400), DEFAULT.evaluate(610, 400));
    assert_eq!(MIXED.y_knot(8), 1_600); // described, not stored
    assert_eq!(max_local_comparisons(&X, &X_INDEX), 3);
    assert_eq!(<BinaryAxis<17>>::MAX_SEARCH_COMPARISONS, 5);
}
```

### Boundary policies and errors

- `Boundary` is the whole v0.1 vocabulary: `Error` or `Clamp`.
- `BoundaryPolicy` names four independent sides — X-below, X-above, Y-below,
  Y-above — and **every side defaults to `Error`**. `BoundaryPolicy::new()`
  with `with_x_below` / `with_x_above` / `with_y_below` / `with_y_above` is
  const-usable, so a policy is part of a `static` definition.
- `SurfaceError` has exactly four variants — `XBelow`, `XAbove`, `YBelow`,
  `YAbove` — each carrying the `coordinate` as supplied and the applicable
  `bound` (the first knot for below, the last knot for above). It implements
  `Display` and `core::error::Error` and is deliberately not
  `#[non_exhaustive]`.
- `Clamp` substitutes the nearest declared endpoint coordinate and evaluates
  the boundary row or column. **Extrapolation is never performed** under either
  selection: a clamped result is a value inside the hull of the stored values.

### Precedence: X before Y

Coordinates are resolved X first, then Y. When both axes are outside `Error`
sides, the **X error wins**. If X clamps, Y is still evaluated under its own
two selections, so a clamped X can be followed by a Y error.

```rust
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static X: [u16; 2] = [0, 10];
static Y: [u16; 2] = [0, 10];
static VALUES: [[i32; 2]; 2] = [[0, 100], [200, 300]];

static STRICT: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
static CLAMP_X_ABOVE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES)
    .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));

fn main() {
    // Both out of domain on Error sides: the X-side error is the one reported.
    assert_eq!(
        STRICT.evaluate(11, 11),
        Err(SurfaceError::XAbove { coordinate: 11, bound: 10 })
    );
    // X clamps to 10 and evaluates the boundary column; nothing is extrapolated.
    assert_eq!(CLAMP_X_ABOVE.evaluate(4_000, 0), Ok(100));
    // X clamped, but Y is still resolved under its own (Error) side.
    assert_eq!(
        CLAMP_X_ABOVE.evaluate(4_000, 11),
        Err(SurfaceError::YAbove { coordinate: 11, bound: 10 })
    );
}
```

### Scalar rounding

Each scalar segment computes the exact signed rational
`(y0 * (span - offset) + y1 * offset) / span` in `i64` arithmetic, where
`span = t1 - t0` and `offset = t - t0`. The division **rounds to nearest, and an
exact half-way value rounds away from zero**. There is one rounding helper in
the crate and every interpolated value goes through it.

```rust
use ph_surfaces::BilinearSurface;

static AXIS: [u16; 2] = [0, 2];
static VALUES: [[i32; 2]; 2] = [[0, 1], [0, -1]];
static TIES: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);

fn main() {
    assert_eq!(TIES.evaluate(1, 0), Ok(1)); // +0.5 rounds away from zero to 1
    assert_eq!(TIES.evaluate(1, 2), Ok(-1)); // -0.5 rounds away from zero to -1
}
```

### Normative X-then-Y bilinear order

Bilinear evaluation always interpolates along X on the lower-Y row, along X on
the upper-Y row, and then interpolates those two **already-rounded** values
along Y. Because every step rounds, X-then-Y and Y-then-X are observably
different functions; the crate fixes X-then-Y and makes it part of the
contract. The locked fixture:

```rust
use ph_surfaces::BilinearSurface;

static AXIS: [u16; 2] = [0, 2];
static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);

fn main() {
    // X on the lower row: 0. X on the upper row: (1 + 3) / 2 = 2.
    // Y between them: (0 + 2) / 2 = 1. Y-then-X would return 2.
    assert_eq!(SURFACE.evaluate(1, 1), Ok(1));
}
```

### No arithmetic-overflow variant

The public v0.1 error surface has no overflow variant because none is
reachable for any surface this crate can define. Both segment weights,
`span - offset` and `offset`, are nonnegative and sum to `span ≤ 65_535`, so
the `i64` numerator `y0 * (span - offset) + y1 * offset` has magnitude at most
`2^31 * 65_535 < 2^47`, far inside `i64`. The rounded quotient lies in the
closed hull of `y0` and `y1`, so each scalar result fits `i32`. The Y step then
receives two `i32` values from the hull of the four corner values and returns
one from the same hull. This holds for the full `u16` axis range, including
knots at `0` and `65_535`, and for grids containing `i32::MIN` and `i32::MAX`;
the conformance suite asserts it on those extremes against an `i128`
reference.

### Stateless

Evaluation is a pure function of the handle and the two coordinates. The
primitive has no reset, warm-up, cache, clock, I/O, persistence, hardware, or
lifecycle semantics. The same handle and the same coordinates always produce
the same result, and evaluating never mutates or allocates anything.

## Examples

The firmware-first Cargo examples listed under [Start here](#start-here) are
the teaching path: static compensation, derating, and calibration maps, plus
an exact resource-budget comparison. They make no vendor, sensor, accuracy, or
safety claim.

The two maps below remain the packaged `ELEVATION` and `CORRECTION` fixtures.
They demonstrate nonuniform axes, mixed-sign values, a boundary policy, and
the rounding rule on hand-computable points. Every declared point is checked
against the independent reference in `tests/conformance/`, and they are two of
the surfaces the packaged downstream `no_std` consumer declares and evaluates.

A mixed-sign elevation map over unevenly spaced plan-view positions, holding
the last column past the far X edge:

```rust
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static ELEVATION_X: [u16; 5] = [0, 25, 60, 100, 180];
static ELEVATION_Y: [u16; 4] = [0, 40, 90, 150];
static ELEVATION_VALUES: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
static ELEVATION: BilinearSurface<5, 4> =
    BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES)
        .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));

fn main() {
    // A declared knot returns its stored height exactly.
    assert_eq!(ELEVATION.evaluate(60, 90), Ok(130));
    // (10, 20): rows give -86 and -44; midway along Y: -65.
    assert_eq!(ELEVATION.evaluate(10, 20), Ok(-65));
    // (75, 100): rows give 114.25 -> 114 and 85; then 114 - 29 * 10 / 60 -> 109.
    assert_eq!(ELEVATION.evaluate(75, 100), Ok(109));
    // (140, 60): rows give 20 and 46.5 -> 47; then 20 + 27 * 20 / 50 -> 31.
    assert_eq!(ELEVATION.evaluate(140, 60), Ok(31));
    // Past the far X edge the last column is held; Y still errors on its side.
    assert_eq!(ELEVATION.evaluate(u16::MAX, 0), Ok(-60));
    assert_eq!(
        ELEVATION.evaluate(500, 151),
        Err(SurfaceError::YAbove { coordinate: 151, bound: 150 })
    );
}
```

An asymmetric process-correction map — X a setpoint code, Y a load code,
values a signed correction in milli-units — holding the last load row above
its range:

```rust
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static CORRECTION_X: [u16; 4] = [40, 55, 90, 200];
static CORRECTION_Y: [u16; 5] = [0, 10, 25, 70, 120];
static CORRECTION_VALUES: [[i32; 4]; 5] = [
    [125, 80, -15, -140],
    [90, 41, -33, -170],
    [30, -7, -61, -205],
    [-48, -95, -150, -260],
    [-110, -142, -199, -333],
];
static CORRECTION: BilinearSurface<4, 5> =
    BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES)
        .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));

fn main() {
    // (47, 5): rows give 104 and 67; (104 + 67) / 2 = 85.5, an exact tie -> 86.
    assert_eq!(CORRECTION.evaluate(47, 5), Ok(86));
    // (145, 100): rows give -205 and -266; then -205 - 61 * 30 / 50 -> -242.
    assert_eq!(CORRECTION.evaluate(145, 100), Ok(-242));
    // (60, 40): rows give -15 and -103; then -15 - 88 * 15 / 45 -> -44.
    assert_eq!(CORRECTION.evaluate(60, 40), Ok(-44));
    // Loads above the table hold the last row; setpoints outside are rejected.
    assert_eq!(CORRECTION.evaluate(90, u16::MAX), Ok(-199));
    assert_eq!(
        CORRECTION.evaluate(39, 500),
        Err(SurfaceError::XBelow { coordinate: 39, bound: 40 })
    );
}
```

## What it is for

Firmware that needs a device-neutral, allocation-free mapping from two `u16`
axes onto an `i32` value — for example multidimensional compensation — without
taking a dependency on `ph-curves` or pulling in host tooling.

## What state it is in

Incubating and unpublished. The binary-lookup baseline, its conformance suite,
mechanical dependency and embedded proofs, examples, package checks, the
compile-time per-axis Linear, Binary, Uniform, and Bucketed strategies (#18),
cross-strategy conformance with a const cost API, selection matrix, and
labelled code-size snapshot (#19), the documentation and package-readiness
gate (#9), and the embedded usage guides, strategy cookbook, and runnable
firmware examples (#22) are implemented. The
[traceability checklist](docs/v0.1-traceability.md) records the evidence.
Publishing, tagging, and stable 1.0 compatibility remain separate maintainer
decisions; `publish = false` stays until then.

## Responsibility

`ph-surfaces` owns static multidimensional mapping mechanics: shape and
invariant validation, axis location, explicit domain policies, deterministic
integer interpolation, and truthful resource and evidence accounting.

## Out of scope

It does not own hardware access, sensor configuration, sampling, clocks,
persistence, calibration discovery, fault or application policy, device
lifecycle, vendor catalogs, or total measurement accuracy.

v0.1 explicitly does not include:

- A dependency on `ph-curves` or extraction of a shared arithmetic crate
- Inverse lookup or solving for either axis
- Arbitrary N-dimensional tensors, signed or wider axes, or generic output
  types
- Scattered points, triangulation, irregular meshes, bicubic interpolation,
  extrapolation, or adaptive fitting
- Dynamic or runtime-loaded grids, runtime mutation, caching, allocation,
  unsafe code, or floating point
- Runtime semantic metadata, units, provenance, or generated error reports
- Host generation, CLI tooling, formula ingestion, or numerical fitting
- Runtime-selectable strategies, runtime-generated indexes, or a direct
  coordinate-to-cell LUT. A direct LUT remains deferred unless a concrete
  firmware consumer supplies a coordinate domain and latency/jitter bound,
  measurements showing Bucketed lookup misses it on a named target/profile, a
  static-data budget, and a reproducible generation and validation plan.
- Device-specific equations, source catalogs, filtering, fusion, scheduling,
  buses, GPIO, async, or storage

## Constraints

- Unconditional `#![no_std]`; core-only runtime; `unsafe` is forbidden
- No `[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]`, and
  none of those tables may name `ph-curves` later either
- MSRV and toolchain pin: Rust 1.94.0, edition 2024
- Version `0.1.0-incubating.1` with `publish = false` until a separate release
  decision

## Resource accounting and cost

**Storage.** The referenced table element payload is exactly
`BilinearSurface::PAYLOAD_BYTES`: `X::KNOT_BYTES + X::INDEX_BYTES +
Y::KNOT_BYTES + Y::INDEX_BYTES + VALUE_BYTES`, with `VALUE_BYTES = 4*NX*NY`.
For the default binary pairing that is `2*NX + 2*NY + 4*NX*NY` bytes. Naming a
strategy changes the two axis terms and nothing else: `2*N` and no index for
`LinearAxis` and `BinaryAxis`, nothing at all for `UniformAxis`, and `2*N`
plus `2*B` for `BucketedAxis<N, B>`. Those figures are exact and
target-independent, and they are only the referenced element payload. It is
not total RAM, flash, binary, or linker cost; alignment, section placement,
code, and stack are outside it. The handle is separate and target-dependent:
`HANDLE_BYTES` is `size_of` of the handle on the current target. Every handle
has the value-grid reference and four one-byte boundary selections; each
Uniform axis adds no reference, each Linear or Binary axis adds one knot-array
reference, and each Bucketed axis adds both a knot-array and an index-array
reference. The default binary/binary handle is therefore three thin references
plus the policy and any alignment padding. It does not grow with `NX` or `NY`
for a fixed strategy pairing. Host tests assert these figures without assuming
a pointer width or field layout beyond Rust's guarantees. Code size, flash
placement, and stack depth are properties of the consuming build and its
linker; this crate states none of them as a guarantee.

Default binary `ELEVATION` 5×4: payload `10 + 8 + 80 = 98`. In-domain searches
are two endpoint comparisons plus `ceil(log2(5))` and `ceil(log2(4))` probes.
A successful evaluation is three interpolations and four grid reads:

```rust
use ph_surfaces::{AxisLookup, BilinearSurface, BinaryAxis};

fn main() {
    assert_eq!(BilinearSurface::<5, 4>::VALUE_BYTES, 80);
    assert_eq!(BilinearSurface::<5, 4>::PAYLOAD_BYTES, 98);
    assert_eq!(BilinearSurface::<5, 4>::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(BilinearSurface::<5, 4>::SUCCESS_GRID_READS, 4);
    assert_eq!(<BinaryAxis<5>>::MAX_SEARCH_COMPARISONS, 3);
    assert_eq!(<BinaryAxis<4>>::MAX_SEARCH_COMPARISONS, 2);
    assert_eq!(
        BilinearSurface::<5, 4>::HANDLE_BYTES,
        core::mem::size_of::<BilinearSurface<5, 4>>()
    );
}
```

Tiny Linear×Linear 3×2: six X knot bytes, four Y knot bytes, 24 value bytes,
payload 34; each axis searches at most `N - 1` knot comparisons:

```rust
use ph_surfaces::{AxisLookup, BilinearSurface, LinearAxis};

fn main() {
    type Tiny = BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>>;
    assert_eq!(Tiny::VALUE_BYTES, 24);
    assert_eq!(Tiny::PAYLOAD_BYTES, 34);
    assert_eq!(<LinearAxis<3>>::MAX_SEARCH_COMPARISONS, 2);
    assert_eq!(<LinearAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
    assert_eq!(Tiny::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(Tiny::SUCCESS_GRID_READS, 4);
}
```

Mixed `BucketedAxis<17, 8>` × `UniformAxis<9, 0, 200>`: X knots+index
`34 + 16`, Y knots 0, grid 612, payload 662. On this concrete irregular axis,
the bucket index reduces the X search bound from 5 comparisons (Binary) to 3;
Uniform uses no knot comparisons. Including the two endpoint comparisons per
in-domain axis, the lookup bound is 7 comparisons instead of 13 for
Binary×Binary, while the referenced payload is 662 bytes instead of 664:

```rust
use ph_surfaces::{
    AxisLookup, BilinearSurface, BinaryAxis, BucketedAxis, UniformAxis,
    bucket_index, max_local_comparisons,
};

fn main() {
    static X: [u16; 17] = [
        0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100,
        1_205, 1_300, 1_410, 1_500, 1_600,
    ];
    static X_INDEX: [u16; 8] = bucket_index(&X);
    type Mixed = BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>>;
    type AllBinary = BilinearSurface<17, 9>;
    assert_eq!(<BucketedAxis<17, 8>>::KNOT_BYTES, 34);
    assert_eq!(<BucketedAxis<17, 8>>::INDEX_BYTES, 16);
    assert_eq!(max_local_comparisons(&X, &X_INDEX), 3);
    assert_eq!(<BinaryAxis<17>>::MAX_SEARCH_COMPARISONS, 5);
    assert_eq!(<UniformAxis<9, 0, 200>>::KNOT_BYTES, 0);
    assert_eq!(<UniformAxis<9, 0, 200>>::MAX_SEARCH_COMPARISONS, 0);
    assert_eq!(Mixed::VALUE_BYTES, 612);
    assert_eq!(Mixed::PAYLOAD_BYTES, 662);
    assert_eq!(AllBinary::PAYLOAD_BYTES, 664);
    assert_eq!(Mixed::SUCCESS_INTERPOLATIONS, 3);
    assert_eq!(Mixed::SUCCESS_GRID_READS, 4);
}
```

**Work.** A worst-case `evaluate` is two axis searches and
`SUCCESS_INTERPOLATIONS` (exactly 3) scalar interpolations. Each in-domain
axis search is two endpoint comparisons plus the search work of that axis's
strategy — `AxisLookup::MAX_SEARCH_COMPARISONS`, and exactly `ceil(log2(len))`
probes for the default binary strategy. A clamped coordinate takes the
endpoint path: one or two comparisons and no probes. A rejected evaluation
returns before any interpolation or `SUCCESS_GRID_READS` (exactly 4) grid
reads, and a rejected X also skips the Y search. Exactly four grid elements
are read on success, and the grid is never scanned. For a `BucketedAxis`,
`max_local_comparisons` states the exact local bound for its own knots and
index, and raising the bucket count to a multiple of itself splits buckets
rather than moving their boundaries, so that bound never increases. That is
operation structure derived from the implementation and asserted by its tests.
It is not a cycle count or a WCET figure: no timing has been measured and none
is claimed.

**Verification targets.** The claims above are verified on the host and on two
representative bare-metal targets, `thumbv7em-none-eabi` (ARM Cortex-M4/M7)
and `riscv32imac-unknown-none-elf`, including a nightly core-only sysroot build
on both. Every other Rust target, and Xtensa in particular, is unproven and
unclaimed.

### Measured code-size (non-normative)

A reproducible recipe records compiler-object `.text` totals for four named,
single-pairing consumers. It is not a guarantee, not total flash, and not
WCET. The committed snapshot is
[`docs/code-size-snapshot.txt`](docs/code-size-snapshot.txt).

```sh
./scripts/measure-code-size.sh
```

- Toolchain: pinned 1.94.0 from `rust-toolchain.toml`, not nightly
- Targets: `thumbv7em-none-eabi`, `riscv32imac-unknown-none-elf`
- Profile: `opt-level = "s"`, `lto = false`, `codegen-units = 1`,
  `panic = "abort"`, `debug = false`
- Tool: `llvm-nm --demangle --print-size` from `llvm-tools-preview`; each line
  totals the compiler-object `.text` emitted for one pairing and its named
  `ph_eval_*` wrapper, not whole-binary flash
- Pairings: default Binary×Binary elevation 5×4; Linear×Linear 3×2;
  Uniform×Uniform 2×2; mixed `BucketedAxis<17, 8>` × `UniformAxis<9, 0, 200>`

Compiler, linker, and `llvm-tools-preview` versions can move these numbers.
Re-run the script and update the snapshot when they do. The `code size
snapshot` CI check diffs the output and returns SKIP if either target or
`llvm-tools-preview` is missing.

## Repository classification

These GitHub fields must agree with the manifest and this README. The bootstrap
token cannot write them; set them on the repository if they are empty.

| Field | Value |
| --- | --- |
| Custom property `Lifecycle` | `Incubating` |
| Custom property `Domain` | `Libraries` |
| Topics | `rust`, `embedded`, `no-std`, `no-alloc`, `interpolation` |

## How it is verified

The canonical entry point is local:

```sh
./scripts/ci.sh
```

That script reports each check as `PASS`, `FAIL`, or `SKIP`. A skipped check is
not a passed check. Release evidence sets an exact nightly and forbids skips:

```sh
NIGHTLY_TOOLCHAIN=nightly-2026-08-08 REQUIRE_NO_SKIPS=1 ./scripts/ci.sh
```

Strict mode also requires a clean Git worktree, validates the package's VCS
commit, and prints a verified archive SHA-256. Local `./scripts/ci.sh` is
authoritative. It gates:

- formatting, debug and release host tests and doctests (including every code
  block in this README), every Cargo example run as an assertion harness,
  clippy with warnings denied, and rustdoc with warnings denied and
  `missing_docs` denied on every public item;
- unconditional `#![no_std]`: no `[features]` table, no `cfg_attr` on the
  attribute, and no feature-gated code anywhere in `src/`;
- an integer-only, core-only, `unsafe`-free runtime, by grepping code paths;
- no `ph-curves` in any form — the manifest text (normal, optional,
  target-specific, development, build, path, Git, `[patch]`, `[replace]`),
  `Cargo.lock`, `cargo metadata --all-features`, and `cargo deny` all reject
  the name;
- the manifest floor (version, `publish = false`, licence, edition, MSRV,
  empty dependency tables);
- the package: the exact packaged file set (no agent notes, changelog, CI,
  deny, toolchain, script, or `docs/` material), a `cargo package` build of
  the artifact, the artifact's own rustdoc, doctests — README blocks
  included — and Cargo examples built from the unpacked package, and a fresh
  downstream `#![no_std]` consumer that declares the firmware quickstart,
  Uniform, and mixed fixtures together with both example maps above and all
  sixteen X/Y strategy pairings, is built and tested against the unpacked
  package on the host, and is built for both embedded targets — ordinarily and
  against a core-only sysroot, which is what proves the pairings themselves are
  allocation-free;
- a guard self-test (`scripts/guard-selftest.sh`) that mutates a copy of the
  tree — feature-conditional `no_std`, an allocator path, a `ph-curves`
  dependency — and requires the matching guard to fail;
- a code-size snapshot (`scripts/measure-code-size.sh`) that records
  single-pairing compiler-object `.text` totals on both embedded targets and
  diffs them against `docs/code-size-snapshot.txt`; the check reports `SKIP` if
  either target or `llvm-tools-preview` is missing;
- representative bare-metal builds on ARM (`thumbv7em-none-eabi`) and RISC-V
  (`riscv32imac-unknown-none-elf`) with the pinned toolchain;
- the no-allocation proof: nightly `-Z build-std=core` builds of the same two
  targets against a sysroot containing only `core`. A plain `--target` build
  is not that proof, because bare-metal `rust-std` sysroots still ship `alloc`.

Not proven, and not claimed: every Rust target, Xtensa, cycle counts,
code-size ceilings, or hard real-time WCET. The committed code-size snapshot
is labelled non-normative and is not a guarantee, not total flash, and not
WCET.

`cargo test` runs the crate's unit tests, its doctests, and the black-box
conformance suite in `tests/conformance/`. The suite exercises only the public
API and compares against an independent `i128` reference with a linear scan of
fixture knot arrays and remainder-based rounding; `strategies.rs` extends that
evidence across every applicable Linear/Binary/Uniform/Bucketed pairing.
Small declared domains are enumerated
exhaustively, the full `u16 × u16` range is sampled with a stated rule and is
not claimed exhaustive, and the locked X-then-Y fixture (axes `[0, 2]`, rows
`[[0, 0], [1, 3]]`, input `(1, 1)` → `1`) is retained. The two example maps
above are the suite's `ELEVATION` and `CORRECTION` fixtures and demonstrate
shape and rounding behaviour only; they claim nothing about any sensor, vendor,
or measurement accuracy.

[`docs/v0.1-traceability.md`](docs/v0.1-traceability.md) maps every
acceptance claim of the v0.1 umbrella to its implementation issue, test,
documentation section, or CI gate.

Hosted GitHub Actions are a **known gap until this repository is public**:
private runs fail before any step starts, so `pull_request` / `push` triggers
are not enabled. The workflow file remains for a manual `workflow_dispatch`
after the repository is public; it is a bounded subset (least privilege, a job
timeout, cancellation, SHA-pinned actions, one job as the aggregate status) and
still skips deny, nightly core-only, and GitHub metadata.

## Contributing and releases

Contributions are welcome under the repository-specific
[`CONTRIBUTING.md`](https://github.com/photon-circus/ph-surfaces/blob/main/CONTRIBUTING.md)
and
[`CODE_OF_CONDUCT.md`](https://github.com/photon-circus/ph-surfaces/blob/main/CODE_OF_CONDUCT.md).
Never put vulnerability details in a public issue; follow the organization
[security policy](https://github.com/photon-circus/.github/security/policy).

Durable publication is a separate maintainer action governed by
[`RELEASING.md`](https://github.com/photon-circus/ph-surfaces/blob/main/RELEASING.md).
A pull request approval does not by itself authorize a visibility change, tag,
crates.io upload, yank, or GitHub Release.
