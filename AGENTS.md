# Agent instructions

Guidance for coding agents working in this repository. Read this before
changing code.

## What this crate is for

`ph-surfaces` gives firmware deterministic, allocation-free two-dimensional
integer mappings. **The `no_std` + no-alloc runtime is the product**, not a
nice-to-have. `ph-surfaces-bake` is the host-side baker: it requires `std`
and `f64`, and must never be linked into target firmware.

Human-facing docs describe a public Active crate published at `0.1.0`. Version,
`publish`, changelog close-out, GitHub visibility, and the crates.io upload
belong to the release process in `RELEASING.md`; do not perform those actions
in an implementation change.

`README.md` is packaged and every one of its Rust code blocks runs as a doctest
(the `cfg(doctest)` module in `crates/surfaces/src/lib.rs` includes it), so a
README example that stops compiling fails `cargo test`. Keep README code blocks
complete and runnable, or tag a non-Rust block with its language (`sh`, `text`).
Unpackaged guides are linked from the README with GitHub URLs, not relative
`docs/` paths.

`crates/surfaces/tests/conformance/` is black-box evidence for the public
contract. It goes through `ph_surfaces::*` only and must never import a private
module, add a dev-dependency, or use `ph-curves` or floating point as an
oracle; its expected values come from the independent `i128` reference in
`crates/surfaces/tests/conformance/reference.rs` or from hand computation shown
in comments. Keep the crate's own unit tests in `crates/surfaces/src/`; do not
move them into the suite or duplicate them there.

`crates/surfaces/src/interp.rs` owns the only rounding policy in the crate:
round to nearest, exact half-way values away from zero. Route every
interpolated value through `div_round_half_away_from_zero` rather than adding
a second implementation. It is also the crate's arithmetic kernel: the only
runtime module allowed 64-bit intermediates (the documented `~2^47` numerator
bound is why they exist), and where any future fixed-point scaling arrives as
typed helpers in code, never as a prose convention spread across modules.
128-bit integers belong only to test oracles. The `integer only` check
enforces both rules; its scanner exempts each file's `#[cfg(test)]` tail, so
keep test modules at the end of every `crates/surfaces/src/` file.

`crates/surfaces/src/axis/` owns the four sealed lookup strategies —
`LinearAxis`, `BinaryAxis` (the default), `UniformAxis`, `BucketedAxis` — each
of which answers only *which segment holds this coordinate*.
`crates/surfaces/src/lookup.rs` owns everything else about a lookup: the
endpoint tests, the boundary policy, the clamped-coordinate substitution, the
cell invariants, and the thin axis-specific `SurfaceError` mapping. Keep that
split. A new strategy must produce the same axis-neutral location result and
preserve exact-knot, boundary, and no-extrapolation semantics, must validate
itself in its own `const fn` constructor, and must not construct, cache, or
mutate anything at runtime. Strategy selection stays type-level: no runtime
enum, no Cargo feature, no strategy branch in a firmware that names one
combination.

`crates/surfaces/src/evaluate.rs` owns the only composition order in the crate.
X is resolved before Y, X interpolates on each of the two Y rows before Y
interpolates between those two already-rounded results, and the X-side error
wins when both coordinates are out of domain. That order is observable,
because every step rounds. Do not add a second evaluation path, a selectable
order, a cached cell, or arithmetic that bypasses `interp.rs`.

## Hard invariants

### 1. No `ph-curves` in any form

Do not add `ph-curves` as a direct, transitive, optional, target-specific,
development, build, path, or Git dependency. v0.1 interpolation is a private
helper in this crate. Shared arithmetic is a later decision after shipped
duplication exists. `deny.toml` bans the crate name; the `no ph-curves` check
greps the manifest
text outside comments, `Cargo.lock`, and `cargo metadata --all-features`, and
the mutation tests in `xtask/tests/mutation.rs` prove that guard fires.

