#!/usr/bin/env sh
# Canonical local CI for ph-surfaces.
#
# This script is the authoritative verification entry point.
#
# Hosted GitHub Actions are a known gap until this repository is public:
# private runs fail before any step starts, so the workflow has no
# pull_request/push triggers. When hosted CI exists, it is a subset and may
# skip deny, nightly core-only, and GitHub metadata.
# A green or missing hosted check is not a substitute for this run.
#
# There is no PowerShell twin. On Windows, use Git Bash.
#
# Every check runs even if an earlier one fails, then a summary is printed.
# Exit code is non-zero if any check failed. SKIP is not PASS.
#
# Usage:
#   ./scripts/ci.sh                          # full matrix
#   SKIP_EMBEDDED=1 ./scripts/ci.sh          # host checks only
#   FAIL_FAST=1 ./scripts/ci.sh              # stop at the first failure
#   CI_ONLY='no ph-curves' ./scripts/ci.sh   # run one named check
#
# What each group proves:
#   - fmt/check/test/clippy/doc: the host contract, with warnings denied.
#   - no_std unconditional / integer only / no ph-curves / manifest floor:
#     source and manifest ratchets that `cargo test` cannot observe.
#   - package list / package build: the exact packaged file set, the packaged
#     artifact building on its own, its rustdoc and doctests (README Rust blocks
#     included) passing from the unpacked package, and a downstream
#     `#![no_std]` consumer that declares and evaluates both documented
#     example maps compiling and testing against it.
#   - code size snapshot: `scripts/measure-code-size.sh` vs
#     `docs/code-size-snapshot.txt`. SKIP if a target or llvm-tools-preview is
#     missing. Not a source ratchet.
#   - guards fire on mutation: the three ratchets above are shown to fail on a
#     mutated copy of the tree (scripts/guard-selftest.sh).
#   - core-only <target>: nightly `-Z build-std=core` builds. These are the
#     no-alloc proof; a plain `--target` build is NOT, because bare-metal
#     `rust-std` sysroots still ship `alloc`.
#   - <target>: ordinary bare-metal builds on the pinned toolchain.

set -u

cd "$(dirname "$0")/.." || exit 1

CARGO_INCREMENTAL=0
export CARGO_INCREMENTAL

PACKAGE_NAME=ph-surfaces
PACKAGE_VERSION=0.1.0-incubating.1
CRATE_DIR="${PACKAGE_NAME}-${PACKAGE_VERSION}"
CRATE_FILE="target/package/${CRATE_DIR}.crate"
SCRATCH=target/ci-scratch

failed=0
skipped=0
summary=""

run_check() {
    name="$1"
    shift

    if [ -n "${CI_ONLY:-}" ] && [ "$CI_ONLY" != "$name" ]; then
        return 0
    fi

    printf '\n==> %s\n' "$name"

    "$@"
    status=$?

    if [ "$status" -eq 2 ]; then
        summary="${summary}  SKIP  ${name}\n"
        skipped=$((skipped + 1))
        return 0
    fi

    if [ "$status" -eq 0 ]; then
        summary="${summary}  PASS  ${name}\n"
    else
        summary="${summary}  FAIL  ${name}\n"
        failed=$((failed + 1))
        if [ "${FAIL_FAST:-0}" != "0" ]; then
            report
            exit 1
        fi
    fi
}

report() {
    printf '\nSummary\n'
    printf '%b' "$summary"
    if [ "$skipped" -gt 0 ]; then
        printf '\n%s check(s) SKIPPED. A skipped check is not a passed check.\n' "$skipped"
        printf 'Install the missing tool or target and re-run before treating this as verified.\n'
    fi
}

