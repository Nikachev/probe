#!/usr/bin/env bash
# Script to build all HIL test target binaries (.elf, .bin, .uf2)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTDIR="$ROOT_DIR/tmp/test-targets"
FAMILY="0xADA52840"      # nRF52840 UF2 family id
BASE="0x26000"          # application start (after MBR + S140 SoftDevice)

mkdir -p "$OUTDIR"

TARGETS=("target_blinky" "target_rtt" "target_fault")

echo "=========================================="
echo " Building HIL Test Target Binaries"
echo "=========================================="

for BIN in "${TARGETS[@]}"; do
    echo ">> Building $BIN (release)..."
    cargo build --release --bin "$BIN"

    echo ">> Copying ELF for $BIN..."
    cp "$ROOT_DIR/target/thumbv7em-none-eabihf/release/$BIN" "$OUTDIR/$BIN.elf"

    echo ">> Extracting raw binary for $BIN..."
    cargo objcopy --release --bin "$BIN" -- -O binary "$OUTDIR/$BIN.bin"

    echo ">> Converting $BIN to UF2..."
    python3 "$SCRIPT_DIR/uf2conv.py" "$OUTDIR/$BIN.bin" -c -b "$BASE" -f "$FAMILY" -o "$OUTDIR/$BIN.uf2"

    echo ">> Done: $OUTDIR/$BIN.elf, $OUTDIR/$BIN.bin, $OUTDIR/$BIN.uf2"
    echo "------------------------------------------"
done

echo "All HIL test targets built successfully in $OUTDIR"
