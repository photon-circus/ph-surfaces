# Generating tables

Task-oriented path from measured sample points to a checked-in static
`ph-surfaces` table. The packaged [README](https://github.com/photon-circus/ph-surfaces/blob/v0.1.0/README.md)
is the firmware contract; this document is the host workflow. It does not
replace the baker crate docs.

This is a generation procedure, not an accuracy claim. `MAX_ERR_LSB` is
deviation between the supplied samples and the table built from them. It is
not a device, vendor, sensor, calibration, timing, flash, or WCET figure.

## Two crates

- **Target:** `ph-surfaces` locates, interpolates, and reports. `no_std`,
  no-alloc, no `f64`. Firmware links this crate only.
- **Host:** `ph-surfaces-bake` ingests samples, quantizes, measures
  deviation, and emits source. `std` and `f64`. It must never be linked into
  target firmware.

A `gen` feature, optional dependency, or `cfg` on `ph-surfaces` that reaches
the baker is forbidden.

Formula authoring, symbolic math, simplification, and fitting are outside this
repository. The baker consumes sample points and never expressions.

## 1. State the grid

The baker does not choose knots. Supply an **explicit** grid: a knot list per
axis, or a uniform origin/step/count matching runtime `UniformAxis`, and a
caller-stated output scale for the `i32` value domain.

Sample text is delimited `X Y value` (whitespace and/or comma). Values are
host `f64`. Out-of-domain samples are errors, not drops. Grid validation uses
the same vocabulary as `BilinearSurface::new`.

```sh
ph-surfaces-bake --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --samples points.txt --x-uniform 0,10,3 --y-uniform 0,5,3 --scale 1
```

The CLI prints operator statistics (RMS, worst sample, per-knot residual).
The durable bound is `MAX_ERR_LSB`.

## 2. Quantize and bound

On-knot samples fill declared nodes. The stored scale is applied with
round-to-nearest, exact half-way away from zero. Off-knot samples participate
only in the bound.

`MAX_ERR_LSB` is `ceil` of the exact rational `|sample*scale − reconstruct|`.
IEEE `f64` bit-patterns are dyadics; bilinear is an exact ratio of the `i32`
grid on the host. `ceil` applies only to the finished residual. A finite ceil
that does not fit `i32` is `BakeError::BoundOverflow`. Host `f64` lerp is not
the oracle.

## 3. Emit Rust

`--emit-rust` writes static knot arrays, a row-major `values[y][x]` grid,
`PAYLOAD_BYTES`, and `MAX_ERR_LSB` to **stdout**. The baker does not own the
destination path. Default pairing is Binary × Binary; `--x-bucketed` /
`--y-bucketed` emit `BucketedAxis` when `B` is in `1..=65_536`.

```sh
ph-surfaces-bake --emit-rust --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
```

Check the emitted `static` tables into firmware. Evaluate them through
`ph-surfaces` only. `cargo xtask generate` writes the baker-owned checked-in
fixture used by the drift gate; that file is not the firmware table.

## 4. Freeze goldens when the numeric path is settled

`--emit-golden` writes integer CSV under
`crates/surfaces/tests/conformance/golden/`, located from the working
directory, or `--out DIR`. Those files are frozen inputs. A failing test is
an implementation defect until proven otherwise. Regenerating them is a
dedicated freeze commit: no implementation source in that commit, and a
changelog justification.

The runtime suite consumes the CSV through `ph_surfaces::*` only. It is not
a floating-point oracle. The runtime `i128` reference in
`tests/conformance/reference.rs` stays.

## License

MIT