check_no_std_unconditional() {
    if grep -nE '^#!\[cfg_attr\(.*no_std' src/lib.rs; then
        printf 'src/lib.rs: #![no_std] is feature-conditional.\n' >&2
        return 1
    fi
    if ! grep -qxF '#![no_std]' src/lib.rs; then
        printf 'src/lib.rs must declare an unconditional #![no_std].\n' >&2
        return 1
    fi
    # Cargo unifies features across the graph, so the only way to be sure no
    # feature can relax `no_std` is for there to be no feature and no cfg on
    # one. A `[features]` table must arrive together with a proof that none of
    # its members touches the crate-level attribute.
    if grep -nE '^\[features\]' Cargo.toml; then
        printf 'Cargo.toml declares a [features] table; none exists on this crate.\n' >&2
        return 1
    fi
    code=$(source_without_comments)
    if printf '%s\n' "$code" | grep -nE 'cfg_attr\(.*no_std'; then
        printf 'src: no_std is under a cfg_attr somewhere.\n' >&2
        return 1
    fi
    if printf '%s\n' "$code" | grep -nE 'feature[[:space:]]*=[[:space:]]*"'; then
        printf 'src: a cfg names a feature, but this crate declares none.\n' >&2
        return 1
    fi
    return 0
}

# Every runtime source line that is not a full-line comment, in a stable order.
# Doc comments legitimately discuss floating point, allocation, features, and
# the banned crate by name; none of that is a code path.
source_without_comments() {
    find src -name '*.rs' -print0 \
        | sort -z \
        | xargs -0 cat \
        | grep -vE '^[[:space:]]*(//|/\*|\*)'
}

check_integer_only() {
    # The runtime is integer-only, core-only, and unsafe-free. `cargo test`
    # cannot observe the absence of a code path, so this grep is the mechanical
    # evidence for that claim; the nightly `-Z build-std=core` check below is
    # the matching proof for the allocator.
    #
    # Full-line comments are stripped first. Doc comments legitimately discuss
    # floating point, magnitude bounds such as 4.23e14, and the banned crate by
    # name, and none of that is a code path.
    #
    # The black-box conformance suite in `tests/` is held to the float and
    # `ph-curves` bans as well: its expected values must come from an integer
    # reference, and `ph-curves` is not an oracle. It runs under the std test
    # harness, so only `src/` is held to the alloc/std ban.
    code=$(source_without_comments)
    all_code=$(find src tests -name '*.rs' -print0 \
        | sort -z \
        | xargs -0 cat \
        | grep -vE '^[[:space:]]*(//|/\*|\*)')

    if printf '%s\n' "$all_code" | grep -nE '\bf32\b|\bf64\b'; then
        printf 'src/tests: code names a floating-point type.\n' >&2
        return 1
    fi
    # Cover every Rust float-literal shape without mistaking an integer range
    # such as `1..2` for a trailing-dot float. The surrounding identifier
    # boundaries keep digits embedded in names from matching.
    float_literal='(^|[^[:alnum:]_])([0-9][0-9_]*\.[0-9_]+([eE][+-]?[0-9_]+)?(_?(f32|f64))?|[0-9][0-9_]*[eE][+-]?[0-9_]+(_?(f32|f64))?|[0-9][0-9_]*_?(f32|f64))([^[:alnum:]_]|$)'
    trailing_dot_float='(^|[^[:alnum:]_])[0-9][0-9_]*\.([^[:alnum:]_.]|$)'
    if printf '%s\n' "$all_code" | grep -nE "$float_literal|$trailing_dot_float"; then
        printf 'src/tests: code contains a floating-point literal.\n' >&2
        return 1
    fi
    if printf '%s\n' "$code" | grep -nE '\balloc::|\bstd::|extern[[:space:]]+crate[[:space:]]+(alloc|std)'; then
        printf 'src: runtime code reaches for alloc or std.\n' >&2
        return 1
    fi
    if printf '%s\n' "$all_code" | grep -nE 'ph.curves'; then
        printf 'src/tests: code references ph-curves.\n' >&2
        return 1
    fi
    # A negative grep for `unsafe` would match this very declaration, so assert
    # the crate-level ban is present instead.
    if ! grep -qxF '#![forbid(unsafe_code)]' src/lib.rs; then
        printf 'src/lib.rs must declare #![forbid(unsafe_code)].\n' >&2
        return 1
    fi
    return 0
}

