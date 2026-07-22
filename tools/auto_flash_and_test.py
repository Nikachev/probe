#!/usr/bin/env python3
"""
Auto-flash and test script for rusty-probe-nicenano.
Waits for NICENANO USB drive to appear (cross-platform macOS/Linux), copies tmp/app.uf2,
and runs HIL test suite as soon as the probe re-enumerates.
"""

import os
import sys
import time
import shutil
import glob
import argparse
import subprocess

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DEFAULT_UF2_PATH = os.path.join(PROJECT_ROOT, "tmp", "app.uf2")


def find_nicenano_drive(custom_path=None):
    """Locates the NICENANO mount point on macOS or Linux."""
    if custom_path and os.path.exists(custom_path):
        return custom_path

    possible_paths = [
        "/Volumes/NICENANO",  # macOS
    ]
    # Add Linux user media mount patterns
    possible_paths.extend(glob.glob("/media/*/NICENANO"))
    possible_paths.extend(glob.glob("/run/media/*/NICENANO"))
    possible_paths.extend(glob.glob("/mnt/NICENANO"))

    for p in possible_paths:
        if os.path.exists(p):
            return p
    return None


def main():
    parser = argparse.ArgumentParser(description="Auto-flash and test rusty-probe-nicenano")
    parser.add_argument("--mount", type=str, help="Custom path to NICENANO drive")
    parser.add_argument("--uf2", type=str, default=DEFAULT_UF2_PATH, help=f"Path to app.uf2 (default: {DEFAULT_UF2_PATH})")
    parser.add_argument("--timeout", type=int, default=60, help="Timeout in seconds to wait for USB drive (default: 60)")
    args = parser.parse_args()

    print("==========================================")
    print(" Waiting for NICENANO USB drive...")
    print(" (Please double-tap RESET on Board A)")
    print("==========================================")

    mount_point = None
    for attempt in range(args.timeout):
        mount_point = find_nicenano_drive(args.mount)
        if mount_point:
            break
        time.sleep(1)

    if not mount_point:
        print(f"❌ Error: NICENANO drive did not appear within {args.timeout}s.")
        sys.exit(1)

    print(f"✅ Found {mount_point}! Copying {args.uf2}...")
    try:
        shutil.copy(args.uf2, mount_point)
        print("✅ Firmware copied successfully! Waiting for reboot...")
    except Exception as e:
        print(f"❌ Copy failed: {e}")
        sys.exit(1)

    print("Waiting 3 seconds for CMSIS-DAP probe to re-enumerate...")
    time.sleep(3)

    runner_script = os.path.join(PROJECT_ROOT, "tools", "run_hil_tests.py")
    res = subprocess.run([sys.executable, runner_script])
    sys.exit(res.returncode)


if __name__ == "__main__":
    main()
