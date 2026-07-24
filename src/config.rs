//! Hardware and system configuration constants for nice!nano v2 (nRF52840).

/// Reported DAP_Info "Firmware Version".
pub const FIRMWARE_VERSION: &str = "nice!nano v2 CMSIS-DAP 0.1.0";

/// USB Vendor ID (pid.codes open source VID).
pub const USB_VID: u16 = 0x1209;

/// USB Product ID (Rusty Probe PID).
pub const USB_PID: u16 = 0x4853;

/// Reported USB Manufacturer string.
pub const USB_MANUFACTURER: &str = "Probe-rs development team";

/// Reported USB Product string.
pub const USB_PRODUCT: &str = "Rusty Probe (nice!nano) with CMSIS-DAP v1/v2 Support";

/// Vector Table Offset Register (VTOR) offset when linked above SoftDevice S140.
pub const APP_VTOR_OFFSET: u32 = 0x0002_6000;

/// Magic byte written to GPREGRET to indicate one-time DFU reset boot state.
pub const GPREGRET_BOOTLOADER_CHECK: u32 = 0xAB;

/// Magic byte written to GPREGRET to trigger Adafruit UF2 bootloader reset (`0x57`).
pub const DFU_MAGIC_UF2_RESET: u32 = 0x57;

/// Default SWD CPU clock frequency for nRF52840 (64 MHz).
pub const DEFAULT_CPU_FREQUENCY: u32 = 64_000_000;

/// Default SWD target clock frequency (5 MHz).
pub const DEFAULT_MAX_SWD_FREQUENCY: u32 = 5_000_000;

/// Pin numbers for nice!nano v2 SWD probe interface.
pub const DEFAULT_SWDIO_PIN: u8 = 20;
pub const DEFAULT_SWCLK_PIN: u8 = 17;
pub const DEFAULT_NRESET_PIN: u8 = 22;

/// Duration of hardware nRESET pulse in milliseconds (10 ms).
pub const NRESET_PULSE_MS: u32 = 10;

/// Number of CPU delay ticks for 10 ms nRESET pulse at 64 MHz core frequency.
pub const NRESET_PULSE_TICKS: u32 = DEFAULT_CPU_FREQUENCY / 1000 * NRESET_PULSE_MS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nreset_pulse_ticks() {
        assert_eq!(NRESET_PULSE_TICKS, 640_000);
        assert_eq!(NRESET_PULSE_MS, 10);
    }

    #[test]
    fn test_config_constants() {
        assert_eq!(USB_VID, 0x1209);
        assert_eq!(USB_PID, 0x4853);
        assert_eq!(APP_VTOR_OFFSET, 0x0002_6000);
    }
}
