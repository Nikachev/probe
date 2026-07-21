//! SWD backend for nice!nano v2 (nRF52840), implementing the `dap_rs`
//! `Swd` / `Jtag` / `swj::Dependencies` / `swo::Swo` traits.
//!
//! nice!nano is a direct 3.3 V board (no external voltage translator), so we
//! don't need direction pins. SWDIO is a dynamic pin: it is driven as a
//! push-pull output while the host is sending, and switched to a floating
//! input while the target drives the line (the target provides the SWDIO
//! pull-up, as required by the ARM SWD spec).

use cortex_m::asm;
use dap_rs::{
    dap::{Dap, DapLeds, HostStatus},
    jtag, swd::{self, Ack},
    swj::{self, Dependencies},
    swo,
};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin, StatefulOutputPin};
use nrf52840_hal::gpio::{
    Floating, Input, Level, OpenDrainIO, Output, Pin, PushPull,
};

/// Default SWD clock: 1 MHz. The host may lower/raise this via DAP_SWJ_Clock.
const DEFAULT_MAX_FREQUENCY: u32 = 1_000_000;

/// Dynamic SWDIO pin: input while the target drives, output while we drive.
///
/// `Invalid` is used only as a transient slot during the in-place
/// `core::mem::replace` that swaps the pin between the two modes (nrf-hal has
/// no dummy-pin constructor).
enum SwdioPin {
    Input(Pin<Input<Floating>>),
    Output(Pin<Output<PushPull>>),
    Invalid,
}

impl SwdioPin {
    fn set_input(&mut self) {
        if matches!(self, SwdioPin::Input(_)) {
            return;
        }
        let prev = core::mem::replace(self, SwdioPin::Invalid);
        let next = match prev {
            SwdioPin::Output(o) => SwdioPin::Input(o.into_floating_input()),
            SwdioPin::Input(i) => SwdioPin::Input(i),
            SwdioPin::Invalid => unreachable!(),
        };
        *self = next;
    }

    fn set_output(&mut self) {
        if matches!(self, SwdioPin::Output(_)) {
            return;
        }
        let prev = core::mem::replace(self, SwdioPin::Invalid);
        let next = match prev {
            SwdioPin::Input(i) => SwdioPin::Output(i.into_push_pull_output(Level::High)),
            SwdioPin::Output(o) => SwdioPin::Output(o),
            SwdioPin::Invalid => unreachable!(),
        };
        *self = next;
    }

    fn set_high(&mut self) {
        if let SwdioPin::Output(o) = self {
            o.set_high().ok();
        }
    }

    fn set_low(&mut self) {
        if let SwdioPin::Output(o) = self {
            o.set_low().ok();
        }
    }

    fn is_high(&mut self) -> bool {
        match self {
            SwdioPin::Input(i) => i.is_high().unwrap_or(false),
            SwdioPin::Output(o) => o.is_set_high().unwrap_or(false),
            SwdioPin::Invalid => false,
        }
    }
}

/// Context holding the SWD pins and timing information.
pub struct Context {
    swdio: SwdioPin,
    swclk: Pin<Output<PushPull>>,
    nreset: Pin<Output<OpenDrainIO>>,
    cpu_frequency: u32,
    max_frequency: u32,
    half_period_ticks: u32,
}

impl Context {
    fn with_frequency(
        swdio: SwdioPin,
        swclk: Pin<Output<PushPull>>,
        nreset: Pin<Output<OpenDrainIO>>,
        cpu_frequency: u32,
        max_frequency: u32,
    ) -> Self {
        let half_period_ticks = (cpu_frequency / max_frequency / 2).max(1);
        Context {
            swdio,
            swclk,
            nreset,
            cpu_frequency,
            max_frequency,
            half_period_ticks,
        }
    }

    fn swdio_to_input(&mut self) {
        self.swdio.set_input();
    }

    fn swdio_to_output(&mut self) {
        self.swdio.set_output();
    }

    fn swclk_to_input(&mut self) {
        // SWCLK is always an output on nice!nano.
    }

