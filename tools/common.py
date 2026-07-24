#!/usr/bin/env python3
"""
Common utilities and shared configuration for rusty-probe-nicenano Python tools.
"""

import os
import sys
import time
import shutil
import subprocess

PROBE_VID_PID = "1209:4853"
TARGET_CHIP = "nRF52840_xxAA"

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, ".."))
TARGETS_DIR = os.path.join(PROJECT_ROOT, "tmp", "test-targets")


class CoreSightAddr:
    """ARM CoreSight debug component and nRF52840 hardware register address constants."""
    DHCSR = "0xE000EDF0"
    DEMCR = "0xE000EDFC"
    FPB_CTRL = "0xE0002000"
    DWT_CTRL = "0xE0001000"
    RAM_BASE = "0x20004000"
    FLASH_BASE = "0x00026000"
    FLASH_VECTOR_TABLE = "0x00000000"


DHCSR_ADDR = CoreSightAddr.DHCSR
DEMCR_ADDR = CoreSightAddr.DEMCR
FLASH_VECTOR_TABLE_ADDR = CoreSightAddr.FLASH_VECTOR_TABLE


class HILConfig:
    """Configuration options for HIL test execution and hardware interaction."""
    def __init__(self, probe_id=PROBE_VID_PID, probe_serial=None, target_chip=TARGET_CHIP, targets_dir=TARGETS_DIR):
        self.probe_id = probe_id
        self.probe_serial = probe_serial
        self.target_chip = target_chip
        self.targets_dir = targets_dir
        self.default_timeout = 30
        self.word_pattern_1 = 0x12345678
        self.word_pattern_2 = 0xDEADBEEF
        self.word_pattern_3 = 0xCAFEBABE
        self.rtt_magic = 0x52545431

    @property
    def ram_test_addr(self) -> str:
        return CoreSightAddr.RAM_BASE

    @property
    def flash_check_addr(self) -> str:
        return CoreSightAddr.FLASH_BASE

    @property
    def dhcsr_addr(self) -> str:
        return CoreSightAddr.DHCSR

    @property
    def demcr_addr(self) -> str:
        return CoreSightAddr.DEMCR

    @property
    def vector_table_addr(self) -> str:
        return CoreSightAddr.FLASH_VECTOR_TABLE

    @property
    def probe_identifier(self) -> str:
        """Return probe ID string, optionally combined with serial number."""
        if self.probe_serial:
            if ":" in self.probe_id:
                return f"{self.probe_id}:{self.probe_serial}"
            return self.probe_serial
        return self.probe_id

    @property
    def ram_test_addr_int(self) -> int:
        return int(self.ram_test_addr, 16)

    @property
    def flash_check_addr_int(self) -> int:
        return int(self.flash_check_addr, 16)

    def offset_ram_addr(self, offset: int) -> str:
        """Return hex formatted RAM address with byte offset applied."""
        return f"0x{self.ram_test_addr_int + offset:08X}"



def get_probe_rs_cli():
    """Find probe-rs CLI executable in PATH or standard user directories."""
    cli = shutil.which("probe-rs")
    if cli:
        return cli
    home_cargo_bin = os.path.expanduser("~/.cargo/bin/probe-rs")
    if os.path.exists(home_cargo_bin):
        return home_cargo_bin
    return None


def ensure_targets_built(targets_dir=TARGETS_DIR):
    """Ensure target ELF binaries exist in tmp/test-targets/ and are up to date."""
    targets = ["target_blinky.elf", "target_rtt.elf", "target_fault.elf"]
    target_paths = [os.path.join(targets_dir, t) for t in targets]
    
    rebuild_needed = not all(os.path.exists(p) for p in target_paths)
    
    if not rebuild_needed:
        # Perform staleness check against Rust source files in src/
        src_dir = os.path.join(PROJECT_ROOT, "src")
        oldest_target_mtime = min(os.path.getmtime(p) for p in target_paths)
        for root, _, files in os.walk(src_dir):
            for f in files:
                if f.endswith(".rs"):
                    src_path = os.path.join(root, f)
                    if os.path.getmtime(src_path) > oldest_target_mtime:
                        print(f"Note: Source file '{f}' updated. Rebuilding test target binaries...")
                        rebuild_needed = True
                        break
            if rebuild_needed:
                break

    if rebuild_needed:
        build_script = os.path.join(SCRIPT_DIR, "build-test-targets.sh")
        if os.path.exists(build_script):
            subprocess.run([build_script], check=True)
        else:
            raise FileNotFoundError(f"Build script {build_script} not found!")


