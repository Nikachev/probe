#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(not(target_arch = "arm"))]
fn main() {}


// Standalone diagnostic (no RTIC, no USB). Uses the onboard LED (P0.15) to
// report how far bring-up gets, to locate where app.rs hangs.
//
// LED sequence (each "blink" = 150ms on / 150ms off):
//   1 blink,  pause  -> reached after LED init (start)
//   2 blinks, pause  -> HFXO (external 32 MHz crystal) started OK
//   then a REPEATING status pattern based on the USB regulator:
//     * 4 FAST blinks, repeating          -> OUTPUTRDY ready (USB power OK)
//     * slow 1s-on / 1s-off heartbeat     -> OUTPUTRDY NOT ready (VBUS/regulator issue)
//
// Interpretation:
//   no blinks at all            -> hang before/at LED init (unexpected)
//   1 blink then dark           -> hang while starting HFXO
//   2 blinks then fast pattern  -> HFXO + USB regulator OK (problem is USB stack)
//   2 blinks then slow heartbeat-> HFXO OK but USB regulator not ready

use cortex_m_rt::entry;
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use nrf52840_hal as hal;
use panic_probe as _;

use hal::gpio::{p0::P0_15, Output, PushPull};

const CYCLES_PER_MS: u32 = 64_000; // CPU runs at 64 MHz

fn delay_ms(ms: u32) {
    cortex_m::asm::delay(CYCLES_PER_MS * ms);
}

type Led = P0_15<Output<PushPull>>;

fn blink(led: &mut Led, n: u32) {
    for _ in 0..n {
        led.set_high().ok();
        delay_ms(150);
        led.set_low().ok();
        delay_ms(150);
    }
}

#[entry]
fn main() -> ! {
    // Application is linked at 0x26000 (after the S140 SoftDevice).
    unsafe {
        let cp = cortex_m::Peripherals::steal();
        cp.SCB.vtor.write(0x0002_6000);
    }

    let dp = hal::pac::Peripherals::take().unwrap();

    // LED on P0.15, active-high.
    let port0 = hal::gpio::p0::Parts::new(dp.P0);
    let mut led = port0.p0_15.into_push_pull_output(hal::gpio::Level::Low);

    // Marker 1: reached start.
    blink(&mut led, 1);
    delay_ms(800);

    // Start HFXO (first suspect: blocks until the 32 MHz crystal is stable).
    let _clocks = hal::clocks::Clocks::new(dp.CLOCK).enable_ext_hfosc();

    // Marker 2: HFXO OK.
    blink(&mut led, 2);
    delay_ms(800);

    // Report USB power status forever, encoded as blink counts per cycle:
    //   2 blinks -> VBUSDETECT=1, OUTPUTRDY=1  (all good)
    //   3 blinks -> VBUSDETECT=1, OUTPUTRDY=0  (VBUS seen, regulator not ready)
    //   4 blinks -> VBUSDETECT=0, OUTPUTRDY=1  (unexpected)
    //   5 blinks -> VBUSDETECT=0, OUTPUTRDY=0  (no VBUS seen at all)
    loop {
        let st = dp.POWER.usbregstatus.read();
        let vbus = st.vbusdetect().bit_is_set();
        let rdy = st.outputrdy().is_ready();
        let n = match (vbus, rdy) {
            (true, true) => 2,
            (true, false) => 3,
            (false, true) => 4,
            (false, false) => 5,
        };
        blink(&mut led, n);
        delay_ms(1500);
    }
}
