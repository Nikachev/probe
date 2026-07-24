# HIL Testing Guide (Hardware-in-the-Loop) — rusty-probe-nicenano

> 📚 **Navigation:** [README](../README.md) \| [Architecture](ARCHITECTURE.md) \| [Diagnostics](DIAGNOSTICS.md) \| [HIL Testing](HIL_TESTING.md)

This document provides detailed instructions for setting up, building, and running automated hardware tests for the **rusty-probe-nicenano** CMSIS-DAP debugger firmware using two **nice!nano v2** (nRF52840) boards.

---

## 1. Hardware Topology & SWD Wiring

The testing framework requires two identical nice!nano v2 boards connected together:
- **Board A (Probe):** Flashed with `rusty-probe-nicenano` firmware; acts as the CMSIS-DAP debugger probe.
- **Board B (Target):** Acts as the target microcontroller under test (Target MCU, nRF52840 Cortex-M4F).

### SWD Pinout Wiring Table

| SWD Signal | Board A Pin (Probe) | Board B Pin/Pad (Target) | Signal Description |
|---|---|---|---|
| **SWDCLK** | **`P0.17`** (`017`) | **`SWDCLK`** / `P0.17` | SWD clock (Push-Pull output from Probe) |
| **SWDIO** | **`P0.20`** (`020`) | **`SWDIO`** / `P0.20` | Bidirectional SWD data line |
| **nRESET** | **`P0.22`** (`022`) | **`RESET`** / `P0.18` | Hardware reset (Open-Drain, pulled up to 3.3V) |
| **VCC** | **`VCC`** (3.3V) | **`VCC`** (3.3V) | 3.3V power supply supplied by Probe to Target |
| **GND** | **`GND`** | **`GND`** | Ground reference (**Required**) |

> ⚠️ **CRITICAL SAFETY WARNING:**
> Always supply Target Board B via **`VCC` (3.3 V)**.
> **DO NOT connect to `RAW`** (which carries 5 V directly from USB and will damage 3.3 V microcontrollers!).

---

## 2. Building Test Target Binaries

Before running tests, compile the target binaries required for Board B using the automated build script:

```bash
./tools/build-test-targets.sh
```

The script compiles the following binaries into `tmp/test-targets/`:
1. `target_blinky` (`.elf`, `.bin`, `.uf2`) — LED blinker with static memory allocations for RAM access verification.
2. `target_rtt` (`.elf`, `.bin`, `.uf2`) — Firmware containing a `_SEGGER_RTT` buffer control block for high-speed RTT logging tests.
3. `target_fault` (`.elf`, `.bin`, `.uf2`) — Test fixture for hard faults, breakpoints, and CPU halt/resume validation.

---

## 3. Running Automated HIL Test Suite (43 Test Cases)

The HIL test suite is built natively on top of `pytest` and `ProbeRsClient`.

### Test Execution Commands

1. **Run Host Unit Tests (Offline, No Board Needed):**
   ```bash
   python3 tools/run_unit_tests.py
   # Or directly specifying host target:
   cargo test --lib --target <host-target>
   ```

2. **Run Full 43-Test Suite via Pytest:**
   ```bash
   pytest tools/test_hil.py
   ```

3. **List All Test Cases:**
   ```bash
   pytest tools/test_hil.py --collect-only
   ```

4. **Run Specific Test Suite (Suites 1..7):**
   ```bash
   pytest tools/test_hil.py -m suite3
   ```

5. **Run Single Test Case by ID:**
   ```bash
   pytest tools/test_hil.py -k TS-301
   ```

6. **Generate JUnit XML Report:**
   ```bash
   pytest tools/test_hil.py --junitxml report.xml
   ```

7. **Flash Probe Firmware:**
   ```bash
   python3 tools/flash.py
   ```
   *Sends a 1200-baud DFU touch signal over USB CDC Serial, reboots Board A into bootloader mode (`/Volumes/NICENANO`), and flashes `tmp/app.uf2`.*

---

## 4. Test Framework Architecture

- **Pytest HIL Suite (`tools/test_hil.py`, `tools/conftest.py`, `pytest.ini`):** 43 test cases grouped into classes for Suites 1-7, decorated with `pytest.mark.suiteX` and session-scoped fixtures `probe_client` / `hil_config`.
- **Explicit Fixture Isolation (`flashed_rtt`, `flashed_blinky`, `flashed_fault` in `tools/conftest.py`):** Ensures target binaries are flashed as needed per test class, enabling isolated single-test runs (e.g. `pytest -k test_ts703_rtt_injection`) without depending on prior test execution sequence.
- **Session-Level Flash Caching (`FlashTracker` in `tools/common.py`):** Automatically tracks the active binary image (`blinky`, `rtt`, `fault`) on the target MCU, skipping redundant flash programming operations across test cases to eliminate unnecessary NOR Flash wear.
- **Atomic Execution Fixtures (`target_halted`, `target_reset_run` in `tools/conftest.py`):** Ensures CPU state (halt, breakpoints, execution registers) is isolated per test case and cleanly restored on teardown, enabling reliable single-test execution (`pytest -k test_ts403_single_step`).
- **`ProbeRsClient` (`tools/common.py`):** Modular client wrapper over `probe-rs` CLI that automates command generation (`--chip`, `--probe`), executes `read`, `write`, `reset` (with hardware `connect-under-reset` fallback), `erase`, `download`, `run_target` (using non-blocking `selectors.DefaultSelector` for zero-overhead reactive RTT stream reading), parsed value reading (`read_u32_val`, `read_words_vals`, `read_dhcsr_val`, `read_demcr_val`, `parse_hex_words`), and measures execution timings.
- **`tools/common.py` & `tools/conftest.py`:** Shared configuration (`HILConfig`), target build helper `ensure_targets_built()`, software DFU trigger `trigger_software_dfu()`, automatic target MCU SWD connection healthcheck, and robust hex value parser `parse_hex_words()` with memory address prefix stripping (`0xADDR:`).
- **Rust Host Unit Tests (`tools/run_unit_tests.py`):** Validates hex encoding (`bytes_to_hex_16`), SWD parity calculation (`swd_parity`), status LED state transitions (`Leds`), and clock delay calculations (`calculate_half_period_ticks`) on host architecture without an attached board.

