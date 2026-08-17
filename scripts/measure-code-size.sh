#!/usr/bin/env sh
# Record compiler-emitted function .text sizes for four named strategy pairings.
#
# Non-normative: not a guarantee, not total flash, and not WCET. The output is
# the committed docs/code-size-snapshot.txt; CI diffs against that file.
#
# Toolchain: the pinned 1.94.0 from rust-toolchain.toml, not nightly.
# Targets: thumbv7em-none-eabi, riscv32imac-unknown-none-elf.
# Profile: opt-level=s, lto=false, codegen-units=1, panic=abort, debug=false.
# Tool: llvm-nm --demangle --print-size from llvm-tools-preview.
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

[lib]
path = "src/lib.rs"

[dependencies]
ph-surfaces = { path = "../.." }

[features]
pairing-binary = []
pairing-linear = []
pairing-mixed = []
pairing-uniform = []

[profile.release]
opt-level = "s"
lto = false
codegen-units = 1
panic = "abort"
debug = false
EOF

cat >"$WORK/src/lib.rs" <<'EOF'
//! Throwaway measurement consumer. Not packaged. The wrappers retain normal
//! Rust symbol mangling; `llvm-nm --demangle` identifies them by item name.
#![no_std]

use ph_surfaces::BilinearSurface;
#[cfg(feature = "pairing-linear")]
use ph_surfaces::LinearAxis;
#[cfg(any(feature = "pairing-mixed", feature = "pairing-uniform"))]
use ph_surfaces::UniformAxis;
#[cfg(feature = "pairing-mixed")]
use ph_surfaces::{BucketedAxis, bucket_index};

#[cfg(feature = "pairing-binary")]
static BINARY_X: [u16; 5] = [0, 25, 60, 100, 180];
#[cfg(feature = "pairing-binary")]
static BINARY_Y: [u16; 4] = [0, 40, 90, 150];
#[cfg(feature = "pairing-binary")]
static BINARY_V: [[i32; 5]; 4] = [
    [-120, -35, 40, 15, -60],
    [-80, 10, 95, 60, -20],
    [-15, 55, 130, 88, 5],
    [-40, 20, 70, 110, 45],
];
#[cfg(feature = "pairing-binary")]
static BINARY: BilinearSurface<5, 4> = BilinearSurface::new(&BINARY_X, &BINARY_Y, &BINARY_V);

#[cfg(feature = "pairing-linear")]
static LINEAR_X: [u16; 3] = [0, 5, 100];
#[cfg(feature = "pairing-linear")]
static LINEAR_Y: [u16; 2] = [0, 20];
#[cfg(feature = "pairing-linear")]
static LINEAR_V: [[i32; 3]; 2] = [[0, 1, 2], [3, 4, 5]];
#[cfg(feature = "pairing-linear")]
static LINEAR: BilinearSurface<3, 2, LinearAxis<3>, LinearAxis<2>> = BilinearSurface::from_axes(
    LinearAxis::new(&LINEAR_X),
    LinearAxis::new(&LINEAR_Y),
    &LINEAR_V,
);

#[cfg(feature = "pairing-uniform")]
static UNIFORM_V: [[i32; 2]; 2] = [[0, 100], [1_000, 1_100]];
#[cfg(feature = "pairing-uniform")]
static UNIFORM: BilinearSurface<2, 2, UniformAxis<2, 0, 10>, UniformAxis<2, 0, 10>> =
    BilinearSurface::from_axes(UniformAxis::new(), UniformAxis::new(), &UNIFORM_V);

#[cfg(feature = "pairing-mixed")]
static MIXED_X: [u16; 17] = [
    0, 100, 210, 300, 405, 500, 610, 700, 805, 900, 1_010, 1_100, 1_205, 1_300,
    1_410, 1_500, 1_600,
];
#[cfg(feature = "pairing-mixed")]
static MIXED_X_INDEX: [u16; 8] = bucket_index(&MIXED_X);
#[cfg(feature = "pairing-mixed")]
static MIXED_V: [[i32; 17]; 9] = mixed_values();
#[cfg(feature = "pairing-mixed")]
static MIXED: BilinearSurface<17, 9, BucketedAxis<17, 8>, UniformAxis<9, 0, 200>> =
    BilinearSurface::from_axes(
        BucketedAxis::new(&MIXED_X, &MIXED_X_INDEX),
        UniformAxis::new(),
        &MIXED_V,
    );

