# Agent instructions

Guidance for coding agents working in this repository. Read this before
changing code.

## What this crate is for

`ph-surfaces` will give firmware deterministic, allocation-free two-dimensional
integer mappings. **The `no_std` + no-alloc runtime is the product**, not a
nice-to-have.

This repository contains the accepted binary-lookup baseline through issues
#2–#8 of umbrella #1, the compile-time per-axis lookup strategies of #18,
cross-strategy conformance and cost evidence from #19, the completed
documentation/package-readiness gate #9, and the embedded usage guides,
strategy cookbook, and runnable firmware examples of #22. Publishing, tagging,
and a stable 1.0 promise are separate maintainer decisions; `publish = false`
stays until then.

`README.md` is packaged and every one of its Rust code blocks runs as a doctest
(the `cfg(doctest)` module in `src/lib.rs` includes it), so a README example
that stops compiling fails `cargo test`. Keep README code blocks complete and
runnable, or tag a non-Rust block with its language (`sh`, `text`).

`tests/conformance/` is black-box evidence for the public contract. It goes
through `ph_surfaces::*` only and must never import a private module, add a
dev-dependency, or use `ph-curves` or floating point as an oracle; its expected
values come from the independent `i128` reference in `tests/conformance/
reference.rs` or from hand computation shown in comments. Keep the crate's own
unit tests in `src/`; do not move them into the suite or duplicate them there.

`src/interp.rs` owns the only rounding policy in the crate: round to nearest,
exact half-way values away from zero. Route every interpolated value through
`div_round_half_away_from_zero` rather than adding a second implementation.

`src/axis/` owns the four sealed lookup strategies — `LinearAxis`,
`BinaryAxis` (the default), `UniformAxis`, `BucketedAxis` — each of which
answers only *which segment holds this coordinate*. `src/lookup.rs` owns
everything else about a lookup: the endpoint tests, the boundary policy, the
clamped-coordinate substitution, the cell invariants, and the thin
axis-specific `SurfaceError` mapping. Keep that split. A new strategy must
produce the same axis-neutral location result and preserve exact-knot,
boundary, and no-extrapolation semantics, must validate itself in its own
`const fn` constructor, and must not construct, cache, or mutate anything at
runtime. Strategy selection stays type-level: no runtime enum, no Cargo
feature, no strategy branch in a firmware that names one combination.

`src/evaluate.rs` owns the only composition order in the crate. X is resolved
before Y, X interpolates on each of the two Y rows before Y interpolates between
those two already-rounded results, and the X-side error wins when both
coordinates are out of domain. That order is observable, because every step
rounds. Do not add a second evaluation path, a selectable order, a cached cell,
or arithmetic that bypasses `interp.rs`.

## Hard invariants

### 1. No `ph-curves` in any form

