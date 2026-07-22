//! Unique device identifier derived from the nRF52840 FICR (Factory
//! Information Configuration Registers).
//!
//! Replaces the RP2040 QSPI-flash UID reading from the original firmware.

use nrf52840_hal::pac;
use static_cell::StaticCell;

// 8 bytes of DEVICEID -> 16 hex characters.
const DEVICE_ID_LEN: usize = 16;
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
    // SAFETY: FICR is a read-only peripheral; reading is always sound.
    let ficr = unsafe { &*pac::FICR::ptr() };
    let id0 = ficr.deviceid[0].read().bits();
    let id1 = ficr.deviceid[1].read().bits();

    let mut bytes = [0u8; 8];
    bytes[0..4].copy_from_slice(&id0.to_be_bytes());
    bytes[4..8].copy_from_slice(&id1.to_be_bytes());

    let out = bytes_to_hex_16(&bytes);

    let id = DEVICE_ID_STR.init(out);
    // SAFETY: `out` only contains ASCII hex digits.
    unsafe { core::str::from_utf8_unchecked(id) }
}

