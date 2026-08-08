#!/usr/bin/env bash
# Script to build all HIL test target binaries (.elf, .bin, .uf2)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTDIR="$ROOT_DIR/tmp/test-targets"
TARGET="thumbv7em-none-eabihf"
FAMILY="0xADA52840"      # nRF52840 UF2 family id
BASE="0x1000"           # application start (after MBR, no SoftDevice)

mkdir -p "$OUTDIR"

TARGETS=("target_blinky" "target_rtt" "target_fault")

echo "=========================================="
echo " Building HIL Test Target Binaries"
echo "=========================================="

echo ">> Batch compiling release target binaries..."
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml" \
    --bin target_blinky --bin target_rtt --bin target_fault

for BIN in "${TARGETS[@]}"; do
    echo ">> Extracting raw binary & generating UF2 for $BIN..."
    cargo objcopy --release --manifest-path "$ROOT_DIR/Cargo.toml" --bin "$BIN" -- -O binary "$OUTDIR/$BIN.bin"
    python3 "$SCRIPT_DIR/uf2conv.py" "$OUTDIR/$BIN.bin" -c -b "$BASE" -f "$FAMILY" -o "$OUTDIR/$BIN.uf2"

    echo ">> Copying ELF for $BIN..."
    cp "$ROOT_DIR/target/$TARGET/release/$BIN" "$OUTDIR/$BIN.elf"

    echo ">> Done: $OUTDIR/$BIN.elf, $OUTDIR/$BIN.bin, $OUTDIR/$BIN.uf2"
    echo "------------------------------------------"
done

echo "All HIL test targets built successfully in $OUTDIR"