Do not add `ph-curves` as a direct, transitive, optional, target-specific,
development, build, path, or Git dependency. v0.1 interpolation is a private
helper in this crate (issue #3). Shared arithmetic is a later decision after
shipped duplication exists. `deny.toml` bans the crate name; the `no ph-curves` check greps the manifest
text outside comments, `Cargo.lock`, and `cargo metadata --all-features`, and
the mutation tests in `tools/xtask/tests/mutation.rs` prove that guard fires.

### 2. `#![no_std]` is unconditional

Never make it feature-conditional. Cargo unifies features across the whole
dependency graph, so `#![cfg_attr(not(feature = "..."), no_std)]` means any
unrelated crate enabling a host feature silently turns a firmware build into a
`std` build. The `no_std unconditional` check therefore also rejects a
`[features]` table and any `feature = "..."` cfg in `src/`; a features table
must arrive together with a proof that none of its members can touch the
attribute.

### 3. Core-only, no allocator, no unsafe

Do not introduce `alloc`, `std`, `unsafe`, or floating point. The `integer
only` check in `tools/xtask` scans `src/` for those code paths, ignoring full
line comments so documentation may still discuss them. The float and
`ph-curves` greps also cover `tests/` and `examples/`, so the conformance suite
and Cargo examples cannot acquire a floating-point or `ph-curves` oracle.
Examples use a host `main` only as an assertion harness: a separate examples
guard rejects allocator/std paths, common allocating prelude types and macros,
host printing/debug macros, and `unsafe` from their uncommented code.

A plain `--target` build does not prove no-alloc: bare-metal `rust-std` ships
`alloc` in the sysroot. The proof is the core-only build, run for both
representative targets:

```sh
cargo +nightly build --target thumbv7em-none-eabi -Z build-std=core
cargo +nightly build --target riscv32imac-unknown-none-elf -Z build-std=core
```

`rust-toolchain.toml` pins 1.94.0, so `+nightly` is required for `-Z`. The
same two targets are also built with the pinned toolchain as ordinary
bare-metal builds; do not describe those as a no-alloc proof.
The gate uses the moving `nightly` alias for ordinary developer runs, but
accepts `--nightly nightly-YYYY-MM-DD`; the release profile requires that flag
to name the reviewed dated nightly, and evidence must record its exact `rustc`
and `cargo` versions.

### 3a. Resource and cost claims stay exact and separate

Public docs state the referenced element payload of the default binary surface
as exactly `2*NX + 2*NY + 4*NX*NY` bytes, the general per-strategy form as
`X::KNOT_BYTES + X::INDEX_BYTES + Y::KNOT_BYTES + Y::INDEX_BYTES + 4*NX*NY`,
and the handle separately as a value-grid reference, the selected strategies'
fields, four policy bytes, and padding. Uniform stores no axis reference,
Linear/Binary one, and Bucketed two; the default binary/binary handle therefore
has three thin references. Never call the payload total RAM, flash, binary, or
linker cost.
Never state a cycle count or WCET figure; the documented worst case is
operation structure (two axis searches of two endpoint comparisons plus that
strategy's `MAX_SEARCH_COMPARISONS`, three scalar interpolations, four grid
reads). `src/surface.rs` and `src/axis/` tests assert the storage figures
without assuming a pointer width.

### 4. Local CI is authoritative

`cargo xtask ci` is the verification entry point. It reports `PASS`, `FAIL`,
and `SKIP` distinctly. A skipped check is not a passed check.

The gate is `tools/xtask`, a zero-dependency Rust crate in its own workspace,
so it runs identically on Windows and Linux with nothing but the pinned cargo
this crate already requires. There is no shell script and no PowerShell twin.
`tools/xtask/Cargo.toml` carries an empty `[workspace]` table so the repository
manifest never gains one and the root `Cargo.lock` stays dependency-free.

`cargo xtask ci --profile release --nightly nightly-YYYY-MM-DD` is the
release-evidence mode: every check must run, a would-be `SKIP` is recorded as
`FAIL`, and package checks require a clean Git worktree, validate the packaged
commit provenance, and print a verified SHA-256 digest. The matrix includes both
debug and release test suites.

`cargo xtask ci --only '<check name>'` runs one check; `cargo xtask list` prints
the registry. `--skip-embedded` drops the target-dependent checks and
`--fail-fast` stops at the first failure. The `package` family builds the
`.crate`, asserts its exact file list, verifies its provenance and digest, and
compiles the downstream `#![no_std]` consumer in `tools/consumer` against it.
`guards fire on mutation` runs `tools/xtask/tests/mutation.rs`, which mutates
copies of the tracked tree and requires the intended guard to fail. Every check
that needs an optional tool or target reports `SKIP`, with the reason, rather
than passing.

Hosted GitHub Actions are a known gap until this repository is public:
private workflow runs fail before any step starts. Do not re-enable
`pull_request` / `push` triggers to "fix" a red check. Do not treat a missing
or failed hosted run as a local-CI failure.

## Coupled edits

| Change | Also update |
| --- | --- |
| Version or crate `publish` setting | `Cargo.lock`, `README.md` and `src/lib.rs` status, this file's status text, `CHANGELOG.md`, `docs/v0.1-traceability.md`, every hardcoded version/tag/dependency/yank example in `RELEASING.md`, and the package constants plus manifest assertions in `tools/xtask/src/checks/` |
| New packaged file | `include` in `Cargo.toml`, and `PACKAGED_FILES` in `tools/xtask/src/checks/package.rs` |
| New guard in `tools/xtask` | A row in `CHECKS` (`tools/xtask/src/checks/mod.rs`) and a mutation case in `tools/xtask/tests/mutation.rs` showing it fails |
| Storage or cost wording | `src/lib.rs` crate docs, `src/surface.rs` / `src/evaluate.rs` / `src/axis/` item docs, `README.md` "Resource accounting and cost" |
| New or changed axis strategy | `src/lib.rs` re-exports and § Contract, `README.md` "Per-axis lookup strategies" table, the sixteen-pairing consumer in `tools/consumer/src/lib.rs`, `docs/v0.1-traceability.md` |
| New dependency | `deny.toml`, the no-`ph-curves` check, and an explicit reason in the PR |
| New or changed public API item | `src/lib.rs` module docs, `README.md` status sections, `CHANGELOG.md`, `docs/v0.1-traceability.md` |
| Example map values (`ELEVATION`, `CORRECTION`) | `tests/conformance/fixtures.rs` and `examples.rs`, `README.md` "Examples", `src/lib.rs` § Examples, `tools/consumer/src/lib.rs` |
| Firmware example fixtures (quickstart, uniform, mixed, fail-safe, cost) | `examples/*.rs`, `tests/conformance/fixtures.rs` and `examples.rs`, README "Start here", `docs/usage-guide.md` / `interpolation-walkthrough.md` / `choosing-a-strategy.md`, `tools/consumer/src/lib.rs` |
| Contract wording or acceptance claim | `README.md` "Contract", `src/lib.rs` § Contract, `docs/v0.1-traceability.md` |

## Validating

```sh
cargo xtask ci
```

## Cursor Cloud specific instructions

This crate has no runtime services; "running the app" means the compile/test/lint
matrix. `cargo xtask ci` is the authoritative end-to-end check and is what to run
to prove the environment works.

The startup update script provisions everything the full matrix needs beyond the
pinned toolchain: the `nightly` toolchain with `rust-src` (for the
`-Z build-std=core` proof), the `thumbv7em-none-eabi` and
`riscv32imac-unknown-none-elf` targets, and `cargo-deny`. The pinned `1.94.0`
toolchain (with `rustfmt`, `clippy`, `rust-src`, `llvm-tools-preview`)
auto-installs from `rust-toolchain.toml` on the first cargo/rustup command run
inside the repo.

Non-obvious gotchas:
- The `github metadata` check reports `SKIP` here (and will keep skipping): it
  needs a public repo plus a token that can read topics/custom properties, which
  the cloud VM does not have. Per this repo's own rules, `SKIP` is expected and is
  not a failure for an ordinary developer run. It is deliberately a failure in
  the `release` profile.
- `cargo xtask ci --skip-embedded` skips the two top-level ordinary embedded
  `cargo build` checks (`thumbv7em-none-eabi`, `riscv32imac-unknown-none-elf`)
  and the `code size snapshot`, which also needs those targets. It does **not**
  skip either nightly core-only `-Z build-std=core` proof or the
  packaged-consumer matrix; pass it only expecting a partial skip, not a fully
  host-only run.
