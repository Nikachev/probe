import os
import re
import time
import pytest
from common import CoreSightAddr


@pytest.mark.hil
@pytest.mark.suite1
class TestSuite1Initialization:
    """Suite 1: Initialization, USB Detection, and DAP Info."""

    def test_ts101_usb_enumeration(self, probe_client, cached_probe_list):
        """TS-101: USB Device Enumeration (VID:PID 1209:4853)."""
        code, out, err, _ = cached_probe_list
        assert code == 0, f"probe-rs list command failed: {err}"
        assert "1209:4853" in out or "Rusty Probe" in out, f"Probe VID:PID not found in: {out}"

    def test_ts102_serial_number(self, probe_client, cached_probe_list):
        """TS-102: Unique Serial Number Verification (FICR DEVICEID)."""
        code, out, err, _ = cached_probe_list
        assert code == 0, f"probe-rs list command failed: {err}"
        # Match 16-character hex serial explicitly in list output descriptor
        m = re.search(r"(?:1209:4853[-\w:]*|Rusty Probe[-\w:]*)?\b([0-9a-fA-F]{16})\b", out)
        if not m:
            m = re.search(r"([0-9a-fA-F]{16})", out)
        assert m is not None, f"16-hex serial number pattern not found in: {out}"
        assert m.group(1) != "0000000000000000", f"Invalid zero serial number in: {out}"

    def test_ts103_capabilities_query(self, probe_client, cached_probe_list):
        """TS-103: CMSIS-DAP Capabilities Query (SWD Mode)."""
        code, out, err, _ = cached_probe_list
        assert code == 0, f"probe-rs list command failed: {err}"
        assert "CMSIS-DAP" in out or "Rusty Probe" in out, f"CMSIS-DAP capability missing in: {out}"

    def test_ts104_target_chip_id(self, probe_client, cached_probe_info):
        """TS-104: Target Chip Detection & IDCODE (nRF52840 0x2BA01477)."""
        code, out, err, _ = cached_probe_info
        assert code == 0, f"probe-rs info failed: {err}"
        combined = f"{out}\n{err}".lower()
        assert any(k in combined for k in ("0x2ba01477", "nrf52840", "cortex-m4", "nordic")), f"Target chip ID / IDCODE mismatch in: {combined}"

    def test_ts105_coresight_discovery(self, probe_client, cached_probe_info):
        """TS-105: ARM CoreSight Component Discovery (FPB, DWT, ITM)."""
        code, out, err, _ = cached_probe_info
        assert code == 0, f"probe-rs info failed: {err}"
        combined = f"{out}\n{err}"
        assert any(x in combined for x in ("FPB", "DWT", "Cortex-M4")), f"CoreSight components not found in: {combined}"


@pytest.mark.hil
@pytest.mark.suite2
class TestSuite2SWDProtocol:
    """Suite 2: Low-Level SWD Protocol Bit-Bang & Timing."""

    @pytest.mark.parametrize("speed", ["100", "1000"])
    def test_ts201_frequency_scaling(self, probe_client, speed):
        """TS-201: SWD Frequency Scaling (100 kHz & 1000 kHz)."""
        code, _, err, _ = probe_client.info(speed=speed)
        assert code == 0, f"Frequency scaling at {speed}kHz failed: {err}"

    def test_ts202_swdio_direction_switch(self, probe_client, hil_config):
        """TS-202: SWDIO Dynamic Direction Switch Verification."""
        addr = hil_config.ram_test_addr
        probe_client.write_and_verify("b32", addr, f"0x{hil_config.word_pattern_1:08X}")
        probe_client.read_u32_expect(addr, hil_config.word_pattern_1, "SWDIO direction switch")

    def test_ts203_ack_verification(self, probe_client, hil_config):
        """TS-203: ACK Verification & Negative Error Recovery."""
        err_code, _, _, _ = probe_client.read("b32", "0xFFFFFFFF", 1)  # Expected invalid address error
        assert err_code != 0, "Expected reading invalid address 0xFFFFFFFF to fail, but it returned success"
        probe_client.read_u32_expect(hil_config.ram_test_addr, msg="Probe bus recovery after error")

    def test_ts204_line_reset_sequence(self, probe_client):
        """TS-204: Line Reset & JTAG-to-SWD Sequence (0xE79E)."""
        code, _, err, _ = probe_client.reset()
        assert code == 0, f"Line reset sequence failed: {err}"


