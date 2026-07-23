#!/usr/bin/env python3
"""
Host Unit Test Runner for rusty-probe-nicenano.

Runs all pure Rust unit tests on the host system without requiring
an attached nRF52840 target board.
"""

import sys
import subprocess

def get_host_target():
    try:
        out = subprocess.check_output(["rustc", "-vV"], text=True)
        for line in out.splitlines():
            if line.startswith("host:"):
                return line.split(":")[1].strip()
    except Exception as e:
        print(f"Warning: Failed to query host target from rustc: {e}")
    return None

def main():
    host_target = get_host_target()
    cmd = ["cargo", "test", "--lib"]
    if host_target:
        cmd.extend(["--target", host_target])
    
    cmd.extend(sys.argv[1:])
    print(f"Running Host Unit Tests: {' '.join(cmd)}")
    res = subprocess.run(cmd)
    sys.exit(res.returncode)

if __name__ == "__main__":
    main()
