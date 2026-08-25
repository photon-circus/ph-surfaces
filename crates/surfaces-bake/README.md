# ph-surfaces-bake

Host baker for `ph-surfaces`.

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

Residual arithmetic uses `num-bigint` and `num-rational`. Those crates stay
off the runtime graph.

The public host API is `BakeInput::quantize` → `QuantizedTable`, plus
`emit_max_err_lsb` for the const fragment.

**Model conformance: N/A. Physical evidence: N/A.**

```sh
ph-surfaces-bake --help
ph-surfaces-bake --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --samples points.txt --x-uniform 0,10,3 --y-uniform 0,5,3 --scale 1
```

Rust emission and goldens are later issues. The baker may take reviewed host
crates for exact residual arithmetic. A declared implementation-line budget
keeps it from growing without bound.

## License

MIT
