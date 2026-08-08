#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(not(target_arch = "arm"))]
fn main() {}


use core::mem::MaybeUninit;
use nrf52840_hal::clocks::{Clocks, ExternalOscillator, Internal, LfOscStopped};
use nrf52840_hal::gpio::{p0, Level, Output, Pin, PushPull};
use nrf52840_hal::usbd::UsbPeripheral;
use embedded_hal::digital::{OutputPin, StatefulOutputPin};
use rtic_monotonics::nrf::timer::prelude::*;
use dap_rs::usb_device::class_prelude::UsbBusAllocator;
use dap_rs::usb_device::device::UsbDeviceState;
use dap_rs::usb_device::{class_prelude::*, prelude::*};
use usbd_serial::SerialPort;
use rusty_probe_nicenano::config::APP_VTOR_OFFSET;
use rusty_probe_nicenano::{Mono, UsbBus};

type AppClocks = Clocks<ExternalOscillator, Internal, LfOscStopped>;

#[rtic::app(device = nrf52840_hal::pac, dispatchers = [SWI0_EGU0, SWI1_EGU1])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        usb_dev: UsbDevice<'static, UsbBus>,
        serial: SerialPort<'static, UsbBus>,
    }

    #[local]
    struct Local {
        led: Pin<Output<PushPull>>,
    }

    #[init(local = [
        clocks: MaybeUninit<AppClocks> = MaybeUninit::uninit(),
        usb_alloc: MaybeUninit<UsbBusAllocator<UsbBus>> = MaybeUninit::uninit(),
    ])]
    fn init(cx: init::Context) -> (Shared, Local) {
        // One-time self-reset: the UF2 bootloader appears to jump into the
        // application without a full system reset, leaving the on-chip USB 3.3V
        // regulator disabled (POWER.USBREGSTATUS.OUTPUTRDY stays 0 even though
        // VBUS is present). A software reset re-arms VBUS detection so the
        // regulator comes up. We guard with GPREGRET to reset exactly once.
        rusty_probe_nicenano::perform_one_time_self_reset();

        unsafe {
            cx.core.SCB.vtor.write(APP_VTOR_OFFSET);
        }
        let dp = cx.device;
        Mono::start(dp.TIMER1);

        let clocks = Clocks::new(dp.CLOCK).enable_ext_hfosc();
        let clocks: &'static AppClocks = cx.local.clocks.write(clocks);

        let port0 = p0::Parts::new(dp.P0);
        let led = port0.p0_15.into_push_pull_output(Level::Low).degrade();

        let usb_periph = UsbPeripheral::new(dp.USBD, clocks);
        let usb_alloc: &'static UsbBusAllocator<UsbBus> =
            cx.local.usb_alloc.write(UsbBusAllocator::new(UsbBus::new(usb_periph)));
        let mut serial = SerialPort::new(usb_alloc);
        let descriptors = StringDescriptors::new(LangID::EN)
            .manufacturer("diag")
            .product("CDC test")
            .serial_number("TEST");
        let mut usb_dev = UsbDeviceBuilder::new(usb_alloc, UsbVidPid(rusty_probe_nicenano::config::USB_VID, rusty_probe_nicenano::config::USB_PID))
            .strings(&[descriptors])
            .unwrap()
            .device_class(0)
            .max_packet_size_0(64)
            .unwrap()
            .max_power(500)
            .unwrap()
            .composite_with_iads()
            .build();

        rusty_probe_nicenano::usb::enable_usbd_interrupts();

        // Kick-start the bus (first poll enables + pulls up D+).
        {
            let s = &mut serial;
            usb_dev.poll(&mut [s]);
        }

        blink::spawn().ok();
        (Shared { usb_dev, serial }, Local { led })
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(local = [led], shared = [usb_dev], priority = 1)]
    async fn blink(mut cx: blink::Context) {
        let led = cx.local.led;
        loop {
            let state = cx.shared.usb_dev.lock(|d| d.state());
            match state {
                UsbDeviceState::Configured => {
                    led.set_high().ok();
                    Mono::delay(500.millis()).await;
                }
                UsbDeviceState::Addressed => {
                    led.toggle().ok();
                    Mono::delay(250.millis()).await;
                }
                UsbDeviceState::Default => {
                    led.toggle().ok();
                    Mono::delay(100.millis()).await;
                }
                _ => {
                    led.toggle().ok();
                    Mono::delay(500.millis()).await;
                }
            }
        }
    }

    #[task(binds = USBD, priority = 2, shared = [usb_dev, serial])]
    fn on_usb(mut cx: on_usb::Context) {
        let mut buf = [0u8; 64];
        cx.shared.usb_dev.lock(|d| {
            cx.shared.serial.lock(|s| {
                if d.poll(&mut [s]) {
                    loop {
                        match s.read(&mut buf) {
                            Ok(count) if count > 0 => {
                                let _ = s.write(&buf[..count]);
                            }
                            _ => break,
                        }
                    }
                }
            })
        });
    }
}
