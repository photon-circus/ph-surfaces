# Agent instructions

Guidance for coding agents working in this repository. Read this before
changing code.

## What this crate is for

`ph-surfaces` will give firmware deterministic, allocation-free two-dimensional
integer mappings. **The `no_std` + no-alloc runtime is the product**, not a
nice-to-have.

This repository is on issue #3 of the v0.1 umbrella: the repository floor plus
the private scalar segment interpolation helper in `src/interp.rs`. Do not
implement `BilinearSurface`, axis lookup, or the public evaluator here. Those
are issues #4–#6.

`src/interp.rs` owns the only rounding policy in the crate: round to nearest,
exact half-way values away from zero. Route every interpolated value through
`div_round_half_away_from_zero` rather than adding a second implementation.

## Hard invariants

### 1. No `ph-curves` in any form

Do not add `ph-curves` as a direct, transitive, optional, target-specific,
development, build, path, or Git dependency. v0.1 interpolation is a private
helper in this crate (issue #3). Shared arithmetic is a later decision after
shipped duplication exists. `deny.toml` bans the crate name; CI greps
`cargo metadata` and `Cargo.lock`.

### 2. `#![no_std]` is unconditional

Never make it feature-conditional. Cargo unifies features across the whole
dependency graph, so `#![cfg_attr(not(feature = "..."), no_std)]` means any
unrelated crate enabling a host feature silently turns a firmware build into a
`std` build.

### 3. Core-only, no allocator, no unsafe

Do not introduce `alloc`, `std`, `unsafe`, or floating point. The `integer
only` check in `scripts/ci.sh` greps `src/` for those code paths, ignoring full
line comments so documentation may still discuss them.

A plain `--target` build does not prove no-alloc: bare-metal `rust-std` ships
`alloc` in the sysroot. The proof is:

```sh
cargo +nightly build --target thumbv7em-none-eabi -Z build-std=core
```

`rust-toolchain.toml` pins 1.92.0, so `+nightly` is required for `-Z`.

### 4. Local CI is authoritative

`./scripts/ci.sh` is the verification entry point. It reports `PASS`, `FAIL`,
and `SKIP` distinctly. A skipped check is not a passed check.

Hosted GitHub Actions are a known gap until this repository is public:
private workflow runs fail before any step starts. Do not re-enable
`pull_request` / `push` triggers to "fix" a red check. Do not treat a missing
or failed hosted run as a local-CI failure.

## Coupled edits

| Change | Also update |
| --- | --- |
| Version or `publish` | `README.md` status, `CHANGELOG.md`, `scripts/ci.sh` manifest assertions |
| New packaged file | `include` in `Cargo.toml` and the package-list check in `scripts/ci.sh` |
| New dependency | `deny.toml`, the no-`ph-curves` check, and an explicit reason in the PR |

## Validating

```sh
./scripts/ci.sh
```
