#!/usr/bin/env bash
# Build the firmware and produce a nice!nano v2 compatible .uf2 file.
#
# Usage:
#   ./tools/make-uf2.sh [BIN_NAME] [OUT_DIR]
#
# Then double-tap RESET on the nice!nano v2 and copy the resulting
# .uf2 to the "NICENANO" USB drive that appears.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN="${1:-app}"
OUTDIR="${2:-$ROOT_DIR/tmp}"
TARGET="thumbv7em-none-eabihf"
FAMILY="0xADA52840"      # nRF52840 UF2 family id
BASE="0x1000"           # application start (after MBR, no SoftDevice)

mkdir -p "$OUTDIR"

echo ">> Building $BIN (release)..."
cargo build --release --bin "$BIN"

echo ">> Extracting raw binary..."
cargo objcopy --release --bin "$BIN" -- -O binary "$OUTDIR/$BIN.bin"

echo ">> Converting to UF2 (family $FAMILY, base $BASE)..."
python3 "$SCRIPT_DIR/uf2conv.py" "$OUTDIR/$BIN.bin" -c -b "$BASE" -f "$FAMILY" -o "$OUTDIR/$BIN.uf2"

echo ">> Done: $OUTDIR/$BIN.uf2"
