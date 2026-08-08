#!/usr/bin/env python3
"""
Consolidated Software DFU Trigger & Auto-Flasher for rusty-probe-nicenano.

Supports automatic CDC 1200-baud touch reset, waiting for Adafruit UF2 mount,
and flashing app.uf2 firmware to Board A.
"""

import os
import sys
import time
import shutil
import argparse
import subprocess

from common import PROJECT_ROOT, ProbeRsClient, find_nice_nano_dfu_mount, trigger_software_dfu

DEFAULT_UF2_PATH = os.path.join(PROJECT_ROOT, "tmp", "app.uf2")


def flash_firmware(auto_reset=True, mount=None, uf2_path=DEFAULT_UF2_PATH, timeout=60, run_tests=False, extra_pytest_args=None):
    print("==========================================")
    if auto_reset:
        print(" Automatic DFU Reset & Firmware Flasher")
    else:
        print(" Waiting for NICENANO USB Drive...")
    print("==========================================")

    mount_point = mount if (mount and os.path.exists(mount)) else find_nice_nano_dfu_mount()

    if not mount_point and auto_reset:
        trigger_software_dfu()
        print("Waiting for NICENANO drive to mount...")
        for _ in range(10):
            mount_point = find_nice_nano_dfu_mount()
            if mount_point:
                break
            time.sleep(0.5)

    if not mount_point:
        if auto_reset:
            print("Note: If software reset did not trigger, please double-tap RESET on Board A.")
        else:
            print("Please double-tap RESET on Board A if drive is not mounted yet.")

        for _ in range(timeout):
            mount_point = find_nice_nano_dfu_mount()
            if mount_point:
                break
            time.sleep(1)

    if not mount_point:
        raise RuntimeError("Could not find NICENANO drive")

    if not os.path.exists(uf2_path):
        raise RuntimeError(f"Firmware file '{uf2_path}' not found")

    print(f"✅ Found {mount_point}! Flashing {uf2_path}...")
    try:
        with open(uf2_path, "rb") as f_in:
            uf2_bytes = f_in.read()

        dest_path = os.path.join(mount_point, "app.uf2")
        written = False
        
        # Wait for OS mount write permissions to settle (macOS FAT volume indexing)
        for attempt in range(5):
            try:
                with open(dest_path, "wb") as f_out:
                    f_out.write(uf2_bytes)
                    try:
                        f_out.flush()
                    except Exception:
                        pass
                written = True
                print("✅ Firmware copied successfully!")
                break
            except (OSError, IOError) as e:
                # [Errno 13] Permission denied / Read-only right at open: OS still mounting volume
                if e.errno in (13, 30, 16) and attempt < 4:
                    time.sleep(0.5)
                    continue
                # If error occurred after write attempt (e.g. device unmounted on final block reboot)
                if written or "Input/output error" in str(e) or "Device not configured" in str(e):
                    print(f"✅ Firmware written! (Board reboot triggered: {e})")
                    written = True
                    break
                print(f"Write error on attempt {attempt + 1}: {e}")
                time.sleep(0.5)

        if not written:
            raise RuntimeError("Failed to write app.uf2 to NICENANO drive")

        # Verify whether Board A unmounted NICENANO volume and rebooted into application
        print("Checking for Board A application reboot...")
        rebooted = False
        for _ in range(10):
            time.sleep(0.5)
            if not os.path.exists(mount_point):
                rebooted = True
                break

        if rebooted:
            print("🚀 Success: Board A unmounted NICENANO drive and rebooted into application!")
        else:
            print("⚠️ Notice: NICENANO drive is still mounted. Bootloader may require single reset or re-touch.")

        if run_tests:
            print("Polling for probe re-enumeration before running tests...")
            client = ProbeRsClient()
            if not client.wait_for_probe(timeout=10.0):
                print("Warning: Probe re-enumeration check timed out, attempting to run pytest anyway...")

            test_file = os.path.join(PROJECT_ROOT, "tools", "test_hil.py")
            cmd = [sys.executable, "-m", "pytest", test_file, "-v"]
            if extra_pytest_args:
                cmd.extend(extra_pytest_args)
            res = subprocess.run(cmd)
            sys.exit(res.returncode)
    except Exception as e:
        raise RuntimeError(f"Flashing failed: {e}") from e


def main():
    parser = argparse.ArgumentParser(description="Flash rusty-probe-nicenano firmware via UF2 bootloader")
    parser.add_argument("--no-reset", action="store_true", help="Do not issue 1200-baud DFU reset touch")
    parser.add_argument("--mount", type=str, help="Custom path to NICENANO drive")
    parser.add_argument("--uf2", type=str, default=DEFAULT_UF2_PATH, help=f"Path to app.uf2 (default: {DEFAULT_UF2_PATH})")
    parser.add_argument("--timeout", type=int, default=60, help="Timeout in seconds to wait for USB drive (default: 60)")
    parser.add_argument("--test", action="store_true", help="Optionally run HIL test suite after flashing")
    args, extra_pytest_args = parser.parse_known_args()

    try:
        flash_firmware(
            auto_reset=not args.no_reset,
            mount=args.mount,
            uf2_path=args.uf2,
            timeout=args.timeout,
            run_tests=args.test,
            extra_pytest_args=extra_pytest_args
        )
    except RuntimeError as e:
        print(f"❌ {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
