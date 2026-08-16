# ph-surfaces

Deterministic `no_std`, no-alloc integer surface mappings for embedded Rust.

[![Lifecycle: Incubating](https://img.shields.io/badge/lifecycle-incubating-orange.svg)](https://github.com/photon-circus/.github/blob/main/REPOSITORY_STANDARDS.md#31-lifecycle-values)
[![MSRV](https://img.shields.io/badge/MSRV-1.92.0-blue.svg)](Cargo.toml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **Lifecycle:** Incubating — the responsibility is bounded and intended to
> become supported. Compatibility follows the documented version and release
> policy, not lifecycle alone.
> **Distribution:** Unpublished. `publish = false`. Version `0.1.0-incubating.1`.
> **Domain:** Libraries.

This repository is a private Incubating Libraries project. It exposes the
validated static surface representation, its deterministic X-then-Y evaluator,
and its boundary and error vocabulary. There is no crates.io publication and no
docs.rs page.

## What this is

A reusable math crate for evaluating static rectilinear two-dimensional integer
surfaces on embedded firmware. The accepted v0.1 destination is:

> Evaluate a static rectilinear two-dimensional `u16 × u16 → i32` surface with
> deterministic X-then-Y bilinear interpolation, four independent Error/Clamp
> boundary sides, no allocation, and no floating point at runtime.

`BilinearSurface::evaluate` implements that mapping today. The order is part of
the contract: X is resolved before Y, so the X-side error wins when both
coordinates leave the domain, and the value is composed by interpolating along X
on each of the two Y rows and then interpolating those two already-rounded
results along Y. Because every step rounds to nearest with exact half-way values
away from zero, a Y-then-X composition would return different values.

```rust
use ph_surfaces::BilinearSurface;

static X: [u16; 3] = [0, 10, 30];
static Y: [u16; 2] = [0, 100];
static VALUES: [[i32; 3]; 2] = [[0, 10, 30], [100, 110, 130]];

static SURFACE: BilinearSurface<3, 2> = BilinearSurface::new(&X, &Y, &VALUES);

fn main() {
    assert_eq!(SURFACE.evaluate(10, 100), Ok(110)); // a declared knot
    assert_eq!(SURFACE.evaluate(20, 50), Ok(70)); // an interior point
}
```

## What it is for

Firmware that needs a device-neutral, allocation-free mapping from two `u16`
axes onto an `i32` value — for example multidimensional compensation — without
taking a dependency on `ph-curves` or pulling in host tooling.

## What state it is in

Incubating and unpublished. The package, license, lockfile, dependency policy,
and canonical CI exist. The public `BilinearSurface<NX, NY>` representation, the
`Boundary` / `BoundaryPolicy` policy vocabulary, `SurfaceError`, the private
scalar interpolation helper, the private binary axis lookup with four-sided
boundary handling, and the public `BilinearSurface::evaluate` all exist. Still
outstanding for v0.1: the black-box conformance suite, the mechanical
dependency, `no_std`, no-allocation, storage, and target proofs, and the
documentation and package-readiness gate.

## Responsibility

`ph-surfaces` owns static multidimensional mapping mechanics: shape and
invariant validation, axis location, explicit domain policies, deterministic
integer interpolation, and truthful resource and evidence accounting.

## Out of scope

It does not own hardware access, sensor configuration, sampling, clocks,
persistence, calibration discovery, fault or application policy, device
lifecycle, vendor catalogs, or total measurement accuracy.

v0.1 also does not include:

- A dependency on `ph-curves` or extraction of a shared arithmetic crate
- Inverse lookup or solving for either axis
- Arbitrary N-dimensional tensors, signed or wider axes, or generic outputs
- Scattered points, triangulation, irregular meshes, bicubic interpolation,
  extrapolation, or adaptive fitting
- Dynamic grids, runtime mutation, caching, allocation, unsafe code, or
  floating point
- Runtime semantic metadata, units, provenance, or generated error reports
- Host generation, CLI tooling, formula ingestion, or numerical fitting
- Device-specific equations, source catalogs, filtering, fusion, scheduling,
  buses, GPIO, async, or storage

## Constraints

- Unconditional `#![no_std]`; core-only runtime; `unsafe` is forbidden
- No `[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]` on this
  scaffold, and none of those tables may name `ph-curves` later either
- MSRV and toolchain pin: Rust 1.92.0, edition 2024
- Version `0.1.0-incubating.1` with `publish = false` until a separate release
  decision

## Repository classification

These GitHub fields must agree with the manifest and this README. The bootstrap
token cannot write them; set them on the repository if they are empty.

| Field | Value |
| --- | --- |
| Custom property `Lifecycle` | `Incubating` |
| Custom property `Domain` | `Libraries` |
| Topics | `rust`, `embedded`, `no-std`, `no-alloc`, `interpolation` |

## How it is verified

The canonical entry point is local:

```sh
./scripts/ci.sh
```

That script reports each check as `PASS`, `FAIL`, or `SKIP`. A skipped check is
not a passed check. Local `./scripts/ci.sh` is authoritative.

Hosted GitHub Actions are a **known gap until this repository is public**:
private runs fail before any step starts, so `pull_request` / `push` triggers
are not enabled. The workflow file remains for a manual `workflow_dispatch`
after the repository is public; it is a subset and may still skip deny,
nightly core-only, extra embedded targets, and GitHub metadata.
