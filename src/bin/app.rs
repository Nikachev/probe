#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(not(target_arch = "arm"))]
fn main() {}

#[cfg(target_arch = "arm")]
use rusty_probe_nicenano as _; // global logger + panic handler + Mono/UsbBus


/// SWD transport pins. These are dedicated GPIOs re-purposed as the CMSIS-DAP
/// SWD host. They are NOT the nice!nano on-board SWD/debug footprint (that one
/// is for *flashing the nice!nano itself*); these are the pins we drive to talk
/// to an external target. Change them here to match your wiring.
#[rtic::app(device = nrf52840_hal::pac, dispatchers = [SWI0_EGU0, SWI1_EGU1])]
mod app {
    use core::mem::MaybeUninit;
    use core::sync::atomic::Ordering;
    use dap_rs::dap::DapVersion;
    use dap_rs::usb::{Request, DAP2_PACKET_SIZE};
    use dap_rs::usb_device::class_prelude::UsbBusAllocator;
    use embedded_hal::digital::{OutputPin, StatefulOutputPin};
    use nrf52840_hal::clocks::{Clocks, ExternalOscillator, Internal, LfOscStopped};
    use nrf52840_hal::gpio::{p0, Level, Output, Pin, PushPull};
    use nrf52840_hal::usbd::UsbPeripheral;
    use rtic_monotonics::nrf::timer::prelude::*;
    use rusty_probe_nicenano::config::{APP_VTOR_OFFSET, FIRMWARE_VERSION};
    use rusty_probe_nicenano::swd::{create_dap, DapHandler, SwdPinConfig, PROBE_STATUS};
    use rusty_probe_nicenano::{usb::ProbeUsb, Mono, UsbBus};

    /// HFXO-backed clocks, kept alive for `'static` so the USB peripheral can
    /// borrow them.
    type AppClocks = Clocks<ExternalOscillator, Internal, LfOscStopped>;

    #[shared]
    struct Shared {
        probe_usb: ProbeUsb,
    }

    #[local]
    struct Local {
        led: Pin<Output<PushPull>>,
        dap: DapHandler,
    }

