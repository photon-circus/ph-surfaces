#!/usr/bin/env sh
# Canonical local CI for ph-surfaces.
#
# This script is the authoritative verification entry point.
#
# Hosted GitHub Actions are a known gap until this repository is public:
# private runs fail before any step starts, so the workflow has no
# pull_request/push triggers. When hosted CI exists, it is a subset and may
# skip deny, nightly core-only, extra embedded targets, and GitHub metadata.
# A green or missing hosted check is not a substitute for this run.
#
# There is no PowerShell twin. On Windows, use Git Bash.
#
# Every check runs even if an earlier one fails, then a summary is printed.
# Exit code is non-zero if any check failed. SKIP is not PASS.
#
# Usage:
#   ./scripts/ci.sh                 # full matrix
#   SKIP_EMBEDDED=1 ./scripts/ci.sh # host checks only
#   FAIL_FAST=1 ./scripts/ci.sh     # stop at the first failure

set -u

cd "$(dirname "$0")/.." || exit 1

CARGO_INCREMENTAL=0
export CARGO_INCREMENTAL

failed=0
skipped=0
summary=""

run_check() {
    name="$1"
    shift

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
    return 0
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
    code=$(find src -name '*.rs' -print0 \
        | sort -z \
        | xargs -0 cat \
        | grep -vE '^[[:space:]]*(//|/\*|\*)')

    if printf '%s\n' "$code" | grep -nE '\bf32\b|\bf64\b'; then
        printf 'src: runtime code names a floating-point type.\n' >&2
        return 1
    fi
    # Cover every Rust float-literal shape without mistaking an integer range
    # such as `1..2` for a trailing-dot float. The surrounding identifier
    # boundaries keep digits embedded in names from matching.
    float_literal='(^|[^[:alnum:]_])([0-9][0-9_]*\.[0-9_]+([eE][+-]?[0-9_]+)?(_?(f32|f64))?|[0-9][0-9_]*[eE][+-]?[0-9_]+(_?(f32|f64))?|[0-9][0-9_]*_?(f32|f64))([^[:alnum:]_]|$)'
    trailing_dot_float='(^|[^[:alnum:]_])[0-9][0-9_]*\.([^[:alnum:]_.]|$)'
    if printf '%s\n' "$code" | grep -nE "$float_literal|$trailing_dot_float"; then
        printf 'src: runtime code contains a floating-point literal.\n' >&2
        return 1
    fi
    if printf '%s\n' "$code" | grep -nE '\balloc::|\bstd::|extern[[:space:]]+crate[[:space:]]+(alloc|std)'; then
        printf 'src: runtime code reaches for alloc or std.\n' >&2
        return 1
    fi
    if printf '%s\n' "$code" | grep -nE 'ph.curves'; then
        printf 'src: runtime code references ph-curves.\n' >&2
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
    if [ ! -f Cargo.lock ]; then
        printf 'Cargo.lock is missing; the lockfile is part of the repository floor.\n' >&2
        return 1
    fi
    if grep -E '^name = "ph-curves"' Cargo.lock; then
        printf 'Cargo.lock contains a ph-curves package.\n' >&2
        return 1
    fi
    if grep -E 'ph-curves' Cargo.lock | grep -E 'source|name'; then
        printf 'Cargo.lock mentions a ph-curves source or package.\n' >&2
        return 1
    fi
    if ! cargo metadata --format-version 1 --offline >/tmp/ph-surfaces-metadata.json; then
        printf 'cargo metadata failed.\n' >&2
        return 1
    fi
    if grep -E '"name":[[:space:]]*"ph-curves"' /tmp/ph-surfaces-metadata.json; then
        printf 'cargo metadata contains a ph-curves package.\n' >&2
        return 1
    fi
    if grep -E 'ph-curves' /tmp/ph-surfaces-metadata.json | grep -E 'source|git'; then
        printf 'cargo metadata contains a ph-curves source.\n' >&2
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
    list=$(cargo package --list --allow-dirty) || return 1
    printf '%s\n' "$list"
    for required in Cargo.toml LICENSE README.md \
        src/lib.rs src/interp.rs src/boundary.rs src/error.rs src/surface.rs; do
        if ! printf '%s\n' "$list" | grep -qx "$required"; then
            printf 'packaged crate is missing %s\n' "$required" >&2
            return 1
        fi
    done
    if printf '%s\n' "$list" | grep -E '^(AGENTS\.md|CHANGELOG\.md|clippy\.toml|deny\.toml|rust-toolchain\.toml|scripts/|\.github/)'; then
        printf 'packaged crate contains non-consumer artifacts.\n' >&2
        return 1
    fi
    return 0
}

check_core_only() {
    if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
        printf 'nightly toolchain not installed; skipping (rustup toolchain install nightly)\n'
        return 2
    fi
    if ! rustup component list --toolchain nightly --installed 2>/dev/null | grep -q '^rust-src'; then
        printf 'nightly rust-src not installed; skipping (rustup component add rust-src --toolchain nightly)\n'
        return 2
    fi
    cargo +nightly build --locked --target thumbv7em-none-eabi -Z build-std=core
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
    cargo check --target "$target" --locked
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
run_check 'github metadata' check_github_metadata

if command -v cargo-deny >/dev/null 2>&1; then
    run_check 'deny' cargo deny check
else
    printf '\n==> deny\ncargo-deny not installed; skipping (cargo install cargo-deny)\n'
    summary="${summary}  SKIP  deny (not installed)\n"
    skipped=$((skipped + 1))
fi

run_check 'core-only thumbv7em-none-eabi' check_core_only
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