The same four-layer vocabulary also rejects `ph-surfaces-bake` on the
**runtime** crate. `crates/surfaces/Cargo.toml` must not name it in any
dependency kind (including optional, path, Git, target-specific, or
dev/build). The runtime package's `Cargo.lock` entry and `cargo metadata`
graph must not resolve it. Do not add `{ name = "ph-surfaces-bake" }` to
`deny.toml` `[bans] deny`: that would deny the baker package itself. The
baker stays in the deny graph so its empty third-party tree is evidence.

Do not add a `gen` feature, optional dependency, or `cfg` on `ph-surfaces`
that reaches the baker. That shape is used by other org crates and is
forbidden here.

### 2. `#![no_std]` is unconditional

Never make it feature-conditional. Cargo unifies features across the whole
dependency graph, so `#![cfg_attr(not(feature = "..."), no_std)]` means any
unrelated crate enabling a host feature silently turns a firmware build into a
`std` build. The `no_std unconditional` check therefore also rejects a
`[features]` table and any `feature = "..."` cfg in `crates/surfaces/src/`; a
features table must arrive together with a proof that none of its members can
touch the attribute.

### 3. Core-only, no allocator, no unsafe

Do not introduce `alloc`, `std`, `unsafe`, or floating point. The `integer
only` check in `xtask` scans `crates/surfaces/src/` for those code paths,
ignoring full line comments so documentation may still discuss them. The float
and `ph-curves` greps also cover `crates/surfaces/tests/` and
`crates/surfaces/examples/`, so the conformance suite and Cargo examples
cannot acquire a floating-point or `ph-curves` oracle. Examples use a host
`main` only as an assertion harness: a separate examples guard rejects
allocator/std paths, common allocating prelude types and macros, host
printing/debug macros, and `unsafe` from their uncommented code.

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
reads). `crates/surfaces/src/surface.rs` and `crates/surfaces/src/axis/` tests
assert the storage figures without assuming a pointer width.

### 4. Local CI is authoritative

`cargo xtask ci` is the verification entry point. It reports `PASS`, `FAIL`,
and `SKIP` distinctly. A skipped check is not a passed check.

The repository is a virtual Cargo workspace: `crates/surfaces` (package
`ph-surfaces`), `crates/surfaces-bake` (package `ph-surfaces-bake`), and
`xtask` (the host gate). `default-members` includes the two shipped
packages and omits the gate, so a bare `cargo build` at the root operates
on shipped packages only. `[workspace.dependencies]` holds xtask host
dependencies and nothing a shipped crate uses. `publish_lock` classifies
every resolved workspace member — `ph-surfaces` and `ph-surfaces-bake`
publish only to crates.io, `xtask` stays `publish = false`, and an
unclassified member fails the gate. Declarative policy lives in
`xtask/config.ron`; reviewed host dependencies are locked in the root
`Cargo.lock`. Those tooling dependencies may use `std` and allocation;
they are isolated from the runtime crate by membership and publication
policy, not by a second lockfile. There is no shell script and no
PowerShell twin. `tools/consumer` and `tools/code-size` keep empty
`[workspace]` tables so they do not join the root workspace.

`crates/surfaces-bake/src` is capped at 1,500 lines of implementation,
excluding `#[cfg(test)]` tails (the same exemption as the integer-only
scanner), fixtures, and generated output directories. Exceeding the budget
is a FAIL, not a quiet raise.

`cargo xtask ci --profile release --nightly nightly-YYYY-MM-DD` is the
release-evidence mode: every check must run, a would-be `SKIP` is recorded as
`FAIL`, and package checks require a clean Git worktree, validate the packaged
commit provenance, and print a verified SHA-256 digest. The matrix includes both
debug and release test suites.

`cargo xtask ci --only '<check name>'` runs one check; `cargo xtask list` prints
the registry. The release profile rejects `--only` — a partial run is not
release evidence — and requires `--nightly nightly-YYYY-MM-DD`. `--skip-embedded`
drops the target-dependent checks, `--coverage` opts into a diagnostic
`cargo-llvm-cov` summary with no percentage threshold, and `--fail-fast` stops
at the first failure.
The `package` family builds the
`.crate`, asserts its exact file list, verifies its provenance and digest, and
compiles the downstream `#![no_std]` consumer in `tools/consumer` against it.
`guards fire on mutation` runs `xtask/tests/mutation.rs`, which mutates
copies of the tracked tree and requires the intended guard to fail. Every check
that needs an optional tool or target reports `SKIP`, with the reason, rather
than passing.

