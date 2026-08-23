# ph-surfaces documentation

The repository [README](../README.md) is the public entry point and describes
the crate's contract. The documents here provide longer usage guidance,
worked examples, traceability, and non-normative build evidence.

## Guides

- [Firmware usage guide](usage-guide.md) — turn a quantized two-dimensional
  map into a static, allocation-free surface and handle boundary failures.
- [Interpolation walkthrough](interpolation-walkthrough.md) — follow one query
  through the crate's exact X-then-Y interpolation and rounding order.
- [Choosing a lookup strategy](choosing-a-strategy.md) — compare the four
  per-axis strategies by referenced storage and bounded search structure.

## Maintainer evidence

- [v0.1 release traceability](v0.1-traceability.md) — compact evidence map from
  the public contract to its implementation, tests, documentation, and CI
  checks.

## Build snapshots

These committed snapshots are reproducible build evidence, not API guarantees,
total flash measurements, cycle counts, or WCET claims.

- [Code-size snapshot](code-size-snapshot.txt)
- [Thumb assembly snapshot](asm-snapshot-thumbv7em-none-eabi.txt)
- [RISC-V assembly snapshot](asm-snapshot-riscv32imac-unknown-none-elf.txt)

For release-only actions and evidence requirements, see
[`RELEASING.md`](../RELEASING.md). Repository documentation under `docs/` is
intentionally excluded from the packaged crate.
