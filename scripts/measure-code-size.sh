#!/usr/bin/env sh
# Record function .text sizes for four named strategy pairings.
#
# Non-normative: not a guarantee, not total flash, and not WCET. The output is
# the committed docs/code-size-snapshot.txt; CI diffs against that file.
#
# Toolchain: the pinned 1.92.0 from rust-toolchain.toml, not nightly.
# Targets: thumbv7em-none-eabi, riscv32imac-unknown-none-elf.
# Profile: opt-level=s, lto=true, codegen-units=1, panic=abort, debug=false.
# Tool: llvm-nm --print-size from llvm-tools-preview.
#
# Exit 2 if a required target or llvm-tools-preview is missing.

set -u

cd "$(dirname "$0")/.." || exit 1

WORK=target/code-size-measure
ARM=thumbv7em-none-eabi
RISCV=riscv32imac-unknown-none-elf

if [ "${SKIP_EMBEDDED:-0}" != "0" ]; then
    printf 'SKIP_EMBEDDED=1; skipping code-size measurement\n' >&2
    exit 2
fi

missing=
for target in "$ARM" "$RISCV"; do
    if ! rustup target list --installed 2>/dev/null | tr -d '\r' | grep -qx "$target"; then
        printf 'target %s not installed; skipping (rustup target add %s)\n' "$target" "$target" >&2
        missing=1
    fi
done

if ! rustup component list --installed 2>/dev/null | tr -d '\r' | grep -q '^llvm-tools'; then
    printf 'llvm-tools-preview not installed; skipping (rustup component add llvm-tools-preview)\n' >&2
    missing=1
fi

sysroot=$(rustc --print sysroot) || exit 1
host=$(rustc -vV | awk '/^host:/{print $2}')
NM=
for candidate in \
    "$sysroot/lib/rustlib/$host/bin/llvm-nm" \
    "$sysroot/lib/rustlib/$host/bin/llvm-nm.exe"; do
    if [ -f "$candidate" ]; then
        NM=$candidate
        break
    fi
done
if [ -z "$NM" ]; then
    printf 'llvm-nm not found under %s; skipping\n' "$sysroot" >&2
    missing=1
fi

if [ -n "$missing" ]; then
    exit 2
fi

rm -rf "$WORK"
mkdir -p "$WORK/src" || exit 1

cat >"$WORK/Cargo.toml" <<'EOF'
[package]
name = "ph-surfaces-code-size"
version = "0.0.0"
edition = "2024"
publish = false

[[bin]]
name = "ph-surfaces-code-size"
path = "src/main.rs"

[dependencies]
ph-surfaces = { path = "../.." }

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
debug = false
EOF

# rust-lld is the default linker for these targets and does not accept gcc's
# -nostartfiles. The measurement bin provides `_start`.

cat >"$WORK/src/main.rs" <<'EOF'
//! Throwaway measurement consumer. Not packaged. `unsafe` is used only for
//! `#[unsafe(no_mangle)]` so `llvm-nm` can find the four wrappers.
#![no_std]
#![no_main]

use core::hint::black_box;
use core::panic::PanicInfo;
use ph_surfaces::{BilinearSurface, BucketedAxis, LinearAxis, UniformAxis, bucket_index};

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

static BINARY_X: [u16; 5] = [0, 25, 60, 100, 180];
static BINARY_Y: [u16; 4] = [0, 40, 90, 150];
static BINARY_V: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
static BINARY: BilinearSurface<5, 4> = BilinearSurface::new(&BINARY_X, &BINARY_Y, &BINARY_V);

static LINEAR_X: [u16; 3] = [0, 5, 100];
static LINEAR_Y: [u16; 2] = [0, 20];
static LINEAR_V: [[i32; 3]; 2] = [[0, 1, 2], [3, 4, 5]];
static LINEAR: BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>> = BilinearSurface::from_axes(
    LinearAxis::new(&LINEAR_X),
    LinearAxis::new(&LINEAR_Y),
    &LINEAR_V,
);

