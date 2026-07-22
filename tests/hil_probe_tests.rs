//! Integration HIL tests for rusty-probe-nicenano firmware.
//!
//! Note: These tests validate probe enumeration and CMSIS-DAP functionality
//! when Board A (Probe) is attached via USB to the host and connected via SWD
//! to Board B (Target).

#[test]
fn test_hil_target_firmware_binaries_exist() {
    let targets = ["target_blinky.elf", "target_rtt.elf", "target_fault.elf"];
    let tmp_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/test-targets");

    for target in &targets {
        let path = tmp_dir.join(target);
        assert!(
            path.exists(),
            "Target binary {:?} does not exist. Run tools/build-test-targets.sh first.",
            path
        );
    }
}
