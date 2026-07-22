#![no_std]
#![no_main]

//! Target RTT Logging & Echo Firmware for HIL testing.
//!
//! Features:
//! - High-rate RTT log emission via defmt.
//! - RTT control block symbol `_SEGGER_RTT` exposed for host probing.
//! - Verification of fast log streaming over CMSIS-DAP SWD.

use cortex_m_rt::entry;
use defmt_rtt as _;
use nrf52840_hal as hal;
use panic_probe as _;

#[no_mangle]
pub static mut RTT_PACKET_COUNT: u32 = 0;

const CYCLES_PER_MS: u32 = 64_000;

fn delay_ms(ms: u32) {
    cortex_m::asm::delay(CYCLES_PER_MS * ms);
}

#[entry]
fn main() -> ! {
    unsafe {
        let cp = cortex_m::Peripherals::steal();
        cp.SCB.vtor.write(0x0002_6000);
    }

    let _dp = hal::pac::Peripherals::take().unwrap();

    let mut count: u32 = 0;
    loop {
        defmt::info!("HIL RTT Test Packet #{=u32}", count);
        unsafe {
            RTT_PACKET_COUNT = count;
        }
        count = count.wrapping_add(1);
        delay_ms(50);
    }
}