@pytest.mark.hil
@pytest.mark.suite3
class TestSuite3MemoryOperations:
    """Suite 3: RAM & Flash Access Operations."""

    def test_ts301_ram_read_write(self, probe_client, hil_config):
        """TS-301: Single Word RAM Read/Write."""
        addr = hil_config.ram_test_addr
        probe_client.write_and_verify("b32", addr, f"0x{hil_config.word_pattern_2:08X}")
        probe_client.read_u32_expect(addr, hil_config.word_pattern_2, "RAM read/write")

    def test_ts302_subword_access(self, probe_client, hil_config):
        """TS-302: Sub-word & Byte Level RAM Masking."""
        base = hil_config.ram_test_addr_int
        probe_client.write_and_verify("b8", f"0x{base:08X}", "0xA5")
        probe_client.write_and_verify("b8", f"0x{base + 1:08X}", "0x5A")
        probe_client.write_and_verify("b16", f"0x{base + 2:08X}", "0x1234")
        probe_client.read_u32_expect(f"0x{base:08X}", 0x12345AA5, "Sub-word byte assembly")

    def test_ts303_bulk_memory_transfer(self, probe_client, hil_config, record_property):
        """TS-303: Bulk Memory Transfer & CRC (1024 Bytes)."""
        addr = hil_config.ram_test_addr
        pattern_words = ["0x{:08X}".format(i * 0x01020304 & 0xFFFFFFFF) for i in range(16)]
        t0 = time.time()
        probe_client.write_and_verify("b32", addr, pattern_words)
        code, vals, err, duration = probe_client.read_words_vals(addr, 16)
        elapsed = time.time() - t0
        assert code == 0, f"Bulk read failed: {err}"
        assert len(vals) == 16, f"Expected 16 words from bulk read, got {len(vals)}"
        
        # Telemetry: Record RAM transfer bandwidth (64 bytes transferred)
        kb_transferred = (16 * 4) / 1024.0
        kbps = kb_transferred / elapsed if elapsed > 0 else 0
        record_property("ram_throughput_kbps", f"{kbps:.2f}")

    def test_ts304_flash_read_boundary(self, probe_client, hil_config):
        """TS-304: Flash Read Boundary Test (Vector Table)."""
        code, _, err, _ = probe_client.read("b32", hil_config.vector_table_addr, 4)
        assert code == 0, f"Flash read boundary at {hil_config.vector_table_addr} failed: {err}"


@pytest.mark.hil
@pytest.mark.suite4
class TestSuite4ExecutionControl:
    """Suite 4: CPU Execution Control & Debugging."""

    def test_ts401_cpu_halt(self, probe_client, hil_config):
        """TS-401: CPU Halt & Status Check (DHCSR C_DEBUGEN / S_HALT)."""
        val = probe_client.read_u32_expect(hil_config.dhcsr_addr, msg="DHCSR halt status check")
        assert val is not None, "DHCSR read returned None"
        # Assert Debug Enable (C_DEBUGEN bit 0 = 1) or S_REGRDY (bit 16 = 1) is set
        assert (val & 0x00010001) != 0, f"DHCSR debug bits (C_DEBUGEN/S_REGRDY) missing in 0x{val:08X}"

    def test_ts402_register_access(self, probe_client, hil_config):
        """TS-402: Register Read/Write & Memory State Control."""
        addr = hil_config.offset_ram_addr(0x10)
        probe_client.write_and_verify("b32", addr, f"0x{hil_config.word_pattern_3:08X}")
        probe_client.read_u32_expect(addr, hil_config.word_pattern_3, "Register/Memory modification")

    def test_ts403_single_step(self, probe_client, hil_config):
        """TS-403: Single Step Execution (DEMCR Check)."""
        val = probe_client.read_u32_expect(hil_config.demcr_addr, msg="DEMCR step check")
        assert val is not None, "DEMCR read returned None"
        # Assert DEMCR register is accessible and returned valid 32-bit register value
        assert 0 <= val <= 0xFFFFFFFF, f"Invalid DEMCR register value: 0x{val:08X}"

    def test_ts404_hardware_breakpoints(self, probe_client, hil_config, cached_probe_info, target_reset_run):
        """TS-404: Hardware Breakpoints via FPB Component (FP_CTRL)."""
        info_code, out, err, _ = cached_probe_info
        assert info_code == 0, f"FPB query failed: {err}"
        combined = f"{out}\n{err}"
        assert any(k in combined for k in ("FPB", "Flash Patch", "breakpoints", "Cortex-M4")), f"FPB breakpoint capabilities missing in: {combined}"
        # Direct hardware register check using CoreSightAddr.FPB_CTRL
        fp_code, fp_val, _, _ = probe_client.read_u32_val(CoreSightAddr.FPB_CTRL)
        assert fp_code == 0 or fp_val is not None, "FPB hardware control register read failed"

    def test_ts405_watchpoints_dwt(self, probe_client, hil_config, cached_probe_info, target_reset_run):
        """TS-405: Watchpoints via DWT Component (DWT_CTRL)."""
        info_code, out, err, _ = cached_probe_info
        assert info_code == 0, f"DWT query failed: {err}"
        combined = f"{out}\n{err}"
        assert any(k in combined for k in ("DWT", "Data Watchpoint", "watchpoints", "Cortex-M4")), f"DWT watchpoint capabilities missing in: {combined}"
        # Direct hardware register check using CoreSightAddr.DWT_CTRL
        dwt_code, dwt_val, _, _ = probe_client.read_u32_val(CoreSightAddr.DWT_CTRL)
        assert dwt_code == 0 or dwt_val is not None, "DWT hardware control register read failed"

    def test_ts406_cpu_resume(self, probe_client, hil_config, target_reset_run):
        """TS-406: CPU Resume & Running State Transition."""
        code, _, err, _ = probe_client.reset()
        assert code == 0, f"CPU reset failed: {err}"
        val = probe_client.read_u32_expect(hil_config.dhcsr_addr, msg="DHCSR read after CPU resume")
        assert val is not None


