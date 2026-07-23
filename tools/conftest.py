#!/usr/bin/env python3
"""
Pytest configuration and fixtures for rusty-probe-nicenano HIL test suite.
"""

import os
import pytest
from common import PROBE_VID_PID, TARGET_CHIP, TARGETS_DIR, HILConfig, ProbeRsClient, ensure_targets_built, get_probe_rs_cli


def pytest_addoption(parser):
    group = parser.getgroup("HIL Probe Testing")
    group.addoption(
        "--probe",
        action="store",
        default=PROBE_VID_PID,
        help=f"Probe VID:PID (default: {PROBE_VID_PID})"
    )
    group.addoption(
        "--serial",
        action="store",
        default=None,
        help="Probe unique serial number for multi-probe filtering"
    )
    group.addoption(
        "--chip",
        action="store",
        default=TARGET_CHIP,
        help=f"Target chip name (default: {TARGET_CHIP})"
    )
    group.addoption(
        "--suite",
        action="store",
        type=int,
        choices=range(1, 8),
        help="Run tests only for specific suite (1-7)"
    )


def pytest_runtest_setup(item):
    selected_suite = item.config.getoption("--suite")
    if selected_suite is not None:
        suite_marker = f"suite{selected_suite}"
        if not item.get_closest_marker(suite_marker):
            pytest.skip(f"Skipping: Test is not in suite {selected_suite}")

    if item.get_closest_marker("hil"):
        cli = get_probe_rs_cli()
        if not cli:
            pytest.skip("Skipping HIL test: 'probe-rs' CLI tool not found in PATH")


@pytest.fixture(scope="session")
def hil_config(request):
    probe_id = request.config.getoption("--probe")
    probe_serial = request.config.getoption("--serial")
    target_chip = request.config.getoption("--chip")
    return HILConfig(probe_id=probe_id, probe_serial=probe_serial, target_chip=target_chip, targets_dir=TARGETS_DIR)


@pytest.fixture(scope="session")
def probe_client(hil_config):
    ensure_targets_built(hil_config.targets_dir)
    client = ProbeRsClient(config=hil_config)
    if not client.is_probe_connected():
        pytest.skip(f"Skipping HIL tests: Probe '{hil_config.probe_identifier}' not connected")
    # Quick SWD healthcheck on target MCU (Board B)
    code, out, err, _ = client.info()
    if code != 0:
        pytest.skip(f"Skipping HIL tests: Target MCU not responding over SWD (Probe connected, but target check failed: {err.strip()})")
    return client


@pytest.fixture(scope="class")
def flashed_rtt(probe_client, hil_config):
    """Class-scoped fixture ensuring target_rtt.elf is flashed once per test class."""
    rtt_elf = os.path.join(hil_config.targets_dir, "target_rtt.elf")
    if os.path.exists(rtt_elf):
        code, _, err, _ = probe_client.download(rtt_elf)
        assert code == 0, f"Failed to flash target_rtt.elf: {err}"
    return rtt_elf


@pytest.fixture
def ensure_target_flashed(probe_client, hil_config):
    """Fixture ensuring target_blinky.elf binary is programmed on target MCU."""
    blinky_elf = os.path.join(hil_config.targets_dir, "target_blinky.elf")
    if os.path.exists(blinky_elf):
        probe_client.download(blinky_elf)
    yield


@pytest.fixture
def target_reset_run(probe_client):
    """Fixture ensuring target MCU is clean and running after test completion."""
    yield
    probe_client.reset()


@pytest.fixture(scope="session")
def cached_probe_list(probe_client):
    """Session-scoped cache for probe-rs list output."""
    return probe_client.list_probes()


@pytest.fixture(scope="session")
def cached_probe_info(probe_client):
    """Session-scoped cache for probe-rs info output."""
    return probe_client.info()