    #[init(local = [
        clocks: MaybeUninit<AppClocks> = MaybeUninit::uninit(),
        usb_alloc: MaybeUninit<UsbBusAllocator<UsbBus>> = MaybeUninit::uninit(),
    ])]
    fn init(cx: init::Context) -> (Shared, Local) {
        // The application is linked at 0x1000 (right after the MBR). Point the
        // vector table there so exceptions/interrupts use our table directly.
        //
        // One-time self-reset: the Adafruit UF2 bootloader jumps into the
        // application WITHOUT a full system reset, leaving the on-chip USB 3.3V
        // regulator / USBD power domain disabled (POWER.USBREGSTATUS.OUTPUTRDY
        // stays 0 even though VBUS is present), so the device can never
        // enumerate. A software reset re-arms VBUS detection and the regulator.
        // We guard with GPREGRET so the reset happens exactly once.

        rusty_probe_nicenano::perform_one_time_self_reset();

        unsafe {
            cx.core.SCB.vtor.write(APP_VTOR_OFFSET);
        }

        let dp = cx.device;
        defmt::info!("nice!nano v2 CMSIS-DAP probe port - boot");

        // Monotonic on TIMER1.
        Mono::start(dp.TIMER1);

        // USB requires the 32 MHz crystal oscillator (HFXO).
        let clocks = Clocks::new(dp.CLOCK).enable_ext_hfosc();
        let clocks: &'static AppClocks = cx.local.clocks.write(clocks);
        defmt::info!("HFXO enabled");

        // NOTE: We intentionally do NOT gate on POWER.USBREGSTATUS.OUTPUTRDY here.
        // On this board that bit can stay 0 even though VBUS is detected, which
        // would hang bring-up. The USB PHY supply is enabled automatically by
        // hardware when VBUS is present (the stock bootloader enumerates fine),
        // and `nrf-usbd`'s enable() waits on the digital `EVENTCAUSE.READY`
        // instead. This matches the nrf-hal examples.

        // Onboard LED on P0.15 (labelled "Blue" in Adafruit board.h, red on this board).
        let port0 = p0::Parts::new(dp.P0);
        let led = port0.p0_15.into_push_pull_output(Level::Low).degrade();

        // --- SWD target interface -------------------------------------------
        // Pins are taken from the free GPIO pool. SWDIO starts as a floating
        // input (target drives during turnaround); SWDCLK is a push-pull
        // output; nRESET is open-drain (Standard0Disconnect1: low = drive
        // reset, high = float so the target's pull-up releases it).
        // The `SWDIO_PIN`/`SWDCLK_PIN`/`NRESET_PIN` constants above document
        // the chosen wiring and are also used in the log line below.
        let pin_cfg = SwdPinConfig::default();
        let pins = pin_cfg.init_pins(port0.p0_20, port0.p0_17, port0.p0_22);
        let dap = create_dap(FIRMWARE_VERSION, pins.swdio, pins.swclk, pins.nreset, pin_cfg.cpu_frequency);
        defmt::info!(
            "SWD backend initialised (SWDIO=P0.{}, SWDCLK=P0.{}, nRESET=P0.{})",
            pin_cfg.swdio_pin,
            pin_cfg.swclk_pin,
            pin_cfg.nreset_pin
        );

        // Bring up the USB CMSIS-DAP device.
        let usb_periph = UsbPeripheral::new(dp.USBD, clocks);
        let usb_alloc: &'static UsbBusAllocator<UsbBus> = cx
            .local
            .usb_alloc
            .write(UsbBusAllocator::new(UsbBus::new(usb_periph)));
        let mut probe_usb = ProbeUsb::new(usb_alloc);

        // Kick-start the USB stack: the first `poll()` is what calls
        // `UsbBus::enable()` (pulls up D+). We must not rely solely on the
        // USBD interrupt to trigger it, because that interrupt only fires once
        // the bus is already enabled (host RESET) — a chicken-and-egg deadlock
        // that leaves the device un-enumerated.
        probe_usb.interrupt();

        // nrf-usbd does not enable USBD interrupts itself; do it here so the
        // `USBD`-bound task fires. We only enable events that the driver clears.
        rusty_probe_nicenano::usb::enable_usbd_interrupts();
        defmt::info!("USB device started");

        blink::spawn().ok();

        (Shared { probe_usb }, Local { led, dap })
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(local = [led], priority = 1)]
    async fn blink(cx: blink::Context) {
        let mut step: u32 = 0;
        loop {
            match PROBE_STATUS.load(Ordering::Relaxed) {
                0 => {
                    // Disconnected / Idle: short heartbeat pulse (100ms ON every 1000ms)
                    if step % 10 == 0 {
                        cx.local.led.set_high().ok();
                    } else {
                        cx.local.led.set_low().ok();
                    }
                    step = (step + 1) % 10;
                    Mono::delay(100.millis()).await;
                }
                1 => {
                    // Connected to host: solid ON
                    cx.local.led.set_high().ok();
                    Mono::delay(500.millis()).await;
                }
                2 => {
                    // Target running: fast blink (5 Hz)
                    cx.local.led.toggle().ok();
                    Mono::delay(100.millis()).await;
                }
                _ => {
                    cx.local.led.set_low().ok();
                    Mono::delay(500.millis()).await;
                }
            }
        }
    }

    /// USB interrupt: poll the stack and dispatch any CMSIS-DAP request to the
    /// SWD backend.
    #[task(binds = USBD, priority = 2, shared = [probe_usb], local = [dap, resp_buf: [u8; DAP2_PACKET_SIZE as usize] = [0; DAP2_PACKET_SIZE as usize]])]
    fn on_usb(mut cx: on_usb::Context) {
        let resp_buf = cx.local.resp_buf;
        let dap = cx.local.dap;

        cx.shared.probe_usb.lock(|probe_usb| {
            while let Some(request) = probe_usb.interrupt() {
                match request {
                    Request::DAP1Command((report, n)) => {
                        let len = dap.process_command(&report[..n], resp_buf, DapVersion::V1);
                        if len > 0 {
                            probe_usb.dap1_reply(&resp_buf[..len]);
                        }
                    }
                    Request::DAP2Command((report, n)) => {
                        let len = dap.process_command(&report[..n], resp_buf, DapVersion::V2);
                        if len > 0 {
                            probe_usb.dap2_reply(&resp_buf[..len]);
                        }
                    }
                    Request::Suspend => {
                        defmt::info!("USB suspend -> releasing SWD interface");
                        dap.suspend();
                        PROBE_STATUS.store(0, Ordering::Relaxed);
                    }
                }
            }
        });
    }
}
