#!/usr/bin/env python3
"""
Common utilities and shared configuration for rusty-probe-nicenano Python tools.
"""

import os
import sys
import shutil

PROBE_VID_PID = "1209:4853"
TARGET_CHIP = "nRF52840_xxAA"

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, ".."))
TARGETS_DIR = os.path.join(PROJECT_ROOT, "tmp", "test-targets")


class HILConfig:
    """Configuration options for HIL test execution and hardware interaction."""
    def __init__(self, probe_id=PROBE_VID_PID, target_chip=TARGET_CHIP, targets_dir=TARGETS_DIR):
        self.probe_id = probe_id
        self.target_chip = target_chip
        self.targets_dir = targets_dir
        self.ram_test_addr = "0x20004000"
        self.flash_check_addr = "0x00026000"
        self.default_timeout = 30


def get_probe_rs_cli():
    """Find probe-rs CLI executable in PATH or standard user directories."""
    cli = shutil.which("probe-rs")
    if cli:
        return cli
    home_cargo_bin = os.path.expanduser("~/.cargo/bin/probe-rs")
    if os.path.exists(home_cargo_bin):
        return home_cargo_bin
    return None


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
