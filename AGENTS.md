# Agent instructions

Guidance for coding agents working in this repository. Read this before
changing code.

## What this crate is for

`ph-surfaces` will give firmware deterministic, allocation-free two-dimensional
integer mappings. **The `no_std` + no-alloc runtime is the product**, not a
nice-to-have.

This repository is on issue #2 of the v0.1 umbrella: crate and repository
floor only. Do not implement interpolation, `BilinearSurface`, axis lookup, or
the public evaluator here. Those are issues #3–#6.

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

Do not introduce `alloc`, `std`, or `unsafe`. A plain `--target` build does not
prove no-alloc: bare-metal `rust-std` ships `alloc` in the sysroot. The proof
is:

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

## Cursor Cloud specific instructions

This crate has no runtime services; "running the app" means the compile/test/lint
matrix. `./scripts/ci.sh` is the authoritative end-to-end check and is what to run
to prove the environment works.

The startup update script provisions everything the full matrix needs beyond the
pinned toolchain: the `nightly` toolchain with `rust-src` (for the
`-Z build-std=core` proof), the `thumbv7em-none-eabi` and
`riscv32imac-unknown-none-elf` targets, and `cargo-deny`. The pinned `1.92.0`
toolchain (with `rustfmt`, `clippy`, `rust-src`) auto-installs from
`rust-toolchain.toml` on the first cargo/rustup command run inside the repo.

Non-obvious gotchas:
- The `github metadata` check reports `SKIP` here (and will keep skipping): it
  needs a public repo plus a token that can read topics/custom properties, which
  the cloud VM does not have. Per this repo's own rules, `SKIP` is expected and is
  not a failure — do not try to "fix" it.
- `SKIP_EMBEDDED=1 ./scripts/ci.sh` only skips the two embedded-target `cargo
  check` steps (`thumbv7em-none-eabi`, `riscv32imac-unknown-none-elf`). It does
  **not** skip the nightly core-only `-Z build-std=core` proof, which still builds
  for `thumbv7em-none-eabi` whenever the `nightly` toolchain and `rust-src` are
  installed (as this environment provides); set it only expecting a partial skip,
  not a fully host-only run. `FAIL_FAST=1` stops at the first failure.
