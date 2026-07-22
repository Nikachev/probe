#!/usr/bin/env python3
"""
Auto-flash and test script for rusty-probe-nicenano.
Waits for /Volumes/NICENANO USB drive to appear, copies tmp/app.uf2,
and runs HIL test suite as soon as the probe enumerates.
"""

import os
import sys
import time
import shutil
import subprocess

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
UF2_PATH = os.path.join(PROJECT_ROOT, "tmp", "app.uf2")
VOLUME_PATH = "/Volumes/NICENANO"

def main():
    print("==========================================")
    print(" Waiting for /Volumes/NICENANO USB drive...")
    print(" (Please double-tap RESET on Board A)")
    print("==========================================")

    mounted = False
    for attempt in range(60):
        if os.path.exists(VOLUME_PATH):
            mounted = True
            break
        time.sleep(1)

    if not mounted:
        print("❌ Error: /Volumes/NICENANO drive did not appear within 60s.")
        sys.exit(1)

    print(f"✅ Found {VOLUME_PATH}! Copying {UF2_PATH}...")
    try:
        shutil.copy(UF2_PATH, VOLUME_PATH)
        print("✅ Firmware copied successfully! Waiting for reboot...")
    except Exception as e:
        print(f"❌ Copy failed: {e}")
        sys.exit(1)

    # Wait for USB enumeration
    print("Waiting 3 seconds for CMSIS-DAP probe to re-enumerate...")
    time.sleep(3)

    # Run HIL tests
    runner_script = os.path.join(PROJECT_ROOT, "tools", "run_hil_tests.py")
    res = subprocess.run([sys.executable, runner_script])
    sys.exit(res.returncode)

if __name__ == "__main__":
    main()
