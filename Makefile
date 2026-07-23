# Makefile for rusty-probe-nicenano firmware and HIL test suite

PYTHON ?= python3
PYTEST ?= pytest

.PHONY: help build build-targets flash test-unit test-hil test-hil-suite test-all clean

help:
	@echo "========================================================================"
	@echo " rusty-probe-nicenano Makefile Targets"
	@echo "========================================================================"
	@echo "  make build             - Build probe firmware release binary"
	@echo "  make build-targets     - Build HIL test target binaries (blinky, rtt, fault)"
	@echo "  make flash             - Auto-reset Board A into DFU and flash app.uf2"
	@echo "  make test-unit         - Run host Rust unit tests (offline)"
	@echo "  make test-hil          - Run full 30-case Pytest HIL test suite"
	@echo "  make test-hil-suite SUITE=N - Run specific HIL test suite (N=1..7)"
	@echo "  make test-all          - Build targets, run unit tests and full HIL suite"
	@echo "  make clean             - Clean build outputs (target/ and tmp/)"
	@echo "========================================================================"

build:
	tools/make-uf2.sh app

build-targets:
	tools/build-test-targets.sh

flash: build
	$(PYTHON) tools/flash.py

test-unit:
	$(PYTHON) tools/run_unit_tests.py $(ARGS)

test-hil: build-targets
	$(PYTEST) tools/test_hil.py -v $(ARGS)

test-hil-suite: build-targets
	@if [ -z "$(SUITE)" ]; then \
		echo "Error: Please specify SUITE=N (e.g. make test-hil-suite SUITE=3)"; \
		exit 1; \
	fi
	$(PYTEST) tools/test_hil.py -v -m suite$(SUITE) $(ARGS)

test-all: test-unit test-hil

clean:
	cargo clean
	rm -rf tmp/ .pytest_cache tools/__pycache__ tools/.pytest_cache
	@echo "✅ Cleaned target/, tmp/, and test cache directories"
