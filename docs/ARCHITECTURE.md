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

## 🗺️ Memory Map & Bootloader Integration

### Memory Layout
The nice!nano v2 comes pre-flashed with the Adafruit UF2 Bootloader and Nordic SoftDevice S140 v6.1.1. The firmware memory map is configured in `memory.x` as follows:

| Address Range | Size | Component / Purpose |
|---|---|---|
| `0x0000_0000 .. 0x0000_1000` | 4 KB | MBR (Master Boot Record) |
| `0x0000_1000 .. 0x0002_6000` | 148 KB | SoftDevice S140 6.1.1 (reserved) |
| **`0x0002_6000 .. 0x000F_4000`** | **824 KB** | **Application Flash (`FLASH`)** |
| `0x000F_4000 .. 0x0010_0000` | 48 KB | Adafruit UF2 Bootloader + Settings |
| `0x2000_0000 .. 0x2004_0000` | 256 KB | System RAM (`RAM`) |

### Vector Table Relocation & Self-Reset
1. **VTOR Relocation:** Since the application is linked at base `0x0002_6000`, the Cortex-M Vector Table Offset Register (`SCB.vtor`) is explicitly rewritten at application entry:
   ```rust
   cx.core.SCB.vtor.write(0x0002_6000);
   ```
2. **One-Time Self-Reset Handoff:** The Adafruit UF2 bootloader jumps into the application (`0x26000`) without resetting peripherals, leaving the internal nRF52840 USB 3.3V power regulator uninitialized (`POWER.USBREGSTATUS.OUTPUTRDY = 0`). To fix this, `init()` performs a one-time software reset guarded by `GPREGRET = 0xAB`:
   ```rust
   let power = unsafe { &*nrf52840_hal::pac::POWER::ptr() };
   if power.gpregret.read().bits() != 0xAB {
       power.gpregret.write(|w| unsafe { w.bits(0xAB) });
       cortex_m::peripheral::SCB::sys_reset();
   }
   power.gpregret.write(|w| unsafe { w.bits(0) });
   ```

---

## ⚡ SWD Bit-Bang Driver (`src/swd.rs`)

The SWD driver implements high-speed bit-banging over GPIO without external hardware level translators.

### Pin Allocations
- **`SWDCLK` (`P0.17`):** Push-Pull output.
- **`SWDIO` (`P0.20`):** Dynamic bidirectional signal (Push-Pull output ⇄ Floating Pull-Up input).
- **`nRESET` (`P0.22`):** Open-Drain output with internal/external pull-up.

### Direct PAC Register Access
To achieve maximum throughput and zero HAL overhead during turnaround phases, pin direction and pin read operations bypass high-level HAL abstractions and access `NRF_P0` registers directly:
```rust
// Direct PAC configuration of P0.20 direction (Input with Pull-Up vs Push-Pull Output)
let p0 = unsafe { &*nrf52840_hal::pac::P0::ptr() };

// Set SWDIO to Input mode with Pull-up:
p0.pin_cnf[20].write(|w| {
    w.dir().input()
     .input().connect()
     .pull().pullup()
     .drive().s0s1()
});

// Fast read of SWDIO pin state:
let bit_is_high = (p0.in_.read().bits() & (1 << 20)) != 0;
```

### CPU Frequency & Delays
The nRF52840 CPU runs at **64 MHz** (`CPU_FREQUENCY = 64_000_000`). Half-period clock delay ticks are computed dynamically based on the requested target SWD frequency:
```rust
let ticks = (64_000_000 / (2 * target_freq_hz)).saturating_sub(4);
cortex_m::asm::delay(ticks);
```

---

## 🔌 USB Stack & CMSIS-DAP (`src/usb.rs`)

### Unique Hardware Serial Number
The probe derives its 16-character hexadecimal USB serial number directly from the read-only hardware register `FICR.DEVICEID[0..1]`:
```rust
let deviceid0 = ficr.deviceid[0].read().bits();
let deviceid1 = ficr.deviceid[1].read().bits();
// Hex formatted string: "6a674c50f23e076c"
```

### Protocol Endpoints
- **CMSIS-DAP v1 (HID):** Uses USB Human Interface Device class endpoints for maximum compatibility across operating systems without requiring custom drivers.
- **CMSIS-DAP v2 (Vendor Bulk):** Uses high-speed raw Bulk endpoints for fast flash memory programming (achieving up to **165.98 KB/s** download speeds).

---

## 🚦 RTIC 2 Concurrency Model (`src/bin/app.rs`)

The firmware uses the **RTIC 2 (Real-Time Interrupt-driven Concurrency)** framework for zero-cost async multitasking:

- **`USBD` Interrupt (Priority 2):** Handles USB bus events and receives CMSIS-DAP packets.
- **`idle` Task:** Serves background processing and USB polling loops.
- **`led_task` (Priority 1):** Manages status LED (`P0.15`) state transitions (idle pulse, connected solid ON, active fast blink).
