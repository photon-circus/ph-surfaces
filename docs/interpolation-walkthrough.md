# Interpolation walkthrough

A firmware correction map, walked in the exact production order. This is not
a generalized formula first: it is one query on one invented table. The same
fixture is `examples/firmware_quickstart.rs` and the README quickstart.

X and Y are quantized operating inputs. The grid is a signed correction.
Units stay with the application.

## Primary fixture

```text
             X=100       X=200
Y=30           40          180
Y=10            0          100

Query: X=125, Y=20  →  50
```

Declared row-major as `values[y][x]`:

```rust
use ph_surfaces::BilinearSurface;

static X: [u16; 2] = [100, 200];
static Y: [u16; 2] = [10, 30];
static VALUES: [[i32; 2]; 2] = [
    [0, 100],  // Y = 10
    [40, 180], // Y = 30
];

static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
```

## Production order

Each scalar step computes `(y0 * (span - offset) + y1 * offset) / span` in
`i64` and rounds to nearest, with an exact half-way value away from zero.
There is one rounding helper in the crate; every interpolated value goes
through it.

1. **The cell.** `125` is inside X `[100, 200]` and `20` is inside Y
   `[10, 30]`. Both coordinates are in-domain, so both axes search (Binary
   default: two endpoint comparisons plus `ceil(log2(N))` probes each).

2. **Lower-row X interpolation** (Y = 10, values `0` and `100`).
   Span `200 - 100 = 100`, offset `125 - 100 = 25`:

   `(0 * 75 + 100 * 25) / 100 = 25`.

   Exact, so rounding does not move the result.

3. **Upper-row X interpolation** (Y = 30, values `40` and `180`).
   Same span and offset:

   `(40 * 75 + 180 * 25) / 100 = 75`.

   Exact.

4. **Final Y interpolation** between the two **already-rounded** X results
   `25` and `75`. Span `30 - 10 = 20`, offset `20 - 10 = 10`:

   `(25 * 10 + 75 * 10) / 20 = 50`.

   Exact.

```rust
assert_eq!(SURFACE.evaluate(125, 20), Ok(50));
// The usual three interpolation steps land exactly on this stored knot.
assert_eq!(SURFACE.evaluate(100, 10), Ok(0));
```

If any of those three divisions had landed on an exact half-way value, the
helper would have rounded away from zero **at that step** before the next
step ran. That is why order is observable.

## Half-way ties

A positive half-way value rounds away from zero to the more positive integer;
a negative half-way value rounds away from zero to the more negative integer.
The crate's locked ties fixture:

```rust
use ph_surfaces::BilinearSurface;

static AXIS: [u16; 2] = [0, 2];
static VALUES: [[i32; 2]; 2] = [[0, 1], [0, -1]];
static TIES: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);

assert_eq!(TIES.evaluate(1, 0), Ok(1));  // +0.5 → 1
assert_eq!(TIES.evaluate(1, 2), Ok(-1)); // -0.5 → -1
```

A toward-zero rounding would return `0` on both of those queries. The
conformance suite includes a mutant oracle that does exactly that, and these
points disagree with it.

## Why X-then-Y is locked

Axes `[0, 2]`, rows `[[0, 0], [1, 3]]`, query `(1, 1)`:

- **X then Y (this crate):** lower row interpolates to `0`; upper row
  interpolates to `(1 + 3) / 2 = 2`; Y interpolates `(0 + 2) / 2 = 1`.
- **Y then X (not this crate):** left column interpolates to `0.5 → 1`; right
  column interpolates to `1.5 → 2`; X interpolates `(1 + 2) / 2 = 1.5 → 2`.

```rust
use ph_surfaces::BilinearSurface;

static AXIS: [u16; 2] = [0, 2];
static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);

assert_eq!(SURFACE.evaluate(1, 1), Ok(1));
```

Because every step rounds, the two orders are different functions. Firmware
that composed Y first would not match this crate, and the black-box suite
rejects that mutant on this fixture.

## Boundary precedence

X is resolved first, then Y.

- Both coordinates outside `Error` sides: the **X** error is reported; Y is
  not consulted.
- X **clamps**: Y is still resolved under its own policy, so a clamped X can
  be followed by a Y error.
- `Clamp` substitutes the nearest declared endpoint and evaluates that
  boundary row or column. The result is a stored or interpolated value inside
  the hull. Extrapolation is never performed.

```rust
use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static X: [u16; 2] = [0, 10];
static Y: [u16; 2] = [0, 10];
static VALUES: [[i32; 2]; 2] = [[0, 100], [200, 300]];

static STRICT: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
static CLAMP_X: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES)
    .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));

assert_eq!(
    STRICT.evaluate(11, 11),
    Err(SurfaceError::XAbove { coordinate: 11, bound: 10 })
);
assert_eq!(CLAMP_X.evaluate(4_000, 0), Ok(100)); // last characterized X column
assert_eq!(
    CLAMP_X.evaluate(4_000, 11),
    Err(SurfaceError::YAbove { coordinate: 11, bound: 10 })
);
```

`examples/fail_safe_boundaries.rs` names all four sides on a firmware-style
policy.

## Boundary outcome table

Domain endpoints are inclusive. `x_min`/`x_max`/`y_min`/`y_max` are the
declared first and last knots.

| Query region | Error on that side | Clamp on that side |
| --- | --- | --- |
| Inside the inclusive domain | interpolates the containing cell | same value; policy is idle |
| X below `x_min`, Y in domain | `XBelow { coordinate, bound: x_min }` | evaluate the first X column at the (possibly clamped) Y |
| X above `x_max`, Y in domain | `XAbove { coordinate, bound: x_max }` | evaluate the last X column |
| Y below `y_min`, X in domain | `YBelow { coordinate, bound: y_min }` | evaluate the first Y row |
| Y above `y_max`, X in domain | `YAbove { coordinate, bound: y_max }` | evaluate the last Y row |
| Both axes outside, both Error | **X** variant; Y is not reported | — |
| X Clamp and Y Error, both outside | — | X substitutes its endpoint, then Y reports its error |
| Any Clamp result | — | a value inside the hull of stored grid values; never an extrapolated point |

No combination of `Error` and `Clamp` extrapolates. A rejected coordinate
returns before interpolation or grid reads; a rejected X also skips the Y
search.
