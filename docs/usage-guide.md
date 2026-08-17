# Firmware usage guide

Task-oriented path from a firmware compensation, derating, or feed-forward
map to a static, allocation-free `BilinearSurface`. The [README](../README.md)
is the five-minute entry; this document is the checklist. It does not replace
the normative contract.

These are usage shapes, not accuracy claims. Numbers are invented and
device-neutral. Do not treat anything here as a vendor table, a calibration
result, a sensor tolerance, or a safety-integrity argument.

Assumptions that hold for every step:

- `#![no_std]` is the normal consuming environment.
- Tables and handles are `static` / `const`. There is no heap, no
  initialization routine, no cache, and no warm-up.
- The application has already quantized coordinates to `u16` and values to
  `i32`.
- Units, scaling, provenance, and validation evidence stay with the
  application. The surface handle records none of them.
- Repeatability and bounded operation structure matter. Actual cycles, stack,
  flash, and WCET require a named target measurement.
- Referenced payload, handle RAM, linked flash, code size, stack, and
  execution time are separate budgets.

## 1. Start from a firmware use case

Pick the two operating inputs the map depends on — ADC codes, timer counts,
temperature or load codes, actuator commands — and the signed output the
firmware will apply as a correction, derating, or feed-forward term.

The application owns the conversion into `u16` coordinates and `i32` values.
`ph-surfaces` never sees volts, degrees, newtons, or milli-units. A typical
shape:

- X: a quantized operating code already in `u16`.
- Y: a second quantized operating code already in `u16`.
- Grid: a signed correction already in `i32`.

If the raw quantity is signed or wider than `u16`, scale and offset it in
application code before `evaluate`. The surface will not do that conversion.

## 2. Choose integer scales the application keeps

Choose scales that preserve the resolution the firmware actually needs. Record
them next to the table in application comments or a host-side design note, not
inside the handle.

`ph-surfaces` neither owns nor records units. Two firmware crates can store
identical `i32` grids and mean entirely different physical quantities. Nothing
in `BilinearSurface` will catch a scale mismatch.

## 3. Declare strictly increasing static axes

Each axis needs at least two strictly increasing `u16` knots. Identify whether
each axis is irregular or an exact arithmetic progression — that choice is
what [choosing a strategy](choosing-a-strategy.md) uses later. For the default
path, store both axes as `static` arrays.

```rust
static X: [u16; 2] = [100, 200]; // operating codes, already quantized
static Y: [u16; 2] = [10, 30];
```

A duplicate or descending knot fails to compile in a `static` definition. That
is the intended outcome for a malformed compensation table.

## 4. Lay out the grid as `values[y][x]`

Y selects the **row**. X selects the **column**. The type is `[[i32; NX]; NY]`.
Writing `values[x][y]` is the common transposition mistake; on a non-square
grid it is a compile-time type error, on a square grid it compiles and is
wrong.

```text
                 X[0] = 100          X[1] = 200
              +-------------------+-------------------+
    Y[0] = 10 | values[0][0] = 0  | values[0][1] = 100|
              +-------------------+-------------------+
    Y[1] = 30 | values[1][0] = 40 | values[1][1] = 180|
              +-------------------+-------------------+
```

```rust
static VALUES: [[i32; 2]; 2] = [
    [0, 100],  // row Y = 10
    [40, 180], // row Y = 30
];
```

`VALUES[0][1]` is the correction at `(X=200, Y=10)`, not `(X=10, Y=200)`.

## 5. Construct the default binary surface from `static` data

Binary lookup is the safe default on both axes. `BilinearSurface::new` is a
`const fn`. The handle references the three tables and never copies them.
There is no allocator, no initialization routine, no cache, and no warm-up.

```rust
use ph_surfaces::BilinearSurface;

static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
```

The runnable form of this table is `examples/firmware_quickstart.rs`.

## 6. Name all four boundary sides

Every side defaults to `Error`. Name each one according to firmware behaviour:

- **Error** — the input is invalid; reject it. Typical for an uncharacterized
  low code or a sensor reading the application has already declared illegal.
- **Clamp** — hold the last characterized edge. The nearest declared endpoint
  is substituted and that boundary row or column is evaluated. Nothing is
  extrapolated.

```rust
use ph_surfaces::{Boundary, BoundaryPolicy};

static HOLD_HIGH_X: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES)
    .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));
```

The four sides are independent. See `examples/fail_safe_boundaries.rs` for
reject versus hold-last-edge, X-before-Y precedence, and the no-extrapolation
proof.

## 7. Evaluate knots, an interior point, every side, and X-before-Y

Before shipping the table:

