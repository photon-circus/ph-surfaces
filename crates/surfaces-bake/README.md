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

`MAX_ERR_LSB` is an i32 value LSB: the maximum absolute deviation between
the supplied samples and the table built from them. For samples whose X
and Y are exact `u16` values, that includes the runtime's rounded
X-then-Y path, not only unrounded host bilinear. It is an upper bound,
not a typical error. It is not a device, vendor, sensor, calibration,
accuracy, timing, flash, or WCET claim.

**Model conformance: N/A. Physical evidence: N/A.**

```sh
ph-surfaces-bake --help
ph-surfaces-bake --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --samples points.txt --x-uniform 0,10,3 --y-uniform 0,5,3 --scale 1
```

Rust emission and goldens are later issues. The crate has zero third-party
dependencies and a 1,500-line implementation budget.

## License

MIT