check_no_ph_curves() {
    # Three layers, each catching a form the others cannot:
    #   1. manifest text: an optional, target-specific, dev, build, path, Git,
    #      [patch], or [replace] entry is rejected before cargo resolves it,
    #      even when the path or Git source does not exist yet;
    #   2. Cargo.lock: any resolved package or source;
    #   3. cargo metadata with every feature enabled: every declared dependency
    #      of every kind (`packages[].dependencies[]` lists them regardless of
    #      feature or target) and every resolved package.
    # deny.toml bans the name as a fourth layer for the transitive case.
    manifests=Cargo.toml
    for extra in .cargo/config.toml .cargo/config; do
        [ -f "$extra" ] && manifests="$manifests $extra"
    done
    # Full-line TOML comments may explain the ban; a value may not name it.
    # shellcheck disable=SC2086
    if grep -HnEi 'ph[-_]curves' $manifests | grep -vE '^[^:]*:[0-9]+:[[:space:]]*#'; then
        printf 'Cargo.toml (or .cargo/config) names ph-curves outside a comment.\n' >&2
        return 1
    fi
    if [ ! -f Cargo.lock ]; then
        printf 'Cargo.lock is missing; the lockfile is part of the repository floor.\n' >&2
        return 1
    fi
    if grep -nEi 'ph[-_]curves' Cargo.lock; then
        printf 'Cargo.lock mentions a ph-curves package or source.\n' >&2
        return 1
    fi
    mkdir -p "$SCRATCH"
    metadata="$SCRATCH/metadata.json"
    if ! cargo metadata --format-version 1 --offline --all-features >"$metadata"; then
        printf 'cargo metadata failed.\n' >&2
        return 1
    fi
    if grep -Ei '"name":[[:space:]]*"ph[-_]curves"' "$metadata"; then
        printf 'cargo metadata declares or resolves a ph-curves package.\n' >&2
        return 1
    fi
    if grep -Ei 'ph[-_]curves' "$metadata"; then
        printf 'cargo metadata mentions ph-curves.\n' >&2
        return 1
    fi
    return 0
}

check_manifest_floor() {
    if grep -q '^\[workspace\]' Cargo.toml; then
        printf 'Cargo.toml must not declare a workspace before a second package exists.\n' >&2
        return 1
    fi
    if ! grep -Eq '^version = "0\.1\.0-incubating\.1"$' Cargo.toml; then
        printf 'Cargo.toml version must be 0.1.0-incubating.1.\n' >&2
        return 1
    fi
    if ! grep -Eq '^publish = false$' Cargo.toml; then
        printf 'Cargo.toml must keep publish = false until a separate release decision.\n' >&2
        return 1
    fi
    if ! grep -Eq '^license = "MIT"$' Cargo.toml; then
        printf 'Cargo.toml license must be MIT.\n' >&2
        return 1
    fi
    if ! grep -Eq '^edition = "2024"$' Cargo.toml; then
        printf 'Cargo.toml edition must be 2024.\n' >&2
        return 1
    fi
    if ! grep -Eq '^rust-version = "1\.92\.0"$' Cargo.toml; then
        printf 'Cargo.toml rust-version must be 1.92.0.\n' >&2
        return 1
    fi
    if ! metadata=$(cargo metadata --format-version 1 --offline --no-deps); then
        printf 'cargo metadata failed while checking dependency tables.\n' >&2
        return 1
    fi
    if ! printf '%s\n' "$metadata" | grep -q '"dependencies":\[\]'; then
        printf 'dependency tables must stay empty on this scaffold.\n' >&2
        return 1
    fi
    if ! grep -q 'Incubating' README.md; then
        printf 'README.md must record Lifecycle Incubating.\n' >&2
        return 1
    fi
    if ! grep -q '0.1.0-incubating.1' README.md; then
        printf 'README.md must record version 0.1.0-incubating.1.\n' >&2
        return 1
    fi
    if ! grep -q 'Libraries' README.md; then
        printf 'README.md must record Domain Libraries.\n' >&2
        return 1
    fi
    if ! grep -q 'MIT License' LICENSE; then
        printf 'LICENSE must be the MIT License.\n' >&2
        return 1
    fi
    return 0
}

