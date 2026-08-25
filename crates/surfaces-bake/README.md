# ph-surfaces-bake

Host baker for `ph-surfaces`.

> [!NOTE]
> **Lifecycle:** Active
> **Distribution:** unpublished host package in this repository
> (`ph-surfaces-bake`). Not on crates.io at runtime `v0.1.0`.
> **Model conformance:** N/A
> **Physical evidence:** N/A

This package requires `std` and `f64`. It is host tooling and must **never**
be linked into target firmware.

Ingest reads sample points from delimited text (X, Y, value as host `f64`)
and an **explicit** grid: a knot list per axis, or a uniform origin/step/count
matching the runtime `UniformAxis`. Quantize fills each declared node from
on-knot samples, applies the caller-stated scale with round-to-nearest
(exact half-way away from zero), and measures deviation of the quantized
table from every supplied sample in i32 value LSBs. The maximum is the
durable `MAX_ERR_LSB` const fragment; RMS, worst-sample coordinate, and
per-knot residual print on the CLI. The baker does not choose knots or
parse expressions.

`MAX_ERR_LSB` is an i32 value LSB: `ceil` of the exact rational
`|sample*scale − reconstruct|`. IEEE `f64` bit-patterns are dyadics;
bilinear is an exact ratio of the `i32` grid, computed on the host with
allocated integers. `ceil` applies only to the finished residual. A finite
residual whose ceil does not fit in `i32` is `BakeError::BoundOverflow`.
Host `f64` lerp is not the bound oracle. For samples whose X and Y are
exact `u16` values, that includes the runtime's rounded X-then-Y path. It
is not a typical error. It is not a device, vendor, sensor, calibration,
accuracy, timing, flash, or WCET claim.

Residual arithmetic uses `num-bigint`, `num-rational`, and `num-traits`.
Those crates stay off the runtime graph.

The public host API is `BakeInput::quantize` → `QuantizedTable`,
`emit_max_err_lsb`, `emit_rust` / `emit_rust_with`, and `write_goldens`.

```sh
ph-surfaces-bake --help
ph-surfaces-bake --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --samples points.txt --x-uniform 0,10,3 --y-uniform 0,5,3 --scale 1
ph-surfaces-bake --emit-rust --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --emit-golden
```

`--emit-rust` writes Rust source on stdout. The baker does not own the
destination path; `cargo xtask generate` places the checked-in copy. The
emitted `MAX_ERR_LSB` is an i32 value LSB: deviation between the supplied
samples and the table built from them. It is not a device, accuracy, timing,
or flash claim. `--emit-golden` writes the repository's checked-in rounding
fixture as frozen integer CSV under
`crates/surfaces/tests/conformance/golden/`, located from the working
directory (or `--out DIR`). It does not ingest the caller's samples.
Those files are read-only inputs: a failing test is an implementation defect
until proven otherwise. Regenerating them belongs in a dedicated golden-freeze
issue.

The baker may take reviewed host crates for exact residual arithmetic. A
declared implementation-line budget keeps it from growing without bound. The
generated fixture under `generated/` is not part of the packaged crate.

## License

MIT