Hosted GitHub Actions run a bounded contributor subset. Local `cargo xtask ci`
remains the complete gate. The public workflow runs on pull requests and pushes
to `main`, calls that same xtask entry point, and exposes one aggregate `ci`
result for branch protection. A hosted failure must be resolved even though the
bounded hosted subset is not the complete release evidence.

## Coupled edits

| Change | Also update |
| --- | --- |
| Version or crate `publish` setting | Release process (`RELEASING.md`): root `Cargo.lock`, changelog heading date, `package.version` and `package.manifest.publish` in `xtask/config.ron`, and GitHub `Lifecycle`. README and crate-doc status already describe published `0.1.0` Active; do not revert them to incubating. Pin unpackaged guide URLs (README, `crates/surfaces/src/lib.rs`, `crates/surfaces/examples/*.rs`) from `main` to the release tag. |
| New packaged file | `include` in `crates/surfaces/Cargo.toml`, and crate-relative `package.files` in `xtask/config.ron` |
| New baker packaged file | `include` in `crates/surfaces-bake/Cargo.toml`, and crate-relative `baker.files` in `xtask/config.ron` |
| New guard in `xtask` | An `Action` variant and required-handler entry in `xtask/src/config.rs`, dispatch in `xtask/src/checks/mod.rs`, a row in `xtask/config.ron`, and a mutation case in `xtask/tests/mutation.rs` showing it fails |
| Storage or cost wording | `crates/surfaces/src/lib.rs` crate docs, `crates/surfaces/src/surface.rs` / `evaluate.rs` / `axis/` item docs, `README.md` "Resource accounting and cost" |
| New or changed axis strategy | `crates/surfaces/src/lib.rs` re-exports and § Contract, `README.md` "Per-axis lookup strategies" table, the sixteen-pairing consumer in `tools/consumer/src/lib.rs`, `docs/v0.1-traceability.md` |
| New runtime/dev/build dependency in the shipped crate | `deny.toml`, the no-`ph-curves` check, and an explicit reason in the PR. Host-only xtask dependencies stay in `[workspace.dependencies]` and must not appear on the shipped graph. |
| New or changed public API item | `crates/surfaces/src/lib.rs` module docs, `README.md` status sections, `CHANGELOG.md`, `docs/v0.1-traceability.md` |
| Example map values (`ELEVATION`, `CORRECTION`) | `crates/surfaces/tests/conformance/fixtures.rs` and `examples.rs`, `README.md` "Examples", `crates/surfaces/src/lib.rs` § Examples, `tools/consumer/src/lib.rs` |
| Firmware example fixtures (quickstart, uniform, mixed, fail-safe, cost) | `crates/surfaces/examples/*.rs`, `crates/surfaces/tests/conformance/fixtures.rs` and `examples.rs`, README "Start here", `docs/usage-guide.md` / `interpolation-walkthrough.md` / `choosing-a-strategy.md`, `tools/consumer/src/lib.rs`, and `examples` in `xtask/config.ron` when the Cargo example set changes |
| Contract wording or acceptance claim | `README.md` "Contract", `crates/surfaces/src/lib.rs` § Contract, `docs/v0.1-traceability.md` |
| New workspace member | An explicit `publish_lock` classification (`ph-surfaces` and `ph-surfaces-bake` → crates.io, `xtask` → locked, anything else fails by name) |

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
- `cargo xtask ci --skip-embedded` skips the two top-level ordinary embedded
  `cargo build` checks (`thumbv7em-none-eabi`, `riscv32imac-unknown-none-elf`)
  and the `code size snapshot`, which also needs those targets. It does **not**
  skip either nightly core-only `-Z build-std=core` proof or the
  packaged-consumer matrix; pass it only expecting a partial skip, not a fully
  host-only run.