check_package_list() {
    list=$(cargo package --list --allow-dirty | tr '\\' '/') || return 1
    printf '%s\n' "$list"
    for required in Cargo.toml LICENSE README.md \
        src/lib.rs src/interp.rs src/lookup.rs src/evaluate.rs src/boundary.rs \
        src/error.rs src/surface.rs src/axis/mod.rs src/axis/linear.rs \
        src/axis/binary.rs src/axis/uniform.rs src/axis/bucketed.rs; do
        if ! printf '%s\n' "$list" | grep -qx "$required"; then
            printf 'packaged crate is missing %s\n' "$required" >&2
            return 1
        fi
    done
    if printf '%s\n' "$list" | grep -E '^(AGENTS\.md|CHANGELOG\.md|clippy\.toml|deny\.toml|rust-toolchain\.toml|scripts/|docs/|tests/|\.github/)'; then
        printf 'packaged crate contains non-consumer artifacts.\n' >&2
        return 1
    fi
    return 0
}

# The exact file set the packaged artifact must contain, one per line, sorted.
# `.cargo_vcs_info.json`, `Cargo.lock`, and `Cargo.toml.orig` are added by
# cargo itself; everything else comes from the `include` allowlist.
expected_package_files() {
    printf '%s\n' \
        .cargo_vcs_info.json \
        Cargo.lock \
        Cargo.toml \
        Cargo.toml.orig \
        LICENSE \
        README.md \
        src/axis/binary.rs \
        src/axis/bucketed.rs \
        src/axis/linear.rs \
        src/axis/mod.rs \
        src/axis/uniform.rs \
        src/boundary.rs \
        src/error.rs \
        src/evaluate.rs \
        src/interp.rs \
        src/lib.rs \
        src/lookup.rs \
        src/surface.rs
}

check_package_build() {
    # 1. Build the .crate. cargo's own verify step unpacks it and builds it,
    #    so a source file missing from `include` fails here.
    cargo package --locked --allow-dirty || return 1
    if [ ! -f "$CRATE_FILE" ]; then
        printf 'expected %s to exist after cargo package.\n' "$CRATE_FILE" >&2
        return 1
    fi

    # 2. Inspect the archive itself, not `--list`: the file set must match
    #    exactly. A stray file is as much a failure as a missing one.
    mkdir -p "$SCRATCH"
    actual="$SCRATCH/package-files.txt"
    expected="$SCRATCH/package-files.expected.txt"
    tar tzf "$CRATE_FILE" | sed "s|^${CRATE_DIR}/||" | grep -v '/$' | sort >"$actual" || return 1
    expected_package_files | sort >"$expected"
    if ! diff -u "$expected" "$actual"; then
        printf 'packaged file set differs from the expected list (- expected, + actual).\n' >&2
        return 1
    fi
    printf 'packaged files:\n'
    cat "$actual"

    # 3. Unpack the artifact and prove its own documentation from the packaged
    #    files alone: rustdoc with warnings denied, and every doctest,
    #    including each README Rust code block (README.md ships and is compiled as
    #    doctests by src/lib.rs). The unpacked crate has no tests/ directory,
    #    so `cargo test` there is unit tests plus doctests only.
    consumer_root="$SCRATCH/package-consumer"
    rm -rf "$consumer_root"
    mkdir -p "$consumer_root/vendor" "$consumer_root/consumer/src"
    tar xzf "$CRATE_FILE" -C "$consumer_root/vendor" || return 1

    consumer_target_dir="$(pwd)/$consumer_root/target"
    (
        cd "$consumer_root/vendor/$CRATE_DIR" || exit 1
        CARGO_TARGET_DIR="$consumer_target_dir" RUSTDOCFLAGS='-D warnings' \
            cargo doc --offline --no-deps || exit 1
        CARGO_TARGET_DIR="$consumer_target_dir" cargo test --offline --doc || exit 1
    ) || return 1

    # 4. Compile and test a fresh downstream #![no_std] consumer against the
    #    unpacked artifact. It declares both documented device-neutral example
    #    maps as statics with a boundary policy, evaluates them, and its host
    #    tests assert the hand-computed points the README states. It is then
    #    built for each bare-metal target that is installed. This is what a
    #    firmware crate would do with the package.
    cat >"$consumer_root/consumer/Cargo.toml" <<EOF
[package]
name = "ph-surfaces-consumer"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
${PACKAGE_NAME} = { path = "../vendor/${CRATE_DIR}" }
EOF

    cat >"$consumer_root/consumer/src/lib.rs" <<'EOF'
//! Minimal downstream `no_std` consumer of the packaged crate: the two
//! documented device-neutral example maps, declared and evaluated.
#![no_std]
#![forbid(unsafe_code)]

use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

static ELEVATION_X: [u16; 5] = [0, 25, 60, 100, 180];
static ELEVATION_Y: [u16; 4] = [0, 40, 90, 150];
static ELEVATION_VALUES: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
static ELEVATION: BilinearSurface<5, 4> =
    BilinearSurface::new(&ELEVATION_X, &ELEVATION_Y, &ELEVATION_VALUES)
        .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));