    fn swclk_to_output(&mut self) {
        // SWCLK is always an output on nice!nano.
    }

    fn nreset_release(&mut self) {
        self.nreset.set_high().ok();
    }

    fn nreset_assert(&mut self) {
        self.nreset.set_low().ok();
    }
}

impl swj::Dependencies<Swd, Jtag> for Context {
    fn process_swj_pins(&mut self, output: swj::Pins, mask: swj::Pins, wait_us: u32) -> swj::Pins {
        if mask.contains(swj::Pins::SWCLK) {
            self.swclk_to_output();
            if output.contains(swj::Pins::SWCLK) {
                self.swclk.set_high();
            } else {
                self.swclk.set_low();
            }
        }

        if mask.contains(swj::Pins::SWDIO) {
            self.swdio_to_output();
            if output.contains(swj::Pins::SWDIO) {
                self.swdio.set_high();
            } else {
                self.swdio.set_low();
            }
        }

        if mask.contains(swj::Pins::NRESET) {
            if output.contains(swj::Pins::NRESET) {
                // "open drain disconnect" -> release (target's pull-up wins)
                self.nreset_release();
            } else {
                self.nreset_assert();
            }
        }

        // Busy-wait up to wait_us, then sample the pin states.
        let delay_ticks = wait_us.saturating_mul(self.cpu_frequency / 1_000_000);
        asm::delay(delay_ticks);

        let mut ret = swj::Pins::empty();
        self.swclk_to_input();
        ret.set(swj::Pins::SWCLK, self.swclk.is_set_high().unwrap_or(false));
        self.swdio_to_input();
        ret.set(swj::Pins::SWDIO, self.swdio.is_high());
        self.nreset_release();
        ret.set(swj::Pins::NRESET, self.nreset.is_high().unwrap_or(false));

        ret
    }

    fn process_swj_sequence(&mut self, data: &[u8], mut bits: usize) {
        self.swclk_to_output();
        self.swdio_to_output();

        let hp = self.half_period_ticks;
        for byte in data {
            let frame_bits = core::cmp::min(bits, 8);
            for i in 0..frame_bits {
                let bit = (byte >> i) & 1;
                if bit != 0 {
                    self.swdio.set_high();
                } else {
                    self.swdio.set_low();
                }
                self.swclk.set_low();
                asm::delay(hp);
                self.swclk.set_high();
                asm::delay(hp);
            }
            bits -= frame_bits;
        }
    }

    fn process_swj_clock(&mut self, max_frequency: u32) -> bool {
        if max_frequency == 0 || max_frequency >= self.cpu_frequency {
            return false;
        }
        // Cap to something we can realistically meet on a 64 MHz core.
        let max_frequency = max_frequency.min(5_000_000);
        self.max_frequency = max_frequency;
        self.half_period_ticks = (self.cpu_frequency / max_frequency / 2).max(1);
        true
    }

    fn high_impedance_mode(&mut self) {
        // Release SWDIO and nRESET, leave SWCLK low.
        self.swdio_to_input();
        self.swclk.set_low().ok();
        self.nreset_release();
    }
}

/// JTAG backend. We only support SWD, so JTAG is marked unavailable and its
/// handlers are no-ops.
pub struct Jtag(Context);

impl From<Jtag> for Context {
    fn from(value: Jtag) -> Self {
        value.0
    }
}

impl From<Context> for Jtag {
    fn from(value: Context) -> Self {
        Self(value)
    }
}

impl jtag::Jtag<Context> for Jtag {
    const AVAILABLE: bool = false;

    fn sequences(&mut self, _data: &[u8], _rxbuf: &mut [u8]) -> u32 {
        0
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.0.process_swj_clock(max_frequency)
    }
}

/// SWD backend.
pub struct Swd(Context);

impl From<Swd> for Context {
    fn from(value: Swd) -> Self {
        value.0
    }
}