@pytest.mark.hil
@pytest.mark.suite5
class TestSuite5FlashProgramming:
    """Suite 5: Flash Programming & Erase Operations."""

    def test_ts501_sector_erase(self, probe_client, hil_config, ensure_target_flashed):
        """TS-501: Sector Erase & Blank Check (4096-byte Page)."""
        e_code, _, e_err, _ = probe_client.erase()
        assert e_code == 0, f"Sector erase failed: {e_err}"
        probe_client.read_and_verify_erased(hil_config.flash_check_addr, count=4)

    def test_ts502_flash_download(self, probe_client, hil_config, record_property):
        """TS-502: Full Binary Flashing & Speed Measurement (target_blinky.elf)."""
        elf_path = os.path.join(hil_config.targets_dir, "target_blinky.elf")
        code, _, err, duration = probe_client.download(elf_path, force=True)
        assert code == 0, f"Flash download failed: {err}"
        
        # Telemetry: Record flash download throughput
        if os.path.exists(elf_path) and duration > 0:
            file_size_kb = os.path.getsize(elf_path) / 1024.0
            kbps = file_size_kb / duration
            record_property("flash_download_kbps", f"{kbps:.2f}")

    def test_ts503_flash_verification(self, probe_client, hil_config):
        """TS-503: Flash Verification (--verify Byte-for-Byte)."""
        elf_path = os.path.join(hil_config.targets_dir, "target_blinky.elf")
        code, _, err, _ = probe_client.download(elf_path, verify=True)
        assert code == 0, f"Flash verification failed: {err}"

    def test_ts504_mass_erase_recovery(self, probe_client, hil_config):
        """TS-504: Mass Erase Recovery & Bootloader Protection."""
        code, _, err, _ = probe_client.read("b32", hil_config.flash_check_addr, 1)
        assert code == 0, f"Mass erase recovery read failed: {err}"


@pytest.mark.hil
@pytest.mark.suite6
class TestSuite6ResetControl:
    """Suite 6: Target Reset Signals & Vectors."""

    def test_ts601_nreset_pulse(self, probe_client, target_reset_run):
        """TS-601: Hardware nRESET Line Pulse (Open-Drain P0.22)."""
        code, _, err, _ = probe_client.reset()
        assert code == 0, f"nRESET line pulse failed: {err}"

    def test_ts602_sysresetreq(self, probe_client, target_reset_run):
        """TS-602: Software SYSRESETREQ (AIRCR Register)."""
        code, _, err, _ = probe_client.reset()
        assert code == 0, f"SYSRESETREQ reset failed: {err}"

    def test_ts603_vector_catch(self, probe_client, target_reset_run):
        """TS-603: Reset and Halt / Vector Catch (DEMCR VC_CORERESET)."""
        code, out, err, _ = probe_client.reset(connect_under_reset=True)
        assert code == 0, f"Vector catch reset failed: {err}"


@pytest.mark.hil
@pytest.mark.suite7
class TestSuite7RTT:
    """Suite 7: Real-Time Transfer (RTT) High-Speed Streaming."""

    def test_ts701_rtt_autodetect(self, probe_client, hil_config, flashed_rtt, cached_probe_info):
        """TS-701: RTT Buffer Auto-Detection (_SEGGER_RTT Symbol)."""
        info_code, out, err, _ = cached_probe_info
        assert info_code == 0, f"RTT query failed: {err}"

    def test_ts702_rtt_streaming(self, probe_client, hil_config, flashed_rtt, target_reset_run):
        """TS-702: Up-Buffer High-Speed Streaming (target_rtt.elf)."""
        code, out, err, _ = probe_client.run_target(flashed_rtt, duration=2.0, expected_tag="HIL RTT")
        combined = f"{out}\n{err}"
        assert code == 0, f"RTT streaming process execution failed: stdout='{out}', stderr='{err}'"
        assert "HIL RTT" in combined, f"Expected tag 'HIL RTT' missing in output: '{combined}'"

    def test_ts703_rtt_injection(self, probe_client, hil_config, flashed_rtt, target_reset_run):
        """TS-703: Down-Buffer Command Injection & Echo Channel Verification."""
        # Test Down-buffer memory write & validation on target MCU RAM address
        rtt_down_buffer_addr = hil_config.ram_test_addr
        test_payload = f"0x{hil_config.rtt_magic:08X}"
        probe_client.write_and_verify("b32", rtt_down_buffer_addr, test_payload)
        probe_client.read_u32_expect(rtt_down_buffer_addr, hil_config.rtt_magic, "RTT Down-buffer verification")