static CORRECTION_X: [u16; 4] = [40, 55, 90, 200];
static CORRECTION_Y: [u16; 5] = [0, 10, 25, 70, 120];
static CORRECTION_VALUES: [[i32; 4]; 5] = [
    [125, 80, -15, -140],
    [90, 41, -33, -170],
    [30, -7, -61, -205],
    [-48, -95, -150, -260],
    [-110, -142, -199, -333],
];
static CORRECTION: BilinearSurface<4, 5> =
    BilinearSurface::new(&CORRECTION_X, &CORRECTION_Y, &CORRECTION_VALUES)
        .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));

/// Evaluates the elevation map; the caller sees the crate's error type.
pub fn elevation(x: u16, y: u16) -> Result<i32, SurfaceError> {
    ELEVATION.evaluate(x, y)
}

/// Evaluates the correction map; the caller sees the crate's error type.
pub fn correction(x: u16, y: u16) -> Result<i32, SurfaceError> {
    CORRECTION.evaluate(x, y)
}

/// Every X/Y pairing of the four compile-time lookup strategies, declared as
/// statics over one equivalent axis pair.
///
/// A firmware selects strategies in the type, so a pairing exists only where it
/// is named. Declaring all sixteen here is what makes them compile — and, when
/// this consumer is built for the bare-metal targets with a core-only sysroot,
/// link without an allocator.
pub mod strategies {
    use ph_surfaces::{
        BilinearSurface, BinaryAxis, BucketedAxis, LinearAxis, SurfaceError, UniformAxis,
        bucket_index,
    };

    static X: [u16; 5] = [0, 25, 50, 75, 100];
    static X_INDEX: [u16; 4] = bucket_index(&X);
    static Y: [u16; 3] = [0, 10, 20];
    static Y_INDEX: [u16; 2] = bucket_index(&Y);
    static VALUES: [[i32; 5]; 3] = [
        [0, 25, 50, 75, 100],
        [10, 35, 60, 85, 110],
        [-20, 5, 30, 55, 80],
    ];

    type Lx = LinearAxis<5>;
    type Bx = BinaryAxis<5>;
    type Ux = UniformAxis<5, 0, 25>;
    type Kx = BucketedAxis<5, 4>;

    type Ly = LinearAxis<3>;
    type By = BinaryAxis<3>;
    type Uy = UniformAxis<3, 0, 10>;
    type Ky = BucketedAxis<3, 2>;

    macro_rules! pairing {
        ($name:ident, $xt:ty, $yt:ty, $x:expr, $y:expr) => {
            static $name: BilinearSurface<5, 3, $xt, $yt> =
                BilinearSurface::from_axes($x, $y, &VALUES);
        };
    }

