# Contributing to ph-surfaces

Thank you for helping improve `ph-surfaces`. This crate owns one narrow
responsibility: deterministic, allocation-free two-dimensional integer surface
evaluation for embedded Rust. Contributions should strengthen that boundary
without adding application policy, hardware access, runtime table generation,
floating point, allocation, or a dependency on `ph-curves`.

By participating, you agree to follow the repository's
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Never put vulnerability details in
a public issue; follow [`SECURITY.md`](SECURITY.md).

## Before opening a change

Read `README.md` and `AGENTS.md`, especially the hard invariants and coupled
edit table. Search the existing issues before opening a new one.

- Focused bug fixes may go directly to a pull request when the failure and
  expected behavior are clear.
- Public API changes, new lookup strategies, dependencies, features, or
  changes to rounding/boundary semantics need an issue with an accepted
  contract and evidence plan first.
- Keep tagging, crates.io upload, yank, GitHub Release, and GitHub settings
  out of an implementation pull request unless its issue explicitly includes
  them. Version, `publish`, and changelog close-out belong to the release
  process in [`RELEASING.md`](RELEASING.md).

A useful issue or pull request states the affected commit/version, the smallest
reproduction, expected and observed behavior, target/toolchain, and whether the
evidence came from tests, code review, simulation, or hardware.

## Development environment

The repository pins its supported Rust toolchain in `rust-toolchain.toml`.
The complete local gate additionally needs:

- `cargo-deny`;
- `thumbv7em-none-eabi` and `riscv32imac-unknown-none-elf`;
- a nightly toolchain with `rust-src` for ordinary development, and a dated
  nightly for release evidence;
- `llvm-tools-preview` (declared by `rust-toolchain.toml`) for the code-size
  and instruction snapshots;
- `gitleaks` for the full-history secret scan (`SKIP` without it; required
  for release evidence).

Start with focused tests while developing. Before requesting review, run the
ordinary canonical gate:

```sh
cargo xtask ci
```

For a Rust behavior change, also run the release profile explicitly:

```sh
cargo test --locked --release
```

If a required prerequisite is unavailable, report the resulting `SKIP`; do not
describe it as a pass. Release-candidate pull requests must additionally use a
reviewed dated nightly and provide a clean zero-skip evidence run:

```sh
cargo xtask ci --profile release --nightly nightly-YYYY-MM-DD
```

## Tests and evidence

- Put implementation-focused unit tests beside the source under
  `crates/surfaces/src/`.
- Keep `crates/surfaces/tests/conformance/` black-box: use only public
  `ph_surfaces::*` APIs and
  the independent integer reference already present there.
- Keep every Rust README block runnable; the README is included in doctests.
- Preserve all sixteen lookup-strategy pairings and both ordinary and
  core-only embedded target builds.
- Add a guard mutation case in `xtask/tests/mutation.rs` whenever a new
  source or manifest invariant is added to `xtask`.
- Do not introduce float, allocator, host-I/O, or `ph-curves` test or example
  paths to make expected values easier to calculate.

## Documentation and changelog

Update every surface named by the coupled-edit table in `AGENTS.md`. Add a
concise entry under `CHANGELOG.md`'s `Unreleased` section for observable fixes,
public documentation changes, gate changes, and API decisions. Keep resource
claims exact: referenced payload, handle size, linked code, stack, comparisons,
cycles, and WCET are different quantities.

## Pull requests

Keep a pull request independently reviewable. Its description should include:

- the problem, root cause, and bounded solution;
- public API/behavior and compatibility effects;
- exact validation commands and results, including every skip;
- documentation/changelog changes;
- remaining risks and linked follow-up issues;
- confirmation that no release or repository-setting mutation is included
  unless explicitly in scope.

Do not commit generated `target/` content or credentials. Hosted GitHub
Actions run a bounded contributor subset of the local gate. Maintainers may
ask for the complete `cargo xtask ci` matrix, including every skip, even when
the hosted `ci` check is green.
