#![cfg_attr(not(test), no_std)]

#[cfg(target_arch = "arm")]
use defmt_rtt as _;
#[cfg(all(target_arch = "arm", not(test)))]
use panic_probe as _;

pub mod config;
pub mod device_signature;
pub mod swd;
#[cfg(target_arch = "arm")]
pub mod usb;

#[cfg(target_arch = "arm")]
use rtic_monotonics::nrf::timer::prelude::*;
#[cfg(target_arch = "arm")]
nrf_timer1_monotonic!(Mono, 1_000_000); // TIMER1, 1 MHz -> 1 us resolution

#[cfg(target_arch = "arm")]
defmt::timestamp!("{=u64:us}", { Mono::now().duration_since_epoch().to_micros() });

/// Concrete USB bus type for the nRF52840 USBD peripheral.
///
/// The `UsbPeripheral` borrows the `Clocks` object; we keep the clocks alive
/// for `'static` (stored in an `init` local), so the bus is `'static` too.
#[cfg(target_arch = "arm")]
pub type UsbBus = nrf52840_hal::usbd::Usbd<nrf52840_hal::usbd::UsbPeripheral<'static>>;

/// Trigger software reset into the Adafruit UF2 DFU bootloader.
/// Sets `GPREGRET = 0x57` (`DFU_MAGIC_UF2_RESET`) before resetting.
#[cfg(target_arch = "arm")]
pub fn reset_to_bootloader() -> ! {
    let power = unsafe { &*nrf52840_hal::pac::POWER::ptr() };
    power.gpregret.write(|w| unsafe { w.bits(config::DFU_MAGIC_UF2_RESET) });
    cortex_m::peripheral::SCB::sys_reset();
}

/// Perform a one-time software reset to re-arm VBUS detection and the USB 3.3V regulator
/// when jumping from the Adafruit UF2 bootloader.
#[cfg(target_arch = "arm")]
pub fn perform_one_time_self_reset() {
    let power = unsafe { &*nrf52840_hal::pac::POWER::ptr() };
    if power.gpregret.read().bits() != config::GPREGRET_BOOTLOADER_CHECK {
        power.gpregret.write(|w| unsafe { w.bits(config::GPREGRET_BOOTLOADER_CHECK) });
        cortex_m::peripheral::SCB::sys_reset();
    }
    power.gpregret.write(|w| unsafe { w.bits(0) });
}