    pairing!(LL, Lx, Ly, LinearAxis::new(&X), LinearAxis::new(&Y));
    pairing!(LB, Lx, By, LinearAxis::new(&X), BinaryAxis::new(&Y));
    pairing!(LU, Lx, Uy, LinearAxis::new(&X), UniformAxis::new());
    pairing!(LK, Lx, Ky, LinearAxis::new(&X), BucketedAxis::new(&Y, &Y_INDEX));
    pairing!(BL, Bx, Ly, BinaryAxis::new(&X), LinearAxis::new(&Y));
    pairing!(BB, Bx, By, BinaryAxis::new(&X), BinaryAxis::new(&Y));
    pairing!(BU, Bx, Uy, BinaryAxis::new(&X), UniformAxis::new());
    pairing!(BK, Bx, Ky, BinaryAxis::new(&X), BucketedAxis::new(&Y, &Y_INDEX));
    pairing!(UL, Ux, Ly, UniformAxis::new(), LinearAxis::new(&Y));
    pairing!(UB, Ux, By, UniformAxis::new(), BinaryAxis::new(&Y));
    pairing!(UU, Ux, Uy, UniformAxis::new(), UniformAxis::new());
    pairing!(UK, Ux, Ky, UniformAxis::new(), BucketedAxis::new(&Y, &Y_INDEX));
    pairing!(KL, Kx, Ly, BucketedAxis::new(&X, &X_INDEX), LinearAxis::new(&Y));
    pairing!(KB, Kx, By, BucketedAxis::new(&X, &X_INDEX), BinaryAxis::new(&Y));
    pairing!(KU, Kx, Uy, BucketedAxis::new(&X, &X_INDEX), UniformAxis::new());
    pairing!(
        KK,
        Kx,
        Ky,
        BucketedAxis::new(&X, &X_INDEX),
        BucketedAxis::new(&Y, &Y_INDEX)
    );

    /// Evaluates all sixteen pairings at the same point, in a fixed order.
    pub fn every_pairing(x: u16, y: u16) -> [Result<i32, SurfaceError>; 16] {
        [
            LL.evaluate(x, y),
            LB.evaluate(x, y),
            LU.evaluate(x, y),
            LK.evaluate(x, y),
            BL.evaluate(x, y),
            BB.evaluate(x, y),
            BU.evaluate(x, y),
            BK.evaluate(x, y),
            UL.evaluate(x, y),
            UB.evaluate(x, y),
            UU.evaluate(x, y),
            UK.evaluate(x, y),
            KL.evaluate(x, y),
            KB.evaluate(x, y),
            KU.evaluate(x, y),
            KK.evaluate(x, y),
        ]
    }