- every declared knot returns its stored value;
- at least one interior operating point matches a hand computation (the
  walkthrough's `(125, 20) → 50` is the fixture used here);
- each of the four out-of-domain sides has an explicit expected outcome;
- when both coordinates are outside `Error` sides, the **X** error is the one
  reported.

```rust
assert_eq!(SURFACE.evaluate(100, 10), Ok(0));
assert_eq!(SURFACE.evaluate(125, 20), Ok(50));
```

The production order of that interior point is in
[the interpolation walkthrough](interpolation-walkthrough.md).

## 8. Diagnose compile-time validation failures

`BilinearSurface::new` and every strategy constructor are `const fn`. In a
`static` or `const` definition, a bad table **fails to compile**. Typical
causes:

- fewer than two knots on an axis;
- a duplicate or descending knot;
- a Uniform descriptor whose step is zero or whose last knot leaves `u16`;
- a Bucketed index that does not match `bucket_index` of its knots;
- a grid whose type is `[[i32; NY]; NX]` instead of `[[i32; NX]; NY]`.

An unequal X/Y transpose is a type error. A square transpose type-checks, so
orientation still has to be reviewed: walk one known `(x, y)` against
`values[y][x]`.

## 9. Switch one axis strategy without changing the contract

Strategies are selected in the type, independently for X and Y. Replacing
`BinaryAxis` on one axis cannot change values, errors, rounding, order, or
boundary behaviour — only stored bytes and search work. Build a non-default
pairing with `BilinearSurface::from_axes`.

```rust
use ph_surfaces::{BilinearSurface, BinaryAxis, UniformAxis};

static EVEN_Y: BilinearSurface<2, 2, BinaryAxis<2>, UniformAxis<2, 10, 20>> =
    BilinearSurface::from_axes(BinaryAxis::new(&X), UniformAxis::new(), &VALUES);
```

`EVEN_Y.evaluate(125, 20)` is still `Ok(50)`. Starting recommendations and
Bucketed tuning are in [choosing a strategy](choosing-a-strategy.md).

## 10. Put every cost figure in the correct budget

Inspect the const cost description (`VALUE_BYTES`, `PAYLOAD_BYTES`,
`HANDLE_BYTES`, `SUCCESS_INTERPOLATIONS`, `SUCCESS_GRID_READS`,
`AxisLookup::MAX_SEARCH_COMPARISONS`, `max_local_comparisons`) and file each
number:

| Figure | Budget |
| --- | --- |
| `PAYLOAD_BYTES` (and the per-axis `KNOT_BYTES` / `INDEX_BYTES`) | referenced static element payload — not total RAM, flash, or binary size |
| `HANDLE_BYTES` (`size_of` of the handle) | handle placement on **this** target; pointer-width dependent |
| two endpoint comparisons plus `MAX_SEARCH_COMPARISONS` per in-domain axis; three interpolations; four grid reads | bounded operation structure — not cycles, jitter, or WCET |
| compiler-object `.text`, linked flash, stack depth, measured latency | the consuming firmware's target, profile, and linker |

Worked comparisons for a tiny Linear/Linear 3×2, a Uniform/Uniform 17×9, and a
mixed Bucketed/Uniform 17×9 are asserted in
`examples/firmware_cost_budget.rs`.

## 11. Cross-compile the instantiated types and measure the real target

The crate's own gates build `thumbv7em-none-eabi` and
`riscv32imac-unknown-none-elf`, ordinarily and with a nightly
`-Z build-std=core` sysroot. That proves the **library** is core-only. It is
not a cycle, flash, stack, or WCET figure for a firmware that names one
pairing.

If code size or latency matters, measure the exact instantiated types on the
named target, toolchain, optimization profile, and linker configuration. Do
not extrapolate a comparison count or a referenced payload into any of those
budgets. The committed [`code-size-snapshot.txt`](code-size-snapshot.txt) is
labelled non-normative for the same reason.

## Authoring checklist

- Axes contain at least two strictly increasing `u16` knots.
- The grid shape is exactly `[[i32; NX]; NY]`.
- Y selects the row and X selects the column (`values[y][x]`).
- Units and fixed-point scales are documented by the application, not stored
  in the surface handle.
- All declared knots reproduce their stored values.
- At least one interior point and one rounding tie are checked by hand.
- All four boundary sides have explicit expected outcomes.
- Strategy choices and any target measurements record the exact target,
  toolchain, profile, linker configuration, and instantiated types.
- Static payload is distinguished from handle placement and linked code.
- The call site's stack and latency requirements are verified on the
  consuming target.
- The table's behaviour for invalid or saturated input codes is explicit.
- Examples perform no heap allocation, I/O, clocks, caching, runtime table
  construction, or hidden initialization.
