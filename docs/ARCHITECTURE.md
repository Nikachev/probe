# Architecture Overview — rusty-probe-nicenano

This document details the software architecture, memory layout, hardware peripherals, and low-level driver implementations for the **rusty-probe** CMSIS-DAP v1/v2 debugger firmware ported to the **nice!nano v2** (nRF52840) board.

---

## 🏗️ System Overview

The firmware turns an nRF52840 microcontroller into an ARM Cortex-M SWD (Serial Wire Debug) probe compatible with host debug tools such as `probe-rs`, OpenOCD, PyOCD, and Keil MDK.

```
+-------------------------------------------------------------------+
|                        Host PC (probe-rs)                         |
+-------------------------------------------------------------------+
                                  |
                                  | USB CMSIS-DAP v1 (HID) / v2 (Bulk)
                                  v
+-------------------------------------------------------------------+
|               nice!nano v2 Probe (nRF52840 MCU)                   |
|                                                                   |
|  +------------------------+      +-----------------------------+  |
|  |   USB Stack (usb.rs)   |      |    SWD Driver (swd.rs)     |  |
|  | - FICR.DEVICEID Serial |      | - Direct PAC Register IO    |  |
|  | - CMSIS-DAP Endpoints  |      | - 64 MHz Precise Bit-Bang   |  |
|  +------------------------+      +-----------------------------+  |
|               |                                 |                 |
|               +----------------+----------------+                 |
|                                |                                  |
|                      Central Config (config.rs)                   |
|                      - Hardware & Timing Tokens                   |
+---------------+---------------------------------+-----------------+
                |                                 |
           LED (P0.15)                    SWD Lines (P0.17, P0.20, P0.22)
                                                  |
                                                  v
                                    +---------------------------+
                                    |     Target MCU (nRF52840) |
                                    +---------------------------+
```

---

## ⚙️ Build Profile & Optimization

The firmware binary is compiled with maximum optimization under `[profile.release]` in `Cargo.toml`:
- `opt-level = 3` (high optimization for bit-banging throughput)
- `codegen-units = 1` and `lto = 'fat'` (full link-time optimization and cross-module inlining)

---

## 🔧 Centralized Configuration (`src/config.rs`)

All hardware pins, system clock frequencies, magic reset values, and USB descriptors are consolidated in `src/config.rs`:
- **Hardware Pins:** `DEFAULT_SWDIO_PIN` (20), `DEFAULT_SWCLK_PIN` (17), `DEFAULT_NRESET_PIN` (22)
- **Frequencies:** `DEFAULT_CPU_FREQUENCY` (64 MHz), `DEFAULT_MAX_SWD_FREQUENCY` (5 MHz)
- **Reset Magic Values:** `GPREGRET_BOOTLOADER_CHECK` (`0xAB`), `DFU_MAGIC_UF2_RESET` (`0x57`), `APP_VTOR_OFFSET` (`0x0000_1000`)
- **USB Constants:** `USB_VID` (`0x1209`), `USB_PID` (`0x4853`), `USB_MANUFACTURER`, `USB_PRODUCT`

---

## 🗺️ Memory Map & Bootloader Integration

### Memory Layout
The nice!nano v2 runs under the Adafruit UF2 Bootloader without SoftDevice. The firmware memory map is configured in `memory.x` as follows:

| Address Range | Size | Component / Purpose |
|---|---|---|
| `0x0000_0000 .. 0x0000_1000` | 4 KB | MBR (Master Boot Record) |
| **`0x0000_1000 .. 0x000F_4000`** | **972 KB** | **Application Flash (`FLASH`)** |
| `0x000F_4000 .. 0x0010_0000` | 4 KB | Adafruit UF2 Bootloader + Settings |
| `0x2000_0000 .. 0x2004_0000` | 256 KB | System RAM (`RAM`) |

### Vector Table Relocation & Self-Reset
1. **VTOR Relocation:** Since the application is linked at base `0x0000_1000` (right after the MBR), the Cortex-M Vector Table Offset Register (`SCB.vtor`) is rewritten at application entry using `APP_VTOR_OFFSET`.
2. **One-Time Self-Reset Handoff:** The Adafruit UF2 bootloader jumps into the application without resetting peripherals, leaving the internal nRF52840 USB 3.3V power regulator uninitialized (`POWER.USBREGSTATUS.OUTPUTRDY = 0`). `init()` performs a one-time software reset guarded by `GPREGRET_BOOTLOADER_CHECK` (`0xAB`).

---

## ⚡ SWD Bit-Bang Driver (`src/swd.rs`)

The SWD driver implements high-speed bit-banging over GPIO without external hardware level translators.

### Pin Allocations & SwdPinConfig
- **`SwdPinConfig` Layout:** Encapsulates target pin definitions (`P0.20` SWDIO, `P0.17` SWDCLK, `P0.22` nRESET) and system CPU frequency (`64_000_000` Hz).
- **`SWDCLK` (`P0.17`):** Push-Pull output with **High Drive (`H0H1`)** mode for sharp pulse edges.
- **`SWDIO` (`P0.20`):** Dynamic bidirectional signal (Push-Pull **High Drive (`H0H1`)** output ⇄ Floating Pull-Up input).
- **`nRESET` (`P0.22`):** Open-Drain output (`Standard0Disconnect1`) with target 3.3V pull-up.

### Fast Parity Calculation (`swd_parity`)
Payload parity is calculated using native target popcount (`count_ones()`), compiling to single-cycle Cortex-M4 bit-count instructions:
```rust
#[inline(always)]
pub fn swd_parity(data: u32) -> bool {
    (data.count_ones() & 1) != 0
}
```

### Direct PAC Register Access via `p0_reg()` Helper
To achieve maximum throughput during bit-banging, register access uses a zero-cost inline helper `p0_reg()`:
```rust
#[inline(always)]
fn p0_reg() -> &'static nrf52840_hal::pac::p0::RegisterBlock {
    unsafe { &*nrf52840_hal::pac::P0::ptr() }
}
```

---

## 🔌 USB Stack & CMSIS-DAP (`src/usb.rs`)

### Protocol Endpoints & CDC Commands
- **CMSIS-DAP v1 (HID):** USB HID endpoints for universal cross-platform compatibility.
- **CMSIS-DAP v2 (Vendor Bulk):** High-speed Bulk endpoints for fast flash programming.
- **CDC Serial Commands:**
  - `dfu` / `bootloader` / `reboot` / `1200 baud touch`: Software trigger into Adafruit UF2 DFU bootloader mode (`DFU_MAGIC_UF2_RESET`).
  - `reset_target` / `target_reset` / `target-reset`: Asserts a 10 ms hardware `nRESET` pulse.
- **USBD Interrupt Encapsulation:** Low-level peripheral interrupt events are configured cleanly via `enable_usbd_interrupts()`.

---

## 🚦 RTIC 2 Concurrency Model (`src/bin/app.rs`)

The firmware uses the **RTIC 2** framework for zero-cost async multitasking:
- **`USBD` Interrupt (Priority 2):** Polling USB events and dispatching CMSIS-DAP packets.
- **`idle` Task:** CPU power-saving (`wfi`).
- **`blink` Task (Priority 1):** Manages status LED (`P0.15`) state transitions.
