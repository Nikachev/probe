# rusty-probe-firmware — nice!nano v2 (nRF52840) port

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Target](https://img.shields.io/badge/Target-nRF52840-blue.svg)](https://www.nordicsemi.com/Products/nRF52840)
[![Protocol](https://img.shields.io/badge/Protocol-CMSIS--DAP%20v1%2Fv2-green.svg)](https://arm-software.github.io/CMSIS_5/DAP/html/index.html)
[![HIL Tests](https://img.shields.io/badge/HIL--Tests-29%2F29%20PASS-brightgreen.svg)](docs/HIL_TESTING.md)

Port of the [`probe-rs/rusty-probe-firmware`](https://github.com/probe-rs/rusty-probe-firmware) CMSIS-DAP v1/v2 debugger (originally built for RP2040) to the compact and affordable **nice!nano v2** board (nRF52840, Cortex-M4F).

Turns the board into a high-speed SWD debug probe for flashing and debugging ARM Cortex-M microcontrollers using `probe-rs`, OpenOCD, PyOCD, and other toolchains.

---

## 🌟 Features

- **CMSIS-DAP v1 (HID) & CMSIS-DAP v2 (Bulk):** Full compatibility with `probe-rs`, OpenOCD, PyOCD, Keil, etc.
- **Unique USB Serial Number:** Derived from the hardware `FICR.DEVICEID` of the nRF52840 MCU.
- **Single LED Status Indication (P0.15):**
  - **Idle / Disconnected:** Short 100 ms pulse once per second.
  - **Connected:** Solid ON when host debugger attaches.
  - **Running / Active:** Fast 5 Hz blink during target execution or active SWD transfers.
- **DFU Reboot:** Software trigger to reboot into Adafruit UF2 DFU Bootloader mode (`GPREGRET = 0x57`).
- **Direct 3.3 V Logic & Zero-Overhead Bit-Bang:** Direct 1-cycle register PAC SWD bit-bang without external level-translator chips.

---

## 📌 Target Connection (SWD Pinout)

| SWD Signal | nRF52840 Pin | nice!nano v2 Header Label | Description / Configuration |
|---|---|---|---|
| **SWDCLK** | **`P0.17`** | **`017`** | SWD clock line (Push-Pull output) |
| **SWDIO** | **`P0.20`** | **`020`** | Bidirectional SWD data line (Push-Pull output ⇄ Pull-Up input) |
| **nRESET** | **`P0.22`** | **`022`** | Target reset line (Open-Drain, pulled up on target board) |
| **VCC (3.3V)** | **`VCC`** | **`VCC`** | Target 3.3V power supply (regulated output from nice!nano) |
| **GND** | **`GND`** | **`GND`** | Ground connection (**Required**) |

> 💡 **Pin Layout Note:**
> All three SWD signal pins (`017`, `020`, `022`) sit right next to each other on the left header row of nice!nano v2, making connection with a standard jumper wire block very convenient!

> ⚠️ **CRITICAL SAFETY WARNING:**
> Always connect your target board's 3.3 V supply pin to **`VCC`**, NOT **`RAW`**.
> The **`RAW`** pin supplies **5 V directly from USB**, which will damage 3.3 V target microcontrollers!

---

## 🛠️ Build & Flash

### Prerequisites
- Rust toolchain:
  ```bash
  rustup target add thumbv7em-none-eabihf
  cargo install flip-link cargo-binutils
  rustup component add llvm-tools
  ```
- Python 3 (the `tools/uf2conv.py` script is included in the repository).

### Building UF2 Firmware

```bash
./tools/make-uf2.sh app
```
The output binary is saved at `tmp/app.uf2` (base address `0x26000`, family ID `0xADA52840`).

### Flashing nice!nano v2
1. Connect nice!nano v2 to USB.
2. Double-tap the **RESET** button on the board — a USB drive named **`NICENANO`** will appear.
3. Copy `tmp/app.uf2` to the `NICENANO` drive:
   - **macOS:**
     ```bash
     cp -X tmp/app.uf2 /Volumes/NICENANO/
     ```
   - **Linux:**
     ```bash
     cp tmp/app.uf2 /media/$USER/NICENANO/  # or /mnt/NICENANO/
     ```
   - **Windows:**
     ```cmd
     copy tmp\app.uf2 D:\
     ```
4. The board automatically reboots into the new CMSIS-DAP probe firmware.

---

## 🚀 Usage with `probe-rs`

Verify probe detection:
```bash
probe-rs list
```
*Output:*
```text
The following debug probes were found:
[0]: Rusty Probe (nice!nano) with CMSIS-DAP v1/v2 Support -- 1209:4853-1:6a674c50f23e076c (CMSIS-DAP)
```

Flash a target MCU (e.g. nRF52840):
```bash
probe-rs download --chip nRF52840_xxAA --probe 1209:4853 firmware.elf
```

Run debug session and RTT output:
```bash
probe-rs run --chip nRF52840_xxAA --probe 1209:4853 firmware.elf --rtt-scan-memory
```

---

## 🧪 Hardware-in-the-Loop (HIL) Testing

The repository includes a complete automated 29-test HIL suite for verifying CMSIS-DAP probe functionality using a second nice!nano v2 board as target MCU.

- **Build Test Targets:** `./tools/build-test-targets.sh`
- **Run Full Automated Test Suite (29/29 PASS):** `./tools/run_hil_tests.py`
- **Run Specific Test Suite or Test:** `./tools/run_hil_tests.py --suite 3` or `./tools/run_hil_tests.py --test TS-301`
- **Cross-Platform Auto-Flash & Test:** `./tools/auto_flash_and_test.py`

### Test Performance Metrics:
- **Flash Download Throughput:** **165.98 KB/s**
- **RAM Transfer Bandwidth:** **10.36 KB/s**
- **Test Pass Rate:** **29/29 (100% Passed)**

For complete wiring diagrams, pinout instructions, and detailed test suite documentation, see the [HIL Testing Guide](docs/HIL_TESTING.md).


---

## 🗺️ Memory Layout

Ships with Adafruit UF2 Bootloader and **SoftDevice S140 6.1.1** (`0x1000..0x26000`):
- `0x00000..0x01000`: MBR
- `0x01000..0x26000`: SoftDevice S140 6.1.1
- `0x26000..0xF4000`: CMSIS-DAP Application (`FLASH`, ~824 KB)
- `0xF4000..0x100000`: Adafruit Bootloader + Settings
