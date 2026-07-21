# RETRO — Key Implementation Lessons & Discoveries

A concise summary of critical findings and architectural quirks identified during the port of `rusty-probe-firmware` to the **nice!nano v2** (nRF52840).

---

### 1. Bootloader Handoff & One-Time USB Power Reset
- **Issue:** The Adafruit UF2 Bootloader jumps directly into the application (`0x26000`) without performing a full system reset. As a result, the on-chip USB 3.3V regulator and USBD power domain remain uninitialized, causing continuous host USB connect/disconnect loops.
- **Solution:** Performed a one-time software reset (`cortex_m::peripheral::SCB::sys_reset()`) guarded by `GPREGRET = 0xAB` at the beginning of `init()`. This properly arms the hardware VBUS detector and USBD peripheral.

### 2. Memory Map & Vector Table Relocation
- **SoftDevice S140 6.1.1:** Present on stock nice!nano v2 boards (`0x1000..0x26000`).
- **App Address:** Application links at **`0x26000`** with `FLASH` length 824 KB (`0xCE000`).
- **VTOR:** Vector table relocation (`SCB.vtor.write(0x0002_6000)`) must be executed explicitly at boot.
- **UF2 Parameters:** `.uf2` image generated with base `0x26000` and family ID `0xADA52840` (`NRF52840`).

### 3. USB Clocking & Kick-start
- **HFXO Requirement:** nRF52840 USBD requires the 32 MHz external crystal (`Clocks::enable_ext_hfosc()`).
- **Interrupt Kick-start:** Initial `probe_usb.interrupt()` (`UsbBus::poll()`) must be invoked manually in `init()` to pull up D+, preventing a chicken-and-egg deadlock before the first `USBD` interrupt.

### 4. Software DFU Reboot Magic
- **Trigger:** Setting `POWER.GPREGRET = 0x57` (`DFU_MAGIC_UF2_RESET`) before triggering `sys_reset()` signals the Adafruit bootloader to stay in UF2 flashing mode (mounts `/Volumes/NICENANO`).

### 5. Dynamic SWDIO Bit-Bang
- **Pin Switching:** nRF52840 HAL pin types lack a dynamic pin constructor. Managed via `SwdioPin` enum with transient `Invalid` state during `core::mem::replace` (`PushPull` output ⇄ `Floating` input).

### 6. Unique USB Serial
- Derived from read-only register `FICR.DEVICEID[0..1]`, generating a stable 16-character hex serial number per chip.
