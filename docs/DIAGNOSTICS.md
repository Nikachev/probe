# Diagnostic Firmware Utilities — rusty-probe-nicenano

The repository includes standalone diagnostic binaries located in `src/bin/` to help troubleshoot hardware bring-up, USB power issues, HFXO crystal clocking, and USBD driver behavior on the **nice!nano v2** (nRF52840).

---

## 🛠️ Diagnostic Binaries Overview

| Binary | Source | Description | Prerequisites / Dependencies |
|---|---|---|---|
| **`diag`** | [`src/bin/diag.rs`](file:///Users/nikachev/github/probe/src/bin/diag.rs) | **Standalone Hardware Bring-up Diagnostic.** Does not use RTIC or USB. Uses LED (`P0.15`) blink patterns to diagnose HFXO 32 MHz crystal oscillator and USB power domain (`POWER.USBREGSTATUS`). | None (bare-metal) |
| **`diag_usb`** | [`src/bin/diag_usb.rs`](file:///Users/nikachev/github/probe/src/bin/diag_usb.rs) | **USB CDC Serial Loopback Diagnostic.** Uses RTIC and `usbd_serial` to test USB enumeration, endpoint interrupts, and CDC echo independently of CMSIS-DAP. | Host USB CDC serial driver |

---

## 💡 `diag`: Hardware Bring-Up Diagnostic

### How It Works
`diag` runs directly upon entry without initializing RTIC or the USB stack. It tests each peripheral subsystem sequentially and encodes its status onto the onboard LED on pin **`P0.15`** (active-high).

Each "blink" is a **150 ms ON / 150 ms OFF** pulse.

### LED Blink Interpretation Table

```
   [Power On] ---> 1 Blink ---> (Start HFXO 32 MHz) ---> 2 Blinks ---> [Continuous Loop]
                     |                                    |                   |
               (Hangs = Code)                      (Hangs = HFXO)       (Blink Code Pattern)
```

| Blink Sequence | Phase / Meaning | Diagnostic Diagnosis |
|---|---|---|
| **No blinks at all** | Pre-LED init failure | Board unpowered, corrupt bootloader, or SWD reset line held down. |
| **1 blink, then stops** | Stuck during HFXO startup | 32 MHz external crystal (`HFXO`) failed to start or oscillator pins disconnected. |
| **2 blinks, then pattern:** | **HFXO OK**, evaluating USB power: | |
| ↳ **2 blinks per loop** | `VBUSDETECT=1`, `OUTPUTRDY=1` | **All Systems Normal!** USB VBUS detected and 3.3V regulator ready. Issue is likely software USB stack or driver related. |
| ↳ **3 blinks per loop** | `VBUSDETECT=1`, `OUTPUTRDY=0` | VBUS detected, but internal 3.3V USB regulator output is not ready. |
| ↳ **4 blinks per loop** | `VBUSDETECT=0`, `OUTPUTRDY=1` | Unexpected state (regulator ready without VBUS). |
| ↳ **5 blinks per loop** | `VBUSDETECT=0`, `OUTPUTRDY=0` | No VBUS voltage detected by nRF52840 POWER peripheral. Check USB cable/port. |

---

## 🔌 `diag_usb`: USB CDC Echo Diagnostic

### How It Works
`diag_usb` sets up the RTIC 2 framework, enables the USBD peripheral with a USB CDC ACM Serial class, and provides an echo loop.

- When connected to a host PC, it enumerates as a USB Serial Device (`VID:PID 1209:4853`, Manufacturer: `diag`, Product: `CDC test`).
- Any characters sent over serial are immediately echoed back.
- The onboard LED (`P0.15`) reflects USB device connection states:
  - **Fast Toggle (100 ms):** `Default` state (Reset / Unconfigured).
  - **Medium Toggle (250 ms):** `Addressed` state.
  - **Solid ON (500 ms interval):** `Configured` state (USB Serial connection fully established).

---

## 📦 Building & Flashing Diagnostic Targets

### 1. Build UF2 Binary
To build diagnostic binaries, use the repository build script:

```bash
# Build standalone hardware diagnostic (diag):
./tools/make-uf2.sh diag

# Build USB CDC serial diagnostic (diag_usb):
./tools/make-uf2.sh diag_usb
```

The output files are generated at `tmp/diag.uf2` and `tmp/diag_usb.uf2`.

### 2. Flash to nice!nano v2
1. Put the nice!nano v2 into UF2 bootloader mode (double-tap **RESET** button). A USB drive named **`NICENANO`** will appear.
2. Copy the requested `.uf2` binary to the drive:
   - **macOS:** `cp -X tmp/diag.uf2 /Volumes/NICENANO/`
   - **Linux:** `cp tmp/diag.uf2 /media/$USER/NICENANO/`
   - **Windows:** `copy tmp\diag.uf2 D:\`
3. Observe the LED blink sequence or open a serial monitor to inspect USB CDC behavior.
