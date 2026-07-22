#!/usr/bin/env python3
"""
Software DFU trigger & auto-flasher for rusty-probe-nicenano.
Sends 1200-baud touch or 'dfu' command over CDC serial to reboot into Adafruit UF2 bootloader,
copies tmp/app.uf2, and executes the HIL test suite.
"""

import os
import sys
import time
import shutil
import glob
import subprocess

from common import PROJECT_ROOT, find_nice_nano_dfu_mount

UF2_PATH = os.path.join(PROJECT_ROOT, "tmp", "app.uf2")


def trigger_software_dfu():
    """Triggers software DFU via 1200 baud touch or 'dfu' command on CDC serial port."""
    ports = glob.glob("/dev/tty.usbmodem*") + glob.glob("/dev/ttyACM*")
    for p in ports:
        try:
            print(f"Sending 1200-baud DFU reset touch to {p}...")
            if sys.platform == "darwin":
                subprocess.run(["stty", "-f", p, "1200"], capture_output=True, timeout=1)
            else:
                subprocess.run(["stty", "-F", p, "1200"], capture_output=True, timeout=1)
        except Exception:
            pass


def main():
    print("==========================================")
    print(" Automatic DFU Reset & Firmware Flasher")
    print("==========================================")

    mount_point = find_nice_nano_dfu_mount()

    if not mount_point:
        # Trigger software reboot into UF2 bootloader
        trigger_software_dfu()
        print("Waiting for NICENANO drive to mount...")
        for _ in range(10):
            mount_point = find_nice_nano_dfu_mount()
            if mount_point:
                break
            time.sleep(0.5)

    if not mount_point:
        print("Note: If software reset did not trigger, please double-tap RESET on Board A.")
        for _ in range(60):
            mount_point = find_nice_nano_dfu_mount()
            if mount_point:
                break
            time.sleep(1)

    if not mount_point:
        print("❌ Error: Could not find NICENANO drive.")
        sys.exit(1)

    print(f"✅ Found {mount_point}! Flashing {UF2_PATH}...")
    try:
        shutil.copy(UF2_PATH, mount_point)
        print("✅ Firmware copied successfully! Waiting for probe re-enumeration...")
        time.sleep(3)
        runner_script = os.path.join(PROJECT_ROOT, "tools", "run_hil_tests.py")
        res = subprocess.run([sys.executable, runner_script])
        sys.exit(res.returncode)
    except Exception as e:
        print(f"❌ Flashing failed: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