    /// Evaluates the default binary surface over the same tables.
    pub fn default_surface(x: u16, y: u16) -> Result<i32, SurfaceError> {
        static DEFAULT: BilinearSurface<5, 3> = BilinearSurface::new(&X, &Y, &VALUES);

        DEFAULT.evaluate(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::{correction, elevation};
    use ph_surfaces::SurfaceError;

    #[test]
    fn elevation_matches_the_documented_points() {
        assert_eq!(elevation(60, 90), Ok(130));
        assert_eq!(elevation(10, 20), Ok(-65));
        assert_eq!(elevation(75, 100), Ok(109));
        assert_eq!(elevation(140, 60), Ok(31));
        assert_eq!(elevation(u16::MAX, 0), Ok(-60));
        assert_eq!(
            elevation(500, 151),
            Err(SurfaceError::YAbove { coordinate: 151, bound: 150 })
        );
    }

    #[test]
    fn every_strategy_pairing_agrees_with_the_default_surface() {
        use super::strategies;

        for x in [0u16, 1, 24, 25, 60, 99, 100] {
            for y in [0u16, 5, 10, 19, 20] {
                let expected = strategies::default_surface(x, y);
                for (index, actual) in strategies::every_pairing(x, y).iter().enumerate() {
                    assert_eq!(*actual, expected, "pairing {index} at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn cost_constants_on_the_default_binary_and_a_mixed_pairing() {
        use ph_surfaces::{BilinearSurface, BucketedAxis, UniformAxis};

        assert_eq!(BilinearSurface::<5, 4>::VALUE_BYTES, 80);
        assert_eq!(BilinearSurface::<5, 4>::PAYLOAD_BYTES, 98);
        assert_eq!(BilinearSurface::<5, 4>::SUCCESS_INTERPOLATIONS, 3);
        assert_eq!(BilinearSurface::<5, 4>::SUCCESS_GRID_READS, 4);

        type Mixed = BilinearSurface<5, 3, BucketedAxis<5, 8>, UniformAxis<3, 0, 50>>;
        assert_eq!(Mixed::VALUE_BYTES, 60);
        assert_eq!(Mixed::PAYLOAD_BYTES, 86);
        assert_eq!(Mixed::SUCCESS_INTERPOLATIONS, 3);
        assert_eq!(Mixed::SUCCESS_GRID_READS, 4);
    }

    #[test]
    fn correction_matches_the_documented_points() {
        assert_eq!(correction(47, 5), Ok(86));
        assert_eq!(correction(145, 100), Ok(-242));
        assert_eq!(correction(60, 40), Ok(-44));
        assert_eq!(correction(90, u16::MAX), Ok(-199));
        assert_eq!(
            correction(39, 500),
            Err(SurfaceError::XBelow { coordinate: 39, bound: 40 })
        );
    }
}
EOF

    (
        cd "$consumer_root/consumer" || exit 1
        CARGO_TARGET_DIR="$consumer_target_dir" cargo build --offline || exit 1
        CARGO_TARGET_DIR="$consumer_target_dir" cargo test --offline || exit 1
        # The consumer's fresh lockfile may name only the two packages: the
        # packaged crate really has no runtime dependency.
        if grep -E '^name = ' Cargo.lock | grep -vE "^name = \"(${PACKAGE_NAME}|ph-surfaces-consumer)\"$"; then
            printf 'the downstream consumer resolved an unexpected package.\n' >&2
            exit 1
        fi
        # A core-only consumer build is what proves the sixteen strategy
        # pairings themselves are allocation-free: the library's own core-only
        # build compiles no instantiation of them, because a generic axis
        # strategy is only monomorphised where a surface names it.
        core_only=0
        if rustup toolchain list 2>/dev/null | grep -q '^nightly' &&
            rustup component list --toolchain nightly --installed 2>/dev/null |
            grep -q '^rust-src'; then
            core_only=1
        else
            printf 'consumer: nightly with rust-src missing; core-only pairing proof skipped\n'
        fi

        missing_target=0
        for target in thumbv7em-none-eabi riscv32imac-unknown-none-elf; do
            if rustup target list --installed 2>/dev/null | grep -qx "$target"; then
                CARGO_TARGET_DIR="$consumer_target_dir" \
                    cargo build --offline --target "$target" || exit 1
                if [ "$core_only" -eq 1 ]; then
                    CARGO_TARGET_DIR="$consumer_target_dir" \
                        cargo +nightly build --offline --target "$target" \
                        -Z build-std=core || exit 1
                fi
            else
                printf 'consumer: target %s not installed; package target proof skipped\n' "$target"
                missing_target=1
            fi
        done
        if [ "$missing_target" -ne 0 ] || [ "$core_only" -eq 0 ]; then
            exit 2
        fi
    )
    consumer_status=$?
    case "$consumer_status" in
        0) return 0 ;;
        2) return 2 ;;
        *) return 1 ;;
    esac
}

check_code_size_snapshot() {
    mkdir -p "$SCRATCH"
    sh scripts/measure-code-size.sh >"$SCRATCH/code-size-snapshot.txt"
    status=$?
    case "$status" in
        0)
            if ! diff -u docs/code-size-snapshot.txt "$SCRATCH/code-size-snapshot.txt"; then
                printf 'code-size snapshot differs from docs/code-size-snapshot.txt.\n' >&2
                printf 'Re-run scripts/measure-code-size.sh and commit the output.\n' >&2
                return 1
            fi
            return 0
            ;;
        2) return 2 ;;
        *) return 1 ;;
    esac
}

check_guard_selftest() {
    sh scripts/guard-selftest.sh
}

check_deny() {
    if ! command -v cargo-deny >/dev/null 2>&1; then
        printf 'cargo-deny not installed; skipping (cargo install cargo-deny)\n'
        return 2
    fi
    cargo deny check
}

check_core_only() {
    target=$1
    if [ "${SKIP_EMBEDDED:-0}" != "0" ]; then
        printf 'SKIP_EMBEDDED=1; skipping core-only %s\n' "$target"
        return 2
    fi
    if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
        printf 'nightly toolchain not installed; skipping (rustup toolchain install nightly)\n'
        return 2
    fi
    if ! rustup component list --toolchain nightly --installed 2>/dev/null | grep -q '^rust-src'; then
        printf 'nightly rust-src not installed; skipping (rustup component add rust-src --toolchain nightly)\n'
        return 2
    fi
    # `-Z build-std=core` builds a sysroot containing only `core` (and
    # `compiler_builtins`), so any `alloc` or `std` reference fails to
    # resolve. That absence is the no-alloc proof; a plain `--target` build
    # against the shipped `rust-std` sysroot proves nothing here.
    cargo +nightly build --locked --target "$target" -Z build-std=core
}