impl From<Context> for Swd {
    fn from(mut value: Context) -> Self {
        // Put the interface into a known state: SWDIO/SWCLK driven, nRESET released.
        value.swdio_to_output();
        value.swclk_to_output();
        value.nreset_release();
        Self(value)
    }
}

impl swd::Swd<Context> for Swd {
    const AVAILABLE: bool = true;

    fn read_inner(&mut self, apndp: swd::APnDP, a: swd::DPRegister) -> swd::Result<u32> {
        // Send request
        let req = swd::make_request(apndp, swd::RnW::R, a);
        self.tx8(req);

        // Read ack: 1 turnaround clock + 3 ack bits.
        let ack = self.rx4() >> 1;

        match Ack::try_ok(ack) {
            Ok(_) => {}
            Err(e) => {
                // On non-OK ACK the target has released the bus but still
                // expects a turnaround clock before the next request.
                self.tx8(0);
                return Err(e);
            }
        }

        // Read data and parity.
        let (data, parity) = self.read_data();

        // Turnaround + trailing: read one bit, then drive SWDIO low to avoid floating.
        self.read_bit();
        self.tx8(0);

        if parity as u8 == (data.count_ones() as u8 & 1) {
            Ok(data)
        } else {
            Err(swd::Error::BadParity)
        }
    }

    fn write_inner(&mut self, apndp: swd::APnDP, a: swd::DPRegister, data: u32) -> swd::Result<()> {
        // Send request
        let req = swd::make_request(apndp, swd::RnW::W, a);
        self.tx8(req);

        // Read ack: 1 turnaround clock + 3 ack bits + 1 turnaround clock.
        let ack = (self.rx5() >> 1) & 0b111;
        match Ack::try_ok(ack) {
            Ok(_) => {}
            Err(e) => {
                self.tx8(0);
                return Err(e);
            }
        }

        // Send data and parity
        let parity = data.count_ones() & 1 == 1;
        self.send_data(data, parity);

        // Trailing idle
        self.tx8(0);

        Ok(())
    }

    fn write_sequence(&mut self, mut num_bits: usize, data: &[u8]) -> swd::Result<()> {
        self.0.swdio_to_output();
        for b in data {
            let bit_count = core::cmp::min(num_bits, 8);
            for i in 0..bit_count {
                self.write_bit((b >> i) & 0x1);
            }
            num_bits -= bit_count;
        }
        Ok(())
    }

    fn read_sequence(&mut self, mut num_bits: usize, data: &mut [u8]) -> swd::Result<()> {
        self.0.swdio_to_input();
        for b in data.iter_mut() {
            let bit_count = core::cmp::min(num_bits, 8);
            for i in 0..bit_count {
                *b |= self.read_bit() << i;
            }
            num_bits -= bit_count;
        }
        Ok(())
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.0.process_swj_clock(max_frequency)
    }
}

impl Swd {
    fn tx8(&mut self, mut data: u8) {
        self.0.swdio_to_output();
        for _ in 0..8 {
            self.write_bit(data & 1);
            data >>= 1;
        }
    }

    fn rx4(&mut self) -> u8 {
        self.0.swdio_to_input();
        let mut data = 0;
        for i in 0..4 {
            data |= (self.read_bit() & 1) << i;
        }
        data
    }

    fn rx5(&mut self) -> u8 {
        self.0.swdio_to_input();
        let mut data = 0;
        for i in 0..5 {
            data |= (self.read_bit() & 1) << i;
        }
        data
    }

    fn send_data(&mut self, mut data: u32, parity: bool) {
        self.0.swdio_to_output();
        for _ in 0..32 {
            self.write_bit((data & 1) as u8);
            data >>= 1;
        }
        self.write_bit(parity as u8);
    }

    fn read_data(&mut self) -> (u32, bool) {
        self.0.swdio_to_input();
        let mut data = 0;
        for i in 0..32 {
            data |= (self.read_bit() as u32 & 1) << i;
        }
        let parity = self.read_bit() != 0;
        (data, parity)
    }

