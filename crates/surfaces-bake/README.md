# ph-surfaces-bake

Host baker for `ph-surfaces`.

This package requires `std` and `f64`. It is host tooling and must **never**
be linked into target firmware.

Sample ingest, quantization, emission, and goldens are later issues. This
crate is the floor: a library plus a thin CLI, zero third-party dependencies,
and a 1,500-line implementation budget.

```sh
ph-surfaces-bake --help
```

## License

MIT
