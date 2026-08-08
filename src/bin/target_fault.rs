#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

// Target Fault Generator Firmware for HIL testing.
//
// Features:
// - Allows triggering HardFaults, Breakpoints, or infinite loops on demand.
// - Host test runner can write to `FAULT_TRIGGER_MODE` in SRAM via SWD to trigger faults.
// - Verifies debugger ability to halt target, inspect CPU registers, and handle exceptions.

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

/// Fault trigger mode: 0 = Normal loop, 1 = BKPT, 2 = Invalid Address Read, 3 = Division by zero
#[no_mangle]
pub static FAULT_TRIGGER_MODE: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub static FAULT_STATUS: AtomicU32 = AtomicU32::new(0xAA55AA55);

fn delay_ms(ms: u32) {
    cortex_m::asm::delay((DEFAULT_CPU_FREQUENCY / 1000) * ms);
}

#[entry]
fn main() -> ! {
    unsafe {
        let cp = cortex_m::Peripherals::steal();
        cp.SCB.vtor.write(APP_VTOR_OFFSET);
    }

    let _dp = hal::pac::Peripherals::take().unwrap();

    loop {
        let mode = FAULT_TRIGGER_MODE.load(Ordering::Relaxed);
        match mode {
            1 => {
                // Trigger breakpoint
                cortex_m::asm::bkpt();
            }
            2 => {
                // Invalid memory read (bad pointer dereference)
                let ptr = 0xFF00_0000 as *const u32;
                let val = unsafe { core::ptr::read_volatile(ptr) };
                FAULT_STATUS.store(val, Ordering::Relaxed);
            }
            3 => {
                // Trigger a division by zero (UsageFault if DIV_0_TRP is set in CCR)
                let zero: u32 = unsafe { core::ptr::read_volatile(&0u32) };
                let result = 1u32.wrapping_div(zero);
                FAULT_STATUS.store(result, Ordering::Relaxed);
            }
            _ => {
                // Normal idle heartbeat
                delay_ms(100);
            }
        }
    }
}
