#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

// Target Blinky Firmware for HIL testing.
//
// Features:
// - VTOR relocation (after MBR, no SoftDevice).
// - Well-known static symbols in SRAM for read/write verification.
// - LED blinking on P0.15 for visual indication of target execution.
// - Counter variable in SRAM incremented each loop.

#[cfg(not(target_arch = "arm"))]
fn main() {}


use cortex_m_rt::entry;
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
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
pub static SRAM_MAGIC_1: AtomicU32 = AtomicU32::new(0xDEADBEEF);

#[no_mangle]
pub static SRAM_MAGIC_2: AtomicU32 = AtomicU32::new(0xCAFEBABE);

#[no_mangle]
pub static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub static mut SRAM_TEST_BUFFER: [u8; 256] = [0xAA; 256];

fn delay_ms(ms: u32) {
    cortex_m::asm::delay((DEFAULT_CPU_FREQUENCY / 1000) * ms);
}

#[entry]
fn main() -> ! {
    // VTOR relocation: application is linked at 0x1000 (after MBR).
    unsafe {
        let cp = cortex_m::Peripherals::steal();
        cp.SCB.vtor.write(APP_VTOR_OFFSET);
    }

    let dp = hal::pac::Peripherals::take().unwrap();
    let port0 = hal::gpio::p0::Parts::new(dp.P0);
    let mut led = port0.p0_15.into_push_pull_output(hal::gpio::Level::Low);

    loop {
        led.set_high().ok();
        delay_ms(200);
        led.set_low().ok();
        delay_ms(200);

        TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
}
