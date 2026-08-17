#!/usr/bin/env sh
# Guard self-test for ph-surfaces.
#
# A guard that has never been seen to fail is not evidence. This script copies
# the tracked tree into target/, applies one forbidden mutation per copy, and
# requires the intended `scripts/ci.sh` check to FAIL on that copy. The working
# tree is never modified.
#
# Cases:
#   no_std-conditional  #![no_std] becomes feature-conditional
#                       -> 'no_std unconditional' must fail
#   alloc               runtime code takes an allocator path
#                       -> 'integer only' must fail, and when nightly rust-src
#                          is present 'core-only thumbv7em-none-eabi' must fail
#   example-float       a Cargo example acquires a floating-point path
#                       -> 'integer only' must fail
#   example-host-path   a Cargo example acquires a std/allocator path
#                       -> 'integer only' must fail
#   ph-curves           Cargo.toml gains a ph-curves dependency
#                       -> 'no ph-curves' must fail
#   strict-skip         an ordinary embedded build is deliberately skipped
#                       -> REQUIRE_NO_SKIPS must turn that SKIP into a failure
#   release-provenance  a tracked-file-only copy has no Git provenance
#                       -> strict 'package list' must fail before packaging
#
# Usage:
#   ./scripts/guard-selftest.sh
#   (also run by ./scripts/ci.sh as 'guards fire on mutation')

set -u

cd "$(dirname "$0")/.." || exit 1

root=target/guard-selftest
failed=0

# make_copy <case>: a fresh copy of every tracked file (plus Cargo.lock) at
# $root/<case>, sharing this repository's target dir so the checks compile
# nothing that has not already been compiled.
make_copy() {
    dir="$root/$1"
    rm -rf "$dir"
    mkdir -p "$dir"
    git ls-files -z | xargs -0 tar cf - | tar xf - -C "$dir" || return 1
    [ -f Cargo.lock ] && cp Cargo.lock "$dir/Cargo.lock"
    return 0
}

# expect_fail <case> <check name> [NAME=VALUE ...]: run one named check on the
# mutated copy with any additional environment assignments and require it to
# fail. Its output is captured and shown only when the guard did not fire, so a
# passing self-test stays short.
expect_fail() {
    case_name=$1
    check=$2
    shift 2
    dir="$root/$case_name"
    log="$dir/$(printf '%s' "$check" | tr ' /' '__').log"

    if (cd "$dir" && env "$@" CI_ONLY="$check" CARGO_TARGET_DIR="$OLDPWD/target" \
        sh scripts/ci.sh) >"$log" 2>&1; then
        printf '  FAIL  %-20s guard "%s" did NOT fire\n' "$case_name" "$check"
        sed 's/^/        /' "$log"
        failed=$((failed + 1))
        return 1
    fi
    printf '  PASS  %-20s guard "%s" fired\n' "$case_name" "$check"
    return 0
}

# rewrite <file> <sed script>: portable in-place sed (no GNU `-i`).
rewrite() {
    sed "$2" "$1" >"$1.tmp" && mv "$1.tmp" "$1"
}

nightly_core_available() {
    rustup toolchain list 2>/dev/null | grep -q '^nightly' \
        && rustup component list --toolchain nightly --installed 2>/dev/null | grep -q '^rust-src' \
        && rustup target list --installed 2>/dev/null | grep -qx thumbv7em-none-eabi
}

printf 'guard self-test: mutating copies under %s\n' "$root"

# --- no_std made feature-conditional ---------------------------------------
if make_copy no_std-conditional; then
    rewrite "$root/no_std-conditional/src/lib.rs" \
        's|^#!\[no_std\]$|#![cfg_attr(not(feature = "std"), no_std)]|'
    if grep -qxF '#![no_std]' "$root/no_std-conditional/src/lib.rs"; then
        printf '  FAIL  no_std-conditional  mutation was not applied\n'
        failed=$((failed + 1))
    else
        expect_fail no_std-conditional 'no_std unconditional'
    fi
else
    failed=$((failed + 1))
fi

# --- an allocator path in runtime code -------------------------------------
if make_copy alloc; then
    cat >>"$root/alloc/src/lib.rs" <<'EOF'

extern crate alloc;

/// A deliberate allocator path; the guards must reject it.
pub fn leak() -> alloc::vec::Vec<u8> {
    alloc::vec::Vec::new()
}
EOF
    expect_fail alloc 'integer only'
    if nightly_core_available; then
        expect_fail alloc 'core-only thumbv7em-none-eabi'
    else
        printf '  SKIP  %-20s nightly rust-src or thumbv7em-none-eabi missing; build-std half not exercised\n' alloc
    fi
else
    failed=$((failed + 1))
fi

# --- a floating-point path in a Cargo example -------------------------------
if make_copy example-float; then
    cat >>"$root/example-float/examples/firmware_quickstart.rs" <<'EOF'

fn forbidden_float() {
    let _: f32 = 0.0;
}
EOF
    expect_fail example-float 'integer only'
else
    failed=$((failed + 1))
fi

# --- a host-only allocator path in a Cargo example --------------------------
if make_copy example-host-path; then
    cat >>"$root/example-host-path/examples/firmware_quickstart.rs" <<'EOF'

fn forbidden_host_path() -> std::vec::Vec<u8> {
    std::vec::Vec::new()
}
EOF
    expect_fail example-host-path 'integer only'
else
    failed=$((failed + 1))
fi

# --- a ph-curves dependency in the manifest --------------------------------
if make_copy ph-curves; then
    # A path dependency that does not exist: cargo could not even resolve it,
    # so the manifest-text layer of the guard is what must fire.
    rewrite "$root/ph-curves/Cargo.toml" \
        's|^\[dependencies\]$|[dependencies]\
ph-curves = { path = "../ph-curves" }|'
    if ! grep -q '^ph-curves = ' "$root/ph-curves/Cargo.toml"; then
        printf '  FAIL  ph-curves            mutation was not applied\n'
        failed=$((failed + 1))
    else
        expect_fail ph-curves 'no ph-curves'
    fi
else
    failed=$((failed + 1))
fi

# --- strict mode turns every skip into a failure ---------------------------
if make_copy strict-skip; then
    expect_fail strict-skip 'thumbv7em-none-eabi' \
        REQUIRE_NO_SKIPS=1 SKIP_EMBEDDED=1
else
    failed=$((failed + 1))
fi

# --- strict packaging requires auditable Git provenance -------------------
if make_copy release-provenance; then
    # make_copy intentionally copies tracked files but not .git. A release
    # package must refuse to proceed when it cannot identify an exact commit.
    # Stop Git discovery at the self-test root so the copy cannot inherit this
    # repository's .git directory from its parent path.
    git_ceiling=$(cd "$root" && pwd -P) || exit 1
    expect_fail release-provenance 'package list' \
        REQUIRE_NO_SKIPS=1 GIT_CEILING_DIRECTORIES="$git_ceiling"
else
    failed=$((failed + 1))
fi

if [ "$failed" -gt 0 ]; then
    printf '%s guard(s) did not fire on a mutation.\n' "$failed" >&2
    exit 1
fi
printf 'every guard fired on its mutation.\n'
exit 0
