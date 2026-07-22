#![cfg_attr(not(test), no_std)]

use defmt_rtt as _;
#[cfg(not(test))]
use panic_probe as _;


pub mod device_signature;
pub mod swd;
pub mod usb;

use rtic_monotonics::nrf::timer::prelude::*;
nrf_timer1_monotonic!(Mono, 1_000_000); // TIMER1, 1 MHz -> 1 us resolution

defmt::timestamp!("{=u64:us}", { Mono::now().duration_since_epoch().to_micros() });

/// Concrete USB bus type for the nRF52840 USBD peripheral.
///
/// The `UsbPeripheral` borrows the `Clocks` object; we keep the clocks alive
/// for `'static` (stored in an `init` local), so the bus is `'static` too.
pub type UsbBus = nrf52840_hal::usbd::Usbd<nrf52840_hal::usbd::UsbPeripheral<'static>>;

/// Trigger software reset into the Adafruit UF2 DFU bootloader.
/// Sets `GPREGRET = 0x57` (`DFU_MAGIC_UF2_RESET`) before resetting.
pub fn reset_to_bootloader() -> ! {
    let power = unsafe { &*nrf52840_hal::pac::POWER::ptr() };
    power.gpregret.write(|w| unsafe { w.bits(0x57) });
    cortex_m::peripheral::SCB::sys_reset();
}