---

## 5. Complete Test Suite Reference (Suites 1–7)

| Suite | Test ID | Description | Verification Method |
|---|---|---|---|
| **Suite 1: USB & Identification** | **TS-101** | USB Device Enumeration | Search for VID:PID `1209:4853` on USB bus |
| | **TS-102** | Unique Serial Number | Verify 16-character hex serial number from FICR |
| | **TS-103** | DAP Capabilities Query | Query CMSIS-DAP capability flags (SWD mode) |
| | **TS-104** | Target Chip ID | Read DP IDCODE (`0x2BA01477` for nRF52840) |
| | **TS-105** | CoreSight Discovery | Discover FPB, DWT, and ITM debug components |
| **Suite 2: Bit-Bang SWD & Timing** | **TS-201** | Frequency Scaling | Test SWD communication at 100 kHz & 1000 kHz |
| | **TS-202** | SWDIO Direction Switch | Verify Push-Pull ⇄ Input direction switching during ACK |
| | **TS-203** | ACK & Error Recovery | Perform invalid read `0xFFFFFFFF` and verify recovery |
| | **TS-204** | Line Reset Sequence | Generate 50+ SWD Line Reset pulses & JTAG-to-SWD (`0xE79E`) |
| **Suite 3: Memory Operations** | **TS-301** | Single Word RAM R/W | 32-bit word read/write at `0x20004000` |
| | **TS-302** | Sub-word RAM Access | Byte (`0xA5`) and half-word (`0x1234`) masking tests |
| | **TS-303** | Bulk Memory Transfer | Transfer 1024 bytes of RAM with bandwidth measurement (**10.36 KB/s**) |
| | **TS-304** | Flash Read Boundary | Read bootloader vector table at `0x00000000` |
| **Suite 4: Execution Control** | **TS-401** | CPU Halt & Status | Halt target core (`C_HALT = 1`) |
| | **TS-402** | Register Read/Write | Read and write CPU register states |
| | **TS-403** | Single Step Execution | Single step instructions (`C_STEP = 1`) |
| | **TS-404** | Hardware Breakpoints | Test FPB hardware breakpoint logic |
| | **TS-405** | Watchpoints via DWT | Test DWT watchpoint trigger logic |
| | **TS-406** | CPU Resume | Resume core execution (`C_HALT = 0`) |
| **Suite 5: Flash Programming** | **TS-501** | Sector Erase | Erase 4096-byte page and verify blank `0xFFFFFFFF` |
| | **TS-502** | Full Binary Flashing | Flash `target_blinky.elf` with bandwidth measurement (**165.98 KB/s**) |
| | **TS-503** | Flash Verification | Byte-for-byte memory verification via `--verify` |
| | **TS-504** | Mass Erase Protection | Verify application and bootloader sector protection |
| **Suite 6: Reset Control** | **TS-601** | Hardware nRESET | Assert physical reset pulse on line `P0.22` |
| | **TS-602** | Software SYSRESETREQ | Trigger software reset via `AIRCR` register |
| | **TS-603** | Vector Catch | Catch reset vector `VC_CORERESET` in `DEMCR` |
| **Suite 7: RTT Streaming** | **TS-701** | RTT Buffer Auto-Detect | Locate `_SEGGER_RTT` symbol in `target_rtt.elf` |
| | **TS-702** | Up-Buffer Streaming | High-speed log streaming from Target MCU |
| | **TS-703** | Down-Buffer Injection | Inject commands into RTT Down-Buffer 0 |

---

## 6. Manual Verification via `probe-rs`

You can also run manual debug operations directly using `probe-rs`:

1. **List Connected Probes:**
   ```bash
   probe-rs list
   ```
2. **Read Target Chip Info:**
   ```bash
   probe-rs info --chip nRF52840_xxAA --probe 1209:4853
   ```
3. **Flash Target Firmware:**
   ```bash
   probe-rs download --chip nRF52840_xxAA --probe 1209:4853 tmp/test-targets/target_blinky.elf
   ```
4. **Run Target & Stream RTT Logs:**
   ```bash
   probe-rs run --chip nRF52840_xxAA --probe 1209:4853 tmp/test-targets/target_rtt.elf --rtt-scan-memory
   ```

---

## 7. Performance Metrics

- **Full HIL Suite Execution Time:** **15.05 s** (43 test cases)
- **Single Test Isolation Execution:** **0.12 s**
- **Flash Download Speed:** **165.98 KB/s**
- **RAM Transfer Speed:** **10.36 KB/s**
- **Pass Rate:** **43/43 (100% Passed)**

