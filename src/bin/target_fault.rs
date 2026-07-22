#![no_std]
#![no_main]

//! Target Fault Generator Firmware for HIL testing.
//!
//! Features:
//! - Allows triggering HardFaults, Breakpoints, or infinite loops on demand.
//! - Host test runner can write to `FAULT_TRIGGER_MODE` in SRAM via SWD to trigger faults.
//! - Verifies debugger ability to halt target, inspect CPU registers, and handle exceptions.

use cortex_m_rt::entry;
use defmt_rtt as _;
use nrf52840_hal as hal;
use panic_probe as _;

#[no_mangle]
pub static mut FAULT_TRIGGER_MODE: u32 = 0; // 0 = Normal loop, 1 = BKPT, 2 = Invalid Address Read, 3 = Division by zero

#[no_mangle]
pub static mut FAULT_STATUS: u32 = 0xAA55AA55;

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

    loop {
        let mode = unsafe { FAULT_TRIGGER_MODE };
        match mode {
            1 => {
                // Trigger breakpoint
                cortex_m::asm::bkpt();
            }
            2 => {
                // Invalid memory read (bad pointer dereference)
                let ptr = 0xFF00_0000 as *const u32;
                let val = unsafe { core::ptr::read_volatile(ptr) };
                unsafe { FAULT_STATUS = val };
            }
            _ => {
                // Normal idle heartbeat
                delay_ms(100);
            }
        }
    }
}