static UNIFORM_V: [[i32; 2]; 2] = [[0, 100], [1_000, 1_100]];
static UNIFORM: BilinearSurface<2, 2, UniformAxis<2, 0, 10>, UniformAxis<2, 0, 10>> =
    BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &UNIFORM_V);

static MIXED_X: [u16; 5] = [0, 1, 2, 40, 1_000];
static MIXED_X_INDEX: [u16; 8] = bucket_index(&MIXED_X);
static MIXED_V: [[i32; 5]; 3] = [
    [0, 10, 20, 400, 10_000],
    [-5, 5, 15, 395, 9_995],
    [-10, 0, 10, 390, 9_990],
];
static MIXED: BilinearSurface<5, 3, BucketedAxis<5, 8>, UniformAxis<3, 0, 50>> =
    BilinearSurface::from_axes(
        BucketedAxis::new(&MIXED_X, &MIXED_X_INDEX),
        UniformAxis::new(),
        &MIXED_V,
    );

fn value(result: Result<i32, ph_surfaces::SurfaceError>) -> i32 {
    match result {
        Ok(v) => v,
        Err(_) => 0,
    }
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn ph_eval_binary(x: u16, y: u16) -> i32 {
    value(BINARY.evaluate(x, y))
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn ph_eval_linear(x: u16, y: u16) -> i32 {
    value(LINEAR.evaluate(x, y))
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn ph_eval_uniform(x: u16, y: u16) -> i32 {
    value(UNIFORM.evaluate(x, y))
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn ph_eval_mixed(x: u16, y: u16) -> i32 {
    value(MIXED.evaluate(x, y))
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let x = black_box(1u16);
        let y = black_box(1u16);
        black_box(ph_eval_binary(x, y));
        black_box(ph_eval_linear(x, y));
        black_box(ph_eval_uniform(x, y));
        black_box(ph_eval_mixed(x, y));
    }
}
EOF

rustc_version=$(rustc --version) || exit 1

symbol_size() {
    lib=$1
    name=$2
    size=$("$NM" --print-size --defined-only "$lib" | tr -d '\r' | awk -v n="$name" '
        NF >= 4 && ($4 == n || $4 == ("_" n)) { print $2; found = 1 }
        END { if (!found) exit 1 }
    ') || {
        printf 'symbol %s not found in %s\n' "$name" "$lib" >&2
        "$NM" --print-size --defined-only "$lib" | tr -d '\r' >&2
        return 1
    }
    printf '%s\n' "$size"
}

print_target() {
    target=$1
    (
        cd "$WORK" || exit 1
        CARGO_TARGET_DIR="$(pwd)/target" cargo build --release --target "$target" >&2
    ) || return 1

    bin="$WORK/target/$target/release/ph-surfaces-code-size"
    if [ ! -f "$bin" ]; then
        printf 'expected %s after the measurement build\n' "$bin" >&2
        return 1
    fi

    printf '%s\n' "$target"
    for name in ph_eval_binary ph_eval_linear ph_eval_mixed ph_eval_uniform; do
        size=$(symbol_size "$bin" "$name") || return 1
        printf '%s %s\n' "$name" "$size"
    done
    printf '\n'
}

{
    printf '%s\n' \
        '# ph-surfaces code-size snapshot (non-normative)' \
        "# Date: 2026-08-17" \
        "# Toolchain: ${rustc_version}" \
        '# Profile: opt-level=s, lto=true, codegen-units=1, panic=abort, debug=false' \
        '# Tool: llvm-nm --print-size (function .text, not whole-binary flash)' \
        '# Pairings:' \
        '#   ph_eval_binary   Binary×Binary ELEVATION 5×4' \
        '#   ph_eval_linear   Linear×Linear 3×2' \
        '#   ph_eval_uniform  Uniform×Uniform 2×2' \
        '#   ph_eval_mixed    BucketedAxis<5, 8> × UniformAxis<3, 0, 50>' \
        '#' \
        '# This is not a guarantee, not total flash, and not WCET.' \
        ''
    print_target "$ARM" || exit 1
    print_target "$RISCV" || exit 1
} || exit 1
