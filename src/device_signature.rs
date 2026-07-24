//! Unique device identifier derived from the nRF52840 FICR (Factory
//! Information Configuration Registers).
//!
//! Replaces the RP2040 QSPI-flash UID reading from the original firmware.

#[cfg(target_arch = "arm")]
use nrf52840_hal::pac;
#[cfg(target_arch = "arm")]
use static_cell::StaticCell;

// 8 bytes of DEVICEID -> 16 hex characters.
const DEVICE_ID_LEN: usize = 16;
#[cfg(target_arch = "arm")]
static DEVICE_ID_STR: StaticCell<[u8; DEVICE_ID_LEN]> = StaticCell::new();

/// Convert 8 raw device ID bytes to a 16-byte ASCII hex array.
pub fn bytes_to_hex_16(bytes: &[u8; 8]) -> [u8; DEVICE_ID_LEN] {
    let hex = b"0123456789abcdef";
    let mut out = [0u8; DEVICE_ID_LEN];
    for (i, b) in bytes.iter().enumerate() {
        out[i * 2] = hex[(b >> 4) as usize];
        out[i * 2 + 1] = hex[(b & 0xf) as usize];
    }
    out
}

/// Returns a stable, unique hex string identifying this chip, suitable for use
/// as a USB serial number. The value is derived from `FICR.DEVICEID[0..2]`.
pub fn device_id_hex() -> &'static str {
    #[cfg(target_arch = "arm")]
    {
        // SAFETY: FICR is a read-only peripheral; reading is always sound.
        let ficr = unsafe { &*pac::FICR::ptr() };
        let id0 = ficr.deviceid[0].read().bits();
        let id1 = ficr.deviceid[1].read().bits();

        let b0 = id0.to_be_bytes();
        let b1 = id1.to_be_bytes();
        let bytes = [b0[0], b0[1], b0[2], b0[3], b1[0], b1[1], b1[2], b1[3]];

        let out = bytes_to_hex_16(&bytes);

        let id = DEVICE_ID_STR.init(out);
        // SAFETY: `out` only contains ASCII hex digits.
        unsafe { core::str::from_utf8_unchecked(id) }
    }
    #[cfg(not(target_arch = "arm"))]
    {
        "0123456789abcdef"
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_hex_16() {
        let input: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let hex = bytes_to_hex_16(&input);
        assert_eq!(&hex, b"0123456789abcdef");
    }

    #[test]
    fn test_bytes_to_hex_16_edge_cases() {
        let zeros = [0u8; 8];
        assert_eq!(&bytes_to_hex_16(&zeros), b"0000000000000000");

        let ones = [0xffu8; 8];
        assert_eq!(&bytes_to_hex_16(&ones), b"ffffffffffffffff");

        let mixed = [0x0f, 0xf0, 0x5a, 0xa5, 0x12, 0x34, 0x78, 0x90];
        assert_eq!(&bytes_to_hex_16(&mixed), b"0ff05aa512347890");
    }

    #[test]
    fn test_device_id_hex_host() {
        let id = device_id_hex();
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}



