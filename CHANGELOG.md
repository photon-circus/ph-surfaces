# Changelog

## Unreleased

### Added

- Independent core-only `ph-surfaces` crate and repository floor at
  `0.1.0-incubating.1` (`publish = false`). No public mapping API yet.
- Private `u16`-domain, `i32`-range scalar segment interpolation in
  `src/interp.rs`, using 64-bit integer arithmetic and rounding to nearest with
  exact half-way values away from zero. One local division helper is the sole
  implementation of that rounding policy. The helper is crate private and does
  not change the public API.
- An `integer only` check in `scripts/ci.sh` that fails if runtime code
  acquires a floating-point, allocator, `std`, or `ph-curves` path, and that
  asserts the crate-level `#![forbid(unsafe_code)]` is still in place.

### Known issues

- Hosted GitHub Actions fail while the repository is private (no usable
  hosted runner before steps run). Verification is local `./scripts/ci.sh`
  until the repository is public.