    #[inline(always)]
    fn write_bit(&mut self, bit: u8) {
        if bit != 0 {
            self.0.swdio.set_high();
        } else {
            self.0.swdio.set_low();
        }
        let hp = self.0.half_period_ticks;
        self.0.swclk.set_low();
        asm::delay(hp);
        self.0.swclk.set_high();
        asm::delay(hp);
    }

    #[inline(always)]
    fn read_bit(&mut self) -> u8 {
        let hp = self.0.half_period_ticks;
        self.0.swclk.set_low();
        asm::delay(hp);
        let bit = self.0.swdio.is_high() as u8;
        self.0.swclk.set_high();
        asm::delay(hp);
        bit
    }
}

/// Dummy SWO backend (trace not supported on nice!nano v2).
pub struct Swo {}

impl swo::Swo for Swo {
    fn set_transport(&mut self, _transport: swo::SwoTransport) {}
    fn set_mode(&mut self, _mode: swo::SwoMode) {}
    fn set_baudrate(&mut self, _baudrate: u32) -> u32 {
        0
    }
    fn set_control(&mut self, _control: swo::SwoControl) {}
    fn polling_data(&mut self, _buf: &mut [u8]) -> u32 {
        0
    }
    fn streaming_data(&mut self) {}
    fn is_active(&self) -> bool {
        false
    }
    fn bytes_available(&self) -> u32 {
        0
    }
    fn buffer_size(&self) -> u32 {
        0
    }
    fn support(&self) -> swo::SwoSupport {
        swo::SwoSupport {
            uart: false,
            manchester: false,
        }
    }
    fn status(&mut self) -> swo::SwoStatus {
        swo::SwoStatus {
            active: false,
            trace_error: false,
            trace_overrun: false,
            bytes_available: 0,
        }
    }
}

use core::sync::atomic::{AtomicU8, Ordering};

/// Global probe status indicator updated by CMSIS-DAP host status commands:
/// * `0`: Disconnected / Idle (short heartbeat pulse)
/// * `1`: Connected to host (solid ON)
/// * `2`: Target running (fast blink)
pub static PROBE_STATUS: AtomicU8 = AtomicU8::new(0);

/// LED status hook for DAP host-status commands.
pub struct Leds;

impl DapLeds for Leds {
    fn react_to_host_status(&mut self, status: HostStatus) {
        match status {
            HostStatus::Connected(connected) => {
                if connected {
                    PROBE_STATUS.store(1, Ordering::Relaxed);
                } else {
                    PROBE_STATUS.store(0, Ordering::Relaxed);
                }
            }
            HostStatus::Running(running) => {
                if running {
                    PROBE_STATUS.store(2, Ordering::Relaxed);
                } else {
                    PROBE_STATUS.store(1, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Delay provider for the DAP processor (used by e.g. DAP_Delay).
pub struct Wait {
    cpu_frequency: u32,
}

impl DelayNs for Wait {
    fn delay_ns(&mut self, ns: u32) {
        let us = (ns / 1_000).max(1);
        asm::delay(us * (self.cpu_frequency / 1_000_000));
    }
}

/// Concrete DAP handler type for nice!nano v2.
pub type DapHandler = Dap<'static, Context, Leds, Wait, Jtag, Swd, Swo>;

/// Build a DAP handler from the SWD pins.
///
/// * `swdio`  - bidirectional data line (start as floating input)
/// * `swclk`  - clock line (push-pull output)
/// * `nreset` - target reset line (open-drain output)
pub fn create_dap(
    version_string: &'static str,
    swdio: Pin<Input<Floating>>,
    swclk: Pin<Output<PushPull>>,
    nreset: Pin<Output<OpenDrainIO>>,
    cpu_frequency: u32,
) -> DapHandler {
    let swdio = SwdioPin::Input(swdio);
    let context = Context::with_frequency(swdio, swclk, nreset, cpu_frequency, DEFAULT_MAX_FREQUENCY);
    let wait = Wait { cpu_frequency };
    let swo = None;
    Dap::new(context, Leds, wait, swo, version_string)
}
