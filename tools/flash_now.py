#!/usr/bin/env python3
"""
Flashes tmp/app.uf2 to nice!nano board.
If NICENANO is not mounted, attempts mounting via diskutil or waiting for user double-tap.
"""

import os
import sys
import time
import shutil
import subprocess

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
UF2_PATH = os.path.join(PROJECT_ROOT, "tmp", "app.uf2")

def find_nicenano_disk():
    res = subprocess.run(["diskutil", "list"], capture_output=True, text=True)
    for line in res.stdout.splitlines():
        if "NICENANO" in line and "disk" in line:
            # line example: 0: NICENANO *33.7 MB disk4
            parts = line.strip().split()
            for p in parts:
                if p.startswith("disk"):
                    return p
    return None

def main():
    print("Checking for NICENANO bootloader drive...")
    
    if not os.path.exists("/Volumes/NICENANO"):
        disk = find_nicenano_disk()
        if disk:
            print(f"Found unmounted NICENANO disk {disk}. Mounting...")
            subprocess.run(["diskutil", "mount", disk])
            time.sleep(1)

    if not os.path.exists("/Volumes/NICENANO"):
        print("Please double-tap RESET on Board A (Probe) now.")
        for _ in range(120):
            disk = find_nicenano_disk()
            if disk:
                subprocess.run(["diskutil", "mount", disk])
            if os.path.exists("/Volumes/NICENANO"):
                break
            time.sleep(1)

    if not os.path.exists("/Volumes/NICENANO"):
        print("❌ Could not find /Volumes/NICENANO. Please double-tap RESET on Board A.")
        sys.exit(1)

    print("Flashing tmp/app.uf2...")
    res = subprocess.run(["cp", "-X", UF2_PATH, "/Volumes/NICENANO/"])
    if res.returncode == 0:
        print("✅ Flashed successfully! Waiting for re-enumeration...")
        time.sleep(3)
        res_test = subprocess.run([sys.executable, os.path.join(PROJECT_ROOT, "tools", "run_hil_tests.py")])
        sys.exit(res_test.returncode)
    else:
        print("❌ Flash failed.")
        sys.exit(1)

if __name__ == "__main__":
    main()
