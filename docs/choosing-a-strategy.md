# Choosing a lookup strategy

Prescriptive starting guidance for flash- and latency-constrained firmware.
Strategies are selected **in the type**, independently for X and Y. A firmware
that names one combination compiles that one: there is no runtime enum, no
Cargo feature, and no strategy branch.

Changing a strategy cannot change values, errors, rounding, composition order,
or boundary behaviour. Only referenced static bytes and the bounded search
work differ.

Fewer comparisons do not necessarily mean fewer cycles. Fewer referenced
static bytes do not necessarily mean a smaller linked binary. Handle bytes
are target-dependent and must come from `BilinearSurface::HANDLE_BYTES` or
`size_of`. Code flash, stack, cycles, jitter, and WCET require a named
reproducible target build or measurement.

## Starting choice

| Situation | Starting choice | Required verification |
| --- | --- | --- |
| Unsure, or a general irregular axis | `BinaryAxis` (the default) | Confirm its exact comparison bound `ceil(log2(N))` is acceptable |
| Knots are an exact arithmetic progression | `UniformAxis` | Confirm the removed knot storage is valuable and measure division on the target if timing matters |
| Axis is very small | Compare `LinearAxis` with `BinaryAxis` | Compare generated target code and measured timing; do not assert a universal knot-count threshold |
| Irregular axis needs a smaller proven local bound | `BucketedAxis` | Compute candidate indexes and `max_local_comparisons`; accept the index only when the bound improvement justifies its bytes |

Binary is the safe default. `BilinearSurface<NX, NY>` and
`BilinearSurface::new` produce it on both axes. Reach for another strategy
when a specific budget — stored knots, a proven local comparison bound, or
the absence of a search — is what the firmware needs, then verify that
budget on the instantiated types.

The four strategies in one line:

- `LinearAxis<N>` — stored knots, bounded scan of at most `N - 1`
  comparisons. Minimum auxiliary structure.
- `BinaryAxis<N>` — stored knots, exactly `ceil(log2(N))` probes. General
  default.
- `UniformAxis<N, ORIGIN, STEP>` — no stored knots; one subtraction and one
  division. Only when the axis is an exact arithmetic progression.
- `BucketedAxis<N, B>` — stored knots plus `2*B` index bytes; one bucket read
  plus a local scan bounded by `max_local_comparisons` for those knots.

Runnable illustrations: `crates/surfaces/examples/uniform_sensor_compensation.rs`,
`crates/surfaces/examples/mixed_calibration_map.rs`, `crates/surfaces/examples/firmware_cost_budget.rs`.

## Independent axes

X and Y choose separately. A mixed pairing is normal: an irregular
characterized X axis can be Bucketed while an evenly coded Y axis is Uniform.
`crates/surfaces/examples/mixed_calibration_map.rs` is that shape, and it evaluates identically
to the default Binary/Binary surface over the same tables.

## Tiny Linear versus Binary

On a 3×2 surface both Linear/Linear and Binary/Binary reference **34** bytes
and perform the same worst-case knot-comparison count (four endpoint
comparisons plus three search comparisons). Choosing between them is a
target-code and measured-timing question, not a universal threshold on `N`.
`crates/surfaces/examples/firmware_cost_budget.rs` asserts those figures and refuses to turn
them into a cycle count.

## Uniform when the axis is an exact progression

If knot `i` is `ORIGIN + i * STEP` and that last value fits `u16`, Uniform
stores nothing: `KNOT_BYTES = 0`, `INDEX_BYTES = 0`,
`MAX_SEARCH_COMPARISONS = 0`. A 17×9 Uniform/Uniform grid is **612**
referenced bytes against **664** for Binary/Binary — 52 bytes of knot arrays
dropped. Location is a subtraction and a division by a compile-time `STEP`,
not zero work in cycles. Measure that division on the target if latency
matters.

`crates/surfaces/examples/uniform_sensor_compensation.rs` shows a small Uniform/Uniform
compensation map, endpoint accessors on described (not stored) knots, and
equality with the equivalent default surface.

## Bucketed tuning procedure

Start from Binary. Generate candidate indexes. Keep a candidate only when
`max_local_comparisons` is **strictly smaller** than the Binary bound you
already have, and pick the **smallest** such index that meets the bound the
firmware actually needs.

Worked irregular X axis (the mixed 17×9 fixture):

```rust
static X: [u16; 17] = [
    0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205,
    1_300, 1_410, 1_500, 1_600,
];
```

Binary search bound on this axis: `ceil(log2(17)) = 5`. Nested candidates:

| `B` | Index bytes `2*B` | `max_local_comparisons` | Keep? |
| --- | --- | --- | --- |
| 2 | 4 | 9 | No — worse than Binary's 5 |
| 4 | 8 | 5 | No — does not improve the bound |
| 8 | 16 | 3 | Yes, if the firmware needs a bound of 3 |
| 16 | 32 | 2 | Yes, only if 3 is not enough and 16 extra bytes are acceptable |

Procedure:

1. Start from Binary (`MAX_SEARCH_COMPARISONS = 5` here).
2. Generate `bucket_index` for nested counts 2, 4, 8, and 16.
3. Record `2*B` index bytes and `max_local_comparisons` for each.
4. Discard candidates that do not improve the required bound (2 and 4 above).
5. Select the smallest remaining index that meets the bound. For a required
   local bound of 3, that is `B = 8` (16 bytes), not `B = 16`.
6. Measure the final instantiated surface on the real target if code size or
   latency matters. The comparison count is not a cycle count: each in-domain
   Bucketed search also performs one bucket read and the arithmetic mapping
   into a bucket.

`crates/surfaces/examples/mixed_calibration_map.rs` asserts `max_local_comparisons == 3` for
`B = 8`, equality with Binary/Binary, and that coarser nested indexes do not
beat Binary. `crates/surfaces/examples/firmware_cost_budget.rs` places the mixed pairing in
the payload / handle / work / target-measurement budgets: referenced payload
**662** bytes versus **664** for all-binary, worst search comparisons 3 + 0,
worst total knot comparisons 7 versus 13, plus the bucket read.

Raising `B` to a multiple of itself splits buckets rather than moving their
boundaries, so `max_local_comparisons` never increases along that nested
sequence. That is a bound property, not a reason to pick a large `B` by
default.
