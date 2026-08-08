#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

// Target RTT Logging & Echo Firmware for HIL testing.
//
// Features:
// - High-rate RTT log emission via defmt.
// - RTT control block symbol `_SEGGER_RTT` exposed for host probing.
// - Verification of fast log streaming over CMSIS-DAP SWD.

#[cfg(not(target_arch = "arm"))]
fn main() {}


use cortex_m_rt::entry;
use defmt_rtt as _;
use nrf52840_hal as hal;
use panic_probe as _;

use rusty_probe_nicenano::config::{APP_VTOR_OFFSET, DEFAULT_CPU_FREQUENCY};

/// Initial SP + Reset_Handler for SWD-flashed targets (Board B has no MBR at 0x0).
#[cfg(target_arch = "arm")]
#[link_section = ".boot_vectors"]
#[no_mangle]
pub static BOOT_VECTORS: [u32; 2] = [0x2004_0000, 0x0000_1005];

use core::sync::atomic::{AtomicU32, Ordering};

#[no_mangle]
pub static RTT_PACKET_COUNT: AtomicU32 = AtomicU32::new(0);

fn delay_ms(ms: u32) {
    cortex_m::asm::delay((DEFAULT_CPU_FREQUENCY / 1000) * ms);
}

#[entry]
fn main() -> ! {
    unsafe {
        let cp = cortex_m::Peripherals::steal();
        cp.SCB.vtor.write(APP_VTOR_OFFSET);
    }

    let _dp = unsafe { hal::pac::Peripherals::steal() };

    let mut count: u32 = 0;
    loop {
        defmt::info!("HIL RTT Test Packet #{=u32}", count);
        RTT_PACKET_COUNT.store(count, Ordering::Relaxed);
        count = count.wrapping_add(1);
        delay_ms(50);
    }
}