check_embedded_target() {
    target=$1
    if [ "${SKIP_EMBEDDED:-0}" != "0" ]; then
        printf 'SKIP_EMBEDDED=1; skipping target %s\n' "$target"
        return 2
    fi
    if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
        printf 'target %s not installed; skipping (rustup target add %s)\n' "$target" "$target"
        return 2
    fi
    cargo build --target "$target" --locked
}

check_github_metadata() {
    if ! command -v gh >/dev/null 2>&1; then
        printf 'gh not installed; skipping GitHub topics/properties check\n'
        return 2
    fi
    if ! topics=$(gh api repos/photon-circus/ph-surfaces --jq '.topics | join(",")' 2>/dev/null); then
        printf 'GitHub topics are not readable with this token; skipping\n'
        return 2
    fi
    printf 'topics: %s\n' "$topics"
    if [ -z "$topics" ]; then
        printf 'GitHub topics are unset; skipping (set: rust, embedded, no-std, no-alloc, interpolation)\n'
        return 2
    fi
    missing=
    for topic in rust embedded no-std no-alloc interpolation; do
        case ",$topics," in
            *",$topic,"*) ;;
            *) missing="$missing $topic" ;;
        esac
    done
    if [ -n "$missing" ]; then
        printf 'missing GitHub topics:%s\n' "$missing" >&2
        return 1
    fi
    if ! props=$(gh api repos/photon-circus/ph-surfaces/properties/values \
        --jq '.[] | [.property_name, (.value // "")] | @tsv' 2>/dev/null); then
        printf 'GitHub custom properties are not readable with this token; skipping\n'
        return 2
    fi
    printf '%s\n' "$props"

    lifecycle=$(printf '%s\n' "$props" | awk -F '\t' '
        $1 == "Lifecycle" { print $2; exit }
    ')
    domain=$(printf '%s\n' "$props" | awk -F '\t' '
        $1 == "Domain" { print $2; exit }
    ')

    properties_unset=0
    if [ -z "$lifecycle" ]; then
        printf 'Lifecycle custom property is unset; skipping unset properties\n'
        properties_unset=1
    elif [ "$lifecycle" != "Incubating" ]; then
        printf 'Lifecycle custom property must be Incubating, found: %s\n' "$lifecycle" >&2
        return 1
    fi
    if [ -z "$domain" ]; then
        printf 'Domain custom property is unset; skipping unset properties\n'
        properties_unset=1
    elif [ "$domain" != "Libraries" ]; then
        printf 'Domain custom property must be Libraries, found: %s\n' "$domain" >&2
        return 1
    fi
    if [ "$properties_unset" -ne 0 ]; then
        return 2
    fi
    return 0
}

run_check 'fmt' cargo fmt --all -- --check
run_check 'check' cargo check --locked
run_check 'test' cargo test --locked
run_check 'clippy' cargo clippy --locked --all-targets -- -D warnings
run_check 'doc' env RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
run_check 'no_std unconditional' check_no_std_unconditional
run_check 'integer only' check_integer_only
run_check 'no ph-curves' check_no_ph_curves
run_check 'manifest floor' check_manifest_floor
run_check 'package list' check_package_list
run_check 'package build' check_package_build
run_check 'code size snapshot' check_code_size_snapshot
run_check 'guards fire on mutation' check_guard_selftest
run_check 'github metadata' check_github_metadata
run_check 'deny' check_deny

run_check 'core-only thumbv7em-none-eabi' check_core_only thumbv7em-none-eabi
run_check 'core-only riscv32imac-unknown-none-elf' check_core_only riscv32imac-unknown-none-elf
run_check 'thumbv7em-none-eabi' check_embedded_target thumbv7em-none-eabi
run_check 'riscv32imac-unknown-none-elf' check_embedded_target riscv32imac-unknown-none-elf

report

if [ "$failed" -gt 0 ]; then
    printf '\n%s check(s) failed.\n' "$failed"
    exit 1
fi

printf '\nAll runnable checks passed.\n'
if [ "$skipped" -gt 0 ]; then
    exit 0
fi
