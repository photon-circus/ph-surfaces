# ph-surfaces-bake

Host baker for `ph-surfaces`.

This package requires `std` and `f64`. It is host tooling and must **never**
be linked into target firmware.

Ingest reads sample points from delimited text (X, Y, value as host `f64`)
and an **explicit** grid: a knot list per axis, or a uniform origin/step/count
matching the runtime `UniformAxis`. A caller-stated output scale is stored
and not applied. The baker does not choose knots, parse expressions, or
quantize values.

```sh
ph-surfaces-bake --help
ph-surfaces-bake --samples points.txt --x-knots 0,10 --y-knots 0,5 --scale 1
ph-surfaces-bake --samples points.txt --x-uniform 0,10,3 --y-uniform 0,5,3 --scale 1
```

Quantization, Rust emission, and goldens are later issues. The crate has
zero third-party dependencies and a 1,500-line implementation budget.

## License

MIT
