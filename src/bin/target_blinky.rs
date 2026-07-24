#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

// Target Blinky Firmware for HIL testing.
//
// Features:
// - VTOR relocation to 0x26000 (nice!nano SoftDevice offset).
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

#[cfg(target_arch = "arm")]
#[link_section = ".boot_vectors"]
#[no_mangle]
pub static BOOT_VECTORS: [u32; 2] = [0x2004_0000, 0x0002_6005];

#[no_mangle]
pub static mut SRAM_MAGIC_1: u32 = 0xDEADBEEF;

#[no_mangle]
pub static mut SRAM_MAGIC_2: u32 = 0xCAFEBABE;

#[no_mangle]
pub static mut TEST_COUNTER: u32 = 0;

#[no_mangle]
pub static mut SRAM_TEST_BUFFER: [u8; 256] = [0xAA; 256];

fn delay_ms(ms: u32) {
    cortex_m::asm::delay((DEFAULT_CPU_FREQUENCY / 1000) * ms);
}

#[entry]
fn main() -> ! {
    // Application is linked at 0x26000 (after Adafruit UF2 Bootloader + SoftDevice S140).
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

        unsafe {
            TEST_COUNTER = TEST_COUNTER.wrapping_add(1);
        }
    }
}