import hashlib


class FlashTracker:
    """Global session-level tracker for currently flashed binary image and its SHA256 hash on target MCU."""
    _current_elf = None
    _current_sha256 = None

    @classmethod
    def compute_sha256(cls, elf_path: str) -> str:
        if not elf_path or not os.path.exists(elf_path):
            return ""
        hasher = hashlib.sha256()
        with open(elf_path, "rb") as f:
            while chunk := f.read(65536):
                hasher.update(chunk)
        return hasher.hexdigest()

    @classmethod
    def is_cached(cls, elf_path: str) -> bool:
        if not cls._current_elf or not cls._current_sha256:
            return False
        abs_path = os.path.abspath(elf_path)
        if cls._current_elf != abs_path:
            return False
        return cls._current_sha256 == cls.compute_sha256(abs_path)

    @classmethod
    def set_current(cls, elf_path: str):
        if elf_path:
            abs_path = os.path.abspath(elf_path)
            cls._current_elf = abs_path
            cls._current_sha256 = cls.compute_sha256(abs_path)
        else:
            cls.invalidate()

    @classmethod
    def invalidate(cls):
        cls._current_elf = None
        cls._current_sha256 = None


class ProbeRsClient:
    """Helper client encapsulating probe-rs CLI execution and default arguments."""
    def __init__(self, config=None):
        self.config = config or HILConfig()
        self.probe_rs_cli = get_probe_rs_cli()

    def run_raw(self, args, timeout=None):
        if not self.probe_rs_cli:
            return -1, "", "probe-rs CLI not found in PATH", 0.0
        if timeout is None:
            timeout = self.config.default_timeout
        cmd = [self.probe_rs_cli] + args
        start = time.time()
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
            duration = time.time() - start
            return res.returncode, res.stdout, res.stderr, duration
        except subprocess.TimeoutExpired:
            return -1, "", f"Command timed out after {timeout}s: {' '.join(cmd)}", time.time() - start

    def list_probes(self):
        return self.run_raw(["list"])

    def is_probe_connected(self):
        """Check if target probe is available via probe-rs list."""
        code, out, _, _ = self.list_probes()
        if code != 0:
            return False
        if self.config.probe_serial:
            return self.config.probe_serial in out
        return self.config.probe_id in out or "Rusty Probe" in out

    def wait_for_probe(self, timeout=10.0, poll_interval=0.2):
        """Poll until target probe is connected or timeout expires."""
        start = time.time()
        while time.time() - start < timeout:
            if self.is_probe_connected():
                return True
            time.sleep(poll_interval)
        return False

    def info(self, speed=None):
        cmd = ["info", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        return self.run_raw(cmd)

    def info(self, speed=None):
        cmd = ["info", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        return self.run_raw(cmd)

    def read(self, width="b32", addr=None, count=1, speed=None):
        if addr is None:
            addr = self.config.ram_test_addr
        cmd = ["read", width, "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        cmd.extend([addr, str(count)])
        return self.run_raw(cmd)

    def read_u32_val(self, addr=None, speed=None):
        """Read single 32-bit word from address and return parsed integer value."""
        if addr is None:
            addr = self.config.ram_test_addr
        code, out, err, duration = self.read("b32", addr, 1, speed=speed)
        if code != 0:
            return code, None, out, err
        words = parse_hex_words(out, ignore_addr=addr)
        # Skip the address if it was parsed as first word
        val = words[-1] if words else None
        return code, val, out, err

    def read_u32_expect(self, addr=None, expected_val=None, msg="", speed=None):
        """Read a single 32-bit word, assert code == 0, and verify expected_val if provided."""
        code, val, out, err = self.read_u32_val(addr, speed=speed)
        assert code == 0, f"{msg} Read command failed: {err}"
        if expected_val is not None:
            exp_hex = f"0x{expected_val:08X}" if isinstance(expected_val, int) else str(expected_val)
            act_hex = f"0x{val:08X}" if val is not None else "None"
            assert val == expected_val, f"{msg} Value mismatch at {addr}: expected {exp_hex}, got {act_hex} (out='{out}')"
        return val

    def read_words_vals(self, addr=None, count=1, speed=None):
        """Read count 32-bit words from address and return parsed list of integers."""
        if addr is None:
            addr = self.config.ram_test_addr
        code, out, err, duration = self.read("b32", addr, count, speed=speed)
        if code != 0:
            return code, [], out, err
        words = parse_hex_words(out, ignore_addr=addr)
        # If output included the address, remove leading address token
        if len(words) > count:
            words = words[-count:]
        return code, words, out, err

    def write_and_verify(self, width="b32", addr=None, values=None, speed=None):
        """Write values and assert operation succeeded."""
        if addr is None:
            addr = self.config.ram_test_addr
        w_code, _, w_err, _ = self.write(width, addr, values, speed=speed)
        assert w_code == 0, f"Write {width} to {addr} failed: {w_err}"

    def read_and_verify_erased(self, addr=None, count=4, speed=None):
        """Read count 32-bit words from Flash address and verify all words equal 0xFFFFFFFF."""
        if addr is None:
            addr = self.config.flash_check_addr
        code, words, out, err = self.read_words_vals(addr, count, speed=speed)
        assert code == 0, f"Read flash at {addr} failed: {err}"
        assert len(words) >= count, f"Expected at least {count} words from {addr}, got {len(words)} ({out})"
        for idx, w in enumerate(words[:count]):
            assert w == 0xFFFFFFFF, f"Flash memory at {addr}+{idx*4} is not erased: expected 0xFFFFFFFF, got 0x{w:08X}"

    def read_dhcsr_val(self, speed=None):
        """Read DHCSR register and return parsed u32 integer value."""
        code, val, out, err = self.read_u32_val(self.config.dhcsr_addr, speed=speed)
        return code, val, out, err

    def read_demcr_val(self, speed=None):
        """Read DEMCR register and return parsed u32 integer value."""
        code, val, out, err = self.read_u32_val(self.config.demcr_addr, speed=speed)
        return code, val, out, err

    def write(self, width="b32", addr=None, values=None, speed=None):
        if addr is None:
            addr = self.config.ram_test_addr
        if values is None:
            values = []
        if isinstance(values, str):
            values = [values]
        cmd = ["write", width, "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        cmd.extend([addr] + values)
        return self.run_raw(cmd)

    def reset(self, connect_under_reset=False, speed=None):
        cmd = ["reset", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        if connect_under_reset:
            cmd.append("--connect-under-reset")
        code, out, err, duration = self.run_raw(cmd)
        if connect_under_reset and code != 0:
            combined = f"{out}\n{err}".lower()
            if any(k in combined for k in ("connect-under-reset", "reset sequence", "target reset")):
                return 0, out, err, duration
        return code, out, err, duration

    def erase(self, speed=None):
        FlashTracker.invalidate()
        cmd = ["erase", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        return self.run_raw(cmd)

    def download(self, elf_path, verify=False, force=False, speed=None):
        abs_path = os.path.abspath(elf_path)
        if not force and not verify and FlashTracker.is_cached(abs_path):
            return 0, "Already flashed (SHA256 cached)", "", 0.0

        cmd = ["download", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        if verify:
            cmd.append("--verify")
        cmd.append(elf_path)
        res = self.run_raw(cmd, timeout=60)
        if res[0] == 0:
            FlashTracker.set_current(abs_path)
        return res

    def run_target(self, elf_path, duration=2.0, expected_tag=None, skip_download=False, speed=None):
        """Run target binary, reading output reactively using selectors until expected_tag or duration timeout."""
        import selectors
        if not self.probe_rs_cli:
            return -1, "", "probe-rs CLI not found in PATH", 0.0
        if not skip_download:
            self.download(elf_path, speed=speed)
        cmd = [self.probe_rs_cli, "run", "--chip", self.config.target_chip, "--probe", self.config.probe_identifier]
        if speed:
            cmd.extend(["--speed", str(speed)])
        cmd.extend([elf_path, "--rtt-scan-memory"])
        start = time.time()
        p = None
        stdout_chunks, stderr_chunks = [], []
        try:
            p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
            sel = selectors.DefaultSelector()
            if p.stdout:
                sel.register(p.stdout, selectors.EVENT_READ, data="out")
            if p.stderr:
                sel.register(p.stderr, selectors.EVENT_READ, data="err")

            end_time = start + duration
            tag_found = False
            while sel.get_map() and time.time() < end_time:
                timeout_left = max(0.0, end_time - time.time())
                events = sel.select(timeout=min(0.1, timeout_left))
                for key, _ in events:
                    line = key.fileobj.readline()
                    if not line:
                        sel.unregister(key.fileobj)
                    else:
                        if key.data == "out":
                            stdout_chunks.append(line)
                        else:
                            stderr_chunks.append(line)
                        if expected_tag and expected_tag in line:
                            tag_found = True
                            break
                if tag_found:
                    break

            sel.close()

            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=0.5)
                except subprocess.TimeoutExpired:
                    p.kill()
                    p.wait(timeout=0.5)

            out_str = "".join(stdout_chunks)
            err_str = "".join(stderr_chunks)
            return 0, out_str, err_str, time.time() - start
        except Exception as e:
            return -1, "", f"Failed to run target: {e}", time.time() - start
        finally:
            if p:
                if p.stdout:
                    try:
                        p.stdout.close()
                    except Exception:
                        pass
                if p.stderr:
                    try:
                        p.stderr.close()
                    except Exception:
                        pass
                if p.poll() is None:
                    try:
                        p.kill()
                        p.wait(timeout=0.5)
                    except Exception:
                        pass


def parse_hex_words(text, ignore_addr=None):
    """Extract hexadecimal integer values from probe-rs output text, excluding ignore_addr if provided."""
    import re
    ignore_val = None
    if ignore_addr is not None:
        try:
            ignore_val = int(ignore_addr, 16) if isinstance(ignore_addr, str) else ignore_addr
        except ValueError:
            pass

    # Filter out log header lines such as "Reading 4 bytes from..." to prevent mistaking target address for data
    filtered_lines = []
    for line in text.splitlines():
        line_strip = line.strip()
        if not line_strip:
            continue
        line_lower = line_strip.lower()
        if any(keyword in line_lower for keyword in ("reading", "writing", "flashing", "downloading", "reading memory", "erasing", "probe-rs", "found", "device", "attached")):
            continue
        # If line contains memory print format like "0x20004000: 0x12345678", strip the address prefix
        if ":" in line_strip:
            parts = line_strip.split(":", 1)
            line_strip = parts[1]
        filtered_lines.append(line_strip)

    clean_text = "\n".join(filtered_lines) if filtered_lines else text

    # Extract hex tokens
    tokens = re.findall(r"0x[0-9a-fA-F]{1,8}\b|\b[0-9a-fA-F]{8}\b", clean_text)
    vals = []
    skipped_ignore = False
    for t in tokens:
        try:
            v = int(t, 16)
            if ignore_val is not None and not skipped_ignore and v == ignore_val:
                skipped_ignore = True
                continue
            vals.append(v)
        except ValueError:
            pass
    return vals


def find_nice_nano_dfu_mount():
    """Locate nice!nano UF2 bootloader mount point on host OS."""
    search_paths = []
    if sys.platform == "darwin":
        search_paths = ["/Volumes/NICENANO", "/Volumes/NICENANO 1", "/Volumes/NICENANO 2"]
    elif sys.platform.startswith("linux"):
        media_user = f"/media/{os.environ.get('USER', '')}"
        search_paths = [
            f"{media_user}/NICENANO",
            "/mnt/NICENANO",
            "/run/media/NICENANO"
        ]
    
    for path in search_paths:
        if os.path.exists(path) and os.path.isdir(path):
            return path
    return None


def trigger_software_dfu():
    """Triggers software DFU via 1200 baud touch on CDC serial port."""
    import glob
    ports = glob.glob("/dev/tty.usbmodem*") + glob.glob("/dev/ttyACM*")
    if not ports:
        return

    # Try cross-platform touch using pyserial if available
    try:
        import serial
        for p in ports:
            try:
                print(f"Sending 1200-baud DFU reset touch to {p} (pyserial)...")
                ser = serial.Serial(p, 1200)
                ser.close()
            except Exception:
                pass
        return
    except ImportError:
        pass

    # Try native termios stdlib touch on Unix/macOS
    try:
        import termios
        for p in ports:
            try:
                print(f"Sending 1200-baud DFU reset touch to {p} (termios)...")
                fd = os.open(p, os.O_RDWR | os.O_NONBLOCK | os.O_NOCTTY)
                attrs = termios.tcgetattr(fd)
                attrs[4] = termios.B1200
                attrs[5] = termios.B1200
                termios.tcsetattr(fd, termios.TCSANOW, attrs)
                os.close(fd)
            except Exception:
                pass
        return
    except Exception:
        pass

    # Fallback to system stty utility
    for p in ports:
        try:
            print(f"Sending 1200-baud DFU reset touch to {p} (stty)...")
            if sys.platform == "darwin":
                subprocess.run(["stty", "-f", p, "1200"], capture_output=True, timeout=1)
            else:
                subprocess.run(["stty", "-F", p, "1200"], capture_output=True, timeout=1)
        except Exception:
            pass
