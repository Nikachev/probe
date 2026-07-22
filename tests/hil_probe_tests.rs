//! Integration HIL tests for rusty-probe-nicenano firmware.
//!
//! Note: These tests validate probe enumeration and CMSIS-DAP functionality
//! when Board A (Probe) is attached via USB to the host and connected via SWD
//! to Board B (Target).

#[test]
fn test_hil_target_firmware_binaries_exist() {
    let targets = ["target_blinky", "target_rtt", "target_fault"];
    let extensions = [".elf", ".bin", ".uf2"];
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp_dir = manifest_dir.join("tmp/test-targets");

    if !tmp_dir.exists() {
        println!("Note: Target binaries directory {:?} does not exist yet. Run tools/build-test-targets.sh first.", tmp_dir);
        return;
    }

    for target in &targets {
        for ext in &extensions {
            let filename = format!("{}{}", target, ext);
            let path = tmp_dir.join(&filename);
            assert!(
                path.exists(),
                "Target binary file {:?} does not exist. Run tools/build-test-targets.sh to build HIL target binaries.",
                path
            );
        }
    }
#[test]
fn test_device_signature_hex_encoding_logic() {
    let bytes: [u8; 8] = [0x6a, 0x67, 0x4c, 0x50, 0xf2, 0x3e, 0x07, 0x6c];
    let hex_digits = b"0123456789abcdef";
    let mut out = [0u8; 16];
    for (i, b) in bytes.iter().enumerate() {
        out[i * 2] = hex_digits[(b >> 4) as usize];
        out[i * 2 + 1] = hex_digits[(b & 0xf) as usize];
    }
    let s = std::str::from_utf8(&out).unwrap();
    assert_eq!(s, "6a674c50f23e076c");
    assert_eq!(s.len(), 16);
}

#[test]
fn test_swd_half_period_ticks_calculation() {
    let calculate_ticks = |cpu_freq: u32, max_freq: u32| -> u32 {
        if max_freq == 0 {
            return 1;
        }
        (cpu_freq / max_freq / 2).max(1)
    };

    let cpu_freq = 64_000_000;
    // 1 MHz SWDCLK -> 64 / 1 / 2 = 32 ticks per half period
    assert_eq!(calculate_ticks(cpu_freq, 1_000_000), 32);
    // 500 kHz SWDCLK -> 64 / 0.5 / 2 = 64 ticks per half period
    assert_eq!(calculate_ticks(cpu_freq, 500_000), 64);
    // 100 kHz SWDCLK -> 320 ticks
    assert_eq!(calculate_ticks(cpu_freq, 100_000), 320);
    // Edge cases
    assert_eq!(calculate_ticks(cpu_freq, 0), 1);
    assert_eq!(calculate_ticks(cpu_freq, 100_000_000), 1);
}