#[cfg(feature = "pairing-mixed")]
const fn mixed_values() -> [[i32; 17]; 9] {
    let mut values = [[0; 17]; 9];
    let mut y = 0;
    while y < 9 {
        let mut x = 0;
        while x < 17 {
            values[y][x] = (x as i32) * 100 - (y as i32) * 37;
            x += 1;
        }
        y += 1;
    }
    values
}

fn value(result: Result<i32, ph_surfaces::SurfaceError>) -> i32 {
    match result {
        Ok(v) => v,
        Err(_) => 0,
    }
}

#[cfg(feature = "pairing-binary")]
#[inline(never)]
pub extern "C" fn ph_eval_binary(x: u16, y: u16) -> i32 {
    value(BINARY.evaluate(x, y))
}

#[cfg(feature = "pairing-linear")]
#[inline(never)]
pub extern "C" fn ph_eval_linear(x: u16, y: u16) -> i32 {
    value(LINEAR.evaluate(x, y))
}

#[cfg(feature = "pairing-uniform")]
#[inline(never)]
pub extern "C" fn ph_eval_uniform(x: u16, y: u16) -> i32 {
    value(UNIFORM.evaluate(x, y))
}

#[cfg(feature = "pairing-mixed")]
#[inline(never)]
pub extern "C" fn ph_eval_mixed(x: u16, y: u16) -> i32 {
    value(MIXED.evaluate(x, y))
}
EOF

rustc_version=$(rustc --version) || exit 1

object_text_size() {
    object=$1
    size=$("$NM" --demangle --print-size --defined-only "$object" | tr -d '\r' | awk '
        function hex_value(c) {
            c = tolower(c)
            return index("0123456789abcdef", c) - 1
        }
        function hex_to_decimal(s, total, i) {
            total = 0
            for (i = 1; i <= length(s); i++) {
                total = total * 16 + hex_value(substr(s, i, 1))
            }
            return total
        }
        NF >= 3 && $3 ~ /^[Tt]$/ { total += hex_to_decimal($2); found = 1 }
        END { if (!found) exit 1; printf "%08x\n", total }
    ') || {
        printf 'no text symbols found in %s\n' "$object" >&2
        "$NM" --demangle --print-size --defined-only "$object" | tr -d '\r' >&2
        return 1
    }
    printf '%s\n' "$size"
}

print_pairing() {
    target=$1
    name=$2
    feature=$3
    build_dir="$WORK/target/$target/$name"
    (
        cd "$WORK" || exit 1
        CARGO_TARGET_DIR="$(pwd)/target/$target/$name" \
            cargo rustc --release --target "$target" --no-default-features \
            --features "$feature" -- --emit=obj >&2
    ) || return 1

    object=$(find "$build_dir/$target/release/deps" -maxdepth 1 \
        -type f -name 'ph_surfaces_code_size-*.o' -print | head -n 1)
    if [ -z "$object" ] || [ ! -f "$object" ]; then
        printf 'expected a ph_surfaces_code_size object after the measurement build\n' >&2
        return 1
    fi

    size=$(object_text_size "$object") || return 1
    printf '%s %s\n' "$name" "$size"
}

print_target() {
    target=$1
    printf '%s\n' "$target"
    print_pairing "$target" ph_eval_binary pairing-binary || return 1
    print_pairing "$target" ph_eval_linear pairing-linear || return 1
    print_pairing "$target" ph_eval_mixed pairing-mixed || return 1
    print_pairing "$target" ph_eval_uniform pairing-uniform || return 1
}

{
    printf '%s\n' \
        '# ph-surfaces code-size snapshot (non-normative)' \
        "# Toolchain: ${rustc_version}" \
        '# Profile: opt-level=s, lto=false, codegen-units=1, panic=abort, debug=false' \
        '# Tool: llvm-nm --demangle --print-size (single-pairing compiler object .text total)' \
        '# Pairings:' \
        '#   ph_eval_binary   Binary×Binary ELEVATION 5×4' \
        '#   ph_eval_linear   Linear×Linear 3×2' \
        '#   ph_eval_uniform  Uniform×Uniform 2×2' \
        '#   ph_eval_mixed    BucketedAxis<17, 8> × UniformAxis<9, 0, 200>' \
        '#' \
        '# This is not a guarantee, not total flash, and not WCET.' \
        ''
    print_target "$ARM" || exit 1
    printf '\n'
    print_target "$RISCV" || exit 1
} || exit 1
