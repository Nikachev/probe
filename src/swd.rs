//! SWD backend for nice!nano v2 (nRF52840), implementing the `dap_rs`
//! `Swd` / `Jtag` / `swj::Dependencies` / `swo::Swo` traits.
//!
//! nice!nano is a direct 3.3 V board (no external voltage translator), so we
//! don't need direction pins. SWDIO is a dynamic pin: it is driven as a
//! push-pull output while the host is sending, and switched to a floating
//! input while the target drives the line (the target provides the SWDIO
//! pull-up, as required by the ARM SWD spec).

#[cfg(target_arch = "arm")]
use cortex_m::asm;
#[cfg(target_arch = "arm")]
use dap_rs::{
    dap::{Dap, DapLeds, HostStatus},
    jtag, swd::{self, Ack},
    swj::{self, Dependencies},
    swo,
};
#[cfg(target_arch = "arm")]
use embedded_hal::delay::DelayNs;
#[cfg(target_arch = "arm")]
use embedded_hal::digital::{InputPin, OutputPin, StatefulOutputPin};
#[cfg(target_arch = "arm")]
use nrf52840_hal::gpio::{
    OpenDrainIO, Output, Pin, PushPull,
};

#[cfg(not(target_arch = "arm"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostStatus {
    Connected(bool),
    Running(bool),
}

#[cfg(target_arch = "arm")]
/// Default SWD clock: 5 MHz (5,000,000 Hz). The host may lower/raise this via DAP_SWJ_Clock.
const DEFAULT_MAX_FREQUENCY: u32 = 5_000_000;


/// SWD pin configuration layout for nice!nano v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwdPinConfig {
    pub swdio_pin: u8,
    pub swclk_pin: u8,
    pub nreset_pin: u8,
    pub cpu_frequency: u32,
}

#[cfg(target_arch = "arm")]
use nrf52840_hal::gpio::{p0, Level, OpenDrainConfig};

#[cfg(target_arch = "arm")]
pub struct SwdPins {
    pub swdio: Pin<Output<PushPull>>,
    pub swclk: Pin<Output<PushPull>>,
    pub nreset: Pin<Output<OpenDrainIO>>,
}

impl Default for SwdPinConfig {
    fn default() -> Self {
        Self {
            swdio_pin: 20,
            swclk_pin: 17,
            nreset_pin: 22,
            cpu_frequency: 64_000_000,
        }
    }
}

#[cfg(target_arch = "arm")]
use nrf52840_hal::gpio::Disconnected;

#[cfg(target_arch = "arm")]
impl SwdPinConfig {
    pub fn init_pins(
        &self,
        swdio: p0::P0_20<Disconnected>,
        swclk: p0::P0_17<Disconnected>,
        nreset: p0::P0_22<Disconnected>,
    ) -> SwdPins {
        let swdio = swdio.into_push_pull_output(Level::High).degrade();
        let swclk = swclk.into_push_pull_output(Level::High).degrade();
        let nreset = nreset
            .into_open_drain_input_output(OpenDrainConfig::Standard0Disconnect1, Level::High)
            .degrade();
        SwdPins { swdio, swclk, nreset }
    }
}

#[cfg(target_arch = "arm")]
#[inline(always)]
fn p0_reg() -> &'static nrf52840_hal::pac::p0::RegisterBlock {
    unsafe { &*nrf52840_hal::pac::P0::ptr() }
}

#[cfg(target_arch = "arm")]
pub struct SwdioPin {
    mask: u32,
}

#[cfg(target_arch = "arm")]
impl SwdioPin {
    pub fn new(pin: Pin<Output<PushPull>>) -> Self {
        let pin_num = pin.pin();
        let mask = 1 << pin_num;
        p0_reg().pin_cnf[pin_num as usize].write(|w| {
            w.dir().output()
             .input().connect()
             .pull().pullup()
             .drive().h0h1()
        });
        Self { mask }
    }

    #[inline(always)]
    fn set_input(&mut self) {
        p0_reg().dirclr.write(|w| unsafe { w.bits(self.mask) });
    }

    #[inline(always)]
    fn set_output(&mut self) {
        p0_reg().dirset.write(|w| unsafe { w.bits(self.mask) });
    }

    #[inline(always)]
    fn set_high(&mut self) {
        p0_reg().outset.write(|w| unsafe { w.bits(self.mask) });
    }

    #[inline(always)]
    fn set_low(&mut self) {
        p0_reg().outclr.write(|w| unsafe { w.bits(self.mask) });
    }

    #[inline(always)]
    fn is_high(&self) -> bool {
        (p0_reg().in_.read().bits() & self.mask) != 0
    }
}

#[cfg(target_arch = "arm")]
/// Context holding the SWD pins and timing information.
pub struct Context {
    swdio: SwdioPin,
    swclk: Pin<Output<PushPull>>,
    swclk_mask: u32,
    nreset: Pin<Output<OpenDrainIO>>,
    cpu_frequency: u32,
    max_frequency: u32,
    half_period_ticks: u32,
}

/// Calculate half-period delay iterations for `cortex_m::asm::delay` based on CPU and target clock frequencies.
/// Note: On Cortex-M4, `asm::delay(n)` runs a 3-cycle loop (`subs; bne`), taking `3 * n` cycles.
pub fn calculate_half_period_ticks(cpu_frequency: u32, max_frequency: u32) -> u32 {
    if max_frequency == 0 {
        return 1;
    }
    let half_cycles = cpu_frequency / max_frequency / 2;
    (half_cycles / 3).max(1)
}

#[cfg(target_arch = "arm")]
impl Context {

    fn with_frequency(
        swdio: SwdioPin,
        swclk: Pin<Output<PushPull>>,
        nreset: Pin<Output<OpenDrainIO>>,
        cpu_frequency: u32,
        max_frequency: u32,
    ) -> Self {
        let pin_num = swclk.pin();
        p0_reg().pin_cnf[pin_num as usize].write(|w| {
            w.dir().output()
             .input().connect()
             .pull().disabled()
             .drive().h0h1()
        });

        let swclk_mask = 1 << pin_num;
        let half_period_ticks = calculate_half_period_ticks(cpu_frequency, max_frequency);
        Context {
            swdio,
            swclk,
            swclk_mask,
            nreset,
            cpu_frequency,
            max_frequency,
            half_period_ticks,
        }
    }

    #[inline(always)]
    fn set_swclk_high(&mut self) {
        p0_reg().outset.write(|w| unsafe { w.bits(self.swclk_mask) });
    }

    #[inline(always)]
    fn set_swclk_low(&mut self) {
        p0_reg().outclr.write(|w| unsafe { w.bits(self.swclk_mask) });
    }

    fn swdio_to_input(&mut self) {
        self.swdio.set_input();
    }

    fn swdio_to_output(&mut self) {
        self.swdio.set_output();
    }

    fn nreset_release(&mut self) {
        self.nreset.set_high().ok();
    }

    fn nreset_assert(&mut self) {
        self.nreset.set_low().ok();
    }
}

#[cfg(target_arch = "arm")]
impl swj::Dependencies<Swd, Jtag> for Context {

    fn process_swj_pins(&mut self, output: swj::Pins, mask: swj::Pins, wait_us: u32) -> swj::Pins {
        if mask.contains(swj::Pins::SWCLK) {
            if output.contains(swj::Pins::SWCLK) {
                self.swclk.set_high().ok();
            } else {
                self.swclk.set_low().ok();
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
        ret.set(swj::Pins::SWCLK, self.swclk.is_set_high().unwrap_or(false));
        self.swdio_to_input();
        ret.set(swj::Pins::SWDIO, self.swdio.is_high());
        ret.set(swj::Pins::NRESET, self.nreset.is_high().unwrap_or(false));

        ret
    }

    fn process_swj_sequence(&mut self, data: &[u8], mut bits: usize) {
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
                self.swclk.set_low().ok();
                asm::delay(hp);
                self.swclk.set_high().ok();
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
        self.half_period_ticks = calculate_half_period_ticks(self.cpu_frequency, max_frequency);
        true
    }

    fn high_impedance_mode(&mut self) {
        // Release SWDIO and nRESET, leave SWCLK low.
        self.swdio_to_input();
        self.swclk.set_low().ok();
        self.nreset_release();
    }
}

#[cfg(target_arch = "arm")]
/// JTAG backend. We only support SWD, so JTAG is marked unavailable and its
/// handlers are no-ops.
pub struct Jtag(Context);

#[cfg(target_arch = "arm")]
impl From<Jtag> for Context {
    fn from(value: Jtag) -> Self {
        value.0
    }
}

#[cfg(target_arch = "arm")]
impl From<Context> for Jtag {
    fn from(value: Context) -> Self {
        Self(value)
    }
}

#[cfg(target_arch = "arm")]
impl jtag::Jtag<Context> for Jtag {
    const AVAILABLE: bool = false;

    fn sequences(&mut self, _data: &[u8], _rxbuf: &mut [u8]) -> u32 {
        0
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.0.process_swj_clock(max_frequency)
    }
}

#[cfg(target_arch = "arm")]
/// SWD backend.
pub struct Swd(Context);

#[cfg(target_arch = "arm")]
impl From<Swd> for Context {
    fn from(value: Swd) -> Self {
        value.0
    }
}

#[cfg(target_arch = "arm")]
impl From<Context> for Swd {
    fn from(mut value: Context) -> Self {
        // Put the interface into a known state: SWDIO driven.
        value.swdio_to_output();
        Self(value)
    }
}

#[cfg(target_arch = "arm")]
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

        // Trailing: drive SWDIO low to avoid floating (first bit serves as turnaround).
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
            let mut byte_val = *b;
            for _ in 0..bit_count {
                self.write_bit(byte_val & 1);
                byte_val >>= 1;
            }
            num_bits -= bit_count;
        }
        Ok(())
    }

    fn read_sequence(&mut self, mut num_bits: usize, data: &mut [u8]) -> swd::Result<()> {
        self.0.swdio_to_input();
        for b in data.iter_mut() {
            let bit_count = core::cmp::min(num_bits, 8);
            let mut byte_val = 0u8;
            for i in 0..bit_count {
                byte_val |= self.read_bit() << i;
            }
            *b = byte_val;
            num_bits -= bit_count;
        }
        Ok(())
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.0.process_swj_clock(max_frequency)
    }
}

#[cfg(target_arch = "arm")]
impl Swd {
    #[inline(always)]
    fn tx8(&mut self, mut data: u8) {
        self.0.swdio_to_output();
        for _ in 0..8 {
            self.write_bit(data & 1);
            data >>= 1;
        }
    }

    #[inline(always)]
    fn rx4(&mut self) -> u8 {
        self.0.swdio_to_input();
        let mut data = 0;
        for i in 0..4 {
            data |= (self.read_bit() & 1) << i;
        }
        data
    }

    #[inline(always)]
    fn rx5(&mut self) -> u8 {
        self.0.swdio_to_input();
        let mut data = 0;
        for i in 0..5 {
            data |= (self.read_bit() & 1) << i;
        }
        data
    }

    #[inline(always)]
    fn send_data(&mut self, mut data: u32, parity: bool) {
        self.0.swdio_to_output();
        for _ in 0..32 {
            self.write_bit((data & 1) as u8);
            data >>= 1;
        }
        self.write_bit(parity as u8);
    }

    #[inline(always)]
    fn read_data(&mut self) -> (u32, bool) {
        self.0.swdio_to_input();
        let mut data = 0u32;
        for i in 0..32 {
            data |= (self.read_bit() as u32) << i;
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
        self.0.set_swclk_low();
        asm::delay(hp);
        self.0.set_swclk_high();
        asm::delay(hp);
    }

    #[inline(always)]
    fn read_bit(&mut self) -> u8 {
        let hp = self.0.half_period_ticks;
        self.0.set_swclk_low();
        asm::delay(hp);
        let bit = self.0.swdio.is_high() as u8;
        self.0.set_swclk_high();
        asm::delay(hp);
        bit
    }
}

/// Dummy SWO backend (trace not supported on nice!nano v2).
#[cfg(target_arch = "arm")]
pub struct Swo {}

#[cfg(target_arch = "arm")]
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

impl Leds {
    pub fn react_to_host_status(&mut self, status: HostStatus) {
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

#[cfg(target_arch = "arm")]
impl DapLeds for Leds {
    fn react_to_host_status(&mut self, status: HostStatus) {
        Leds::react_to_host_status(self, status);
    }
}


#[cfg(target_arch = "arm")]
/// Delay provider for the DAP processor (used by e.g. DAP_Delay).
pub struct Wait {
    cpu_frequency: u32,
}

#[cfg(target_arch = "arm")]
impl DelayNs for Wait {
    fn delay_ns(&mut self, ns: u32) {
        let us = (ns / 1_000).max(1);
        asm::delay(us * (self.cpu_frequency / 1_000_000));
    }
}

#[cfg(target_arch = "arm")]
/// Concrete DAP handler type for nice!nano v2.
pub type DapHandler = Dap<'static, Context, Leds, Wait, Jtag, Swd, Swo>;

#[cfg(target_arch = "arm")]
/// Build a DAP handler from the SWD pins.
///
/// * `swdio`  - bidirectional data line (start as pullup input)
/// * `swclk`  - clock line (push-pull output)
/// * `nreset` - target reset line (open-drain output)
pub fn create_dap(
    version_string: &'static str,
    swdio: Pin<Output<PushPull>>,
    swclk: Pin<Output<PushPull>>,
    nreset: Pin<Output<OpenDrainIO>>,
    cpu_frequency: u32,
) -> DapHandler {
    pulse_target_nreset();
    let swdio = SwdioPin::new(swdio);
    let context = Context::with_frequency(swdio, swclk, nreset, cpu_frequency, DEFAULT_MAX_FREQUENCY);
    let wait = Wait { cpu_frequency };
    let swo = None;
    Dap::new(context, Leds, wait, swo, version_string)
}


/// Calculate even parity for a 32-bit SWD data payload.
pub fn swd_parity(data: u32) -> bool {
    data.count_ones() % 2 != 0
}

/// Assert a 10 ms hardware reset pulse on the target nRESET line.
pub fn pulse_target_nreset() {
    #[cfg(target_arch = "arm")]
    {
        let pin_mask = 1 << SwdPinConfig::default().nreset_pin;
        p0_reg().outclr.write(|w| unsafe { w.bits(pin_mask) });
        cortex_m::asm::delay(640_000);
        p0_reg().outset.write(|w| unsafe { w.bits(pin_mask) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_half_period_ticks() {
        let cpu_freq = 64_000_000;
        assert_eq!(calculate_half_period_ticks(cpu_freq, 5_000_000), 2);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 2_000_000), 5);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 1_000_000), 10);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 500_000), 21);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 100_000), 106);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 50_000), 213);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 0), 1);
        assert_eq!(calculate_half_period_ticks(cpu_freq, 100_000_000), 1);
        // Test edge cases with odd CPU frequencies
        assert_eq!(calculate_half_period_ticks(16_000_000, 1_000_000), 2);
    }

    #[test]
    fn test_swd_parity() {
        assert_eq!(swd_parity(0x0000_0000), false);
        assert_eq!(swd_parity(0x0000_0001), true);
        assert_eq!(swd_parity(0x0000_0003), false);
        assert_eq!(swd_parity(0x8000_0000), true);
        assert_eq!(swd_parity(0xFFFF_FFFF), false);
        assert_eq!(swd_parity(0xDEAD_BEEF), (0xDEAD_BEEFu32.count_ones() % 2 != 0));
        assert_eq!(swd_parity(0x5555_5555), false); // 16 set bits -> even -> false
        assert_eq!(swd_parity(0x5555_5557), true);  // 17 set bits -> odd -> true
    }

    #[test]
    fn test_leds_host_status_transitions() {
        let mut leds = Leds;
        leds.react_to_host_status(HostStatus::Connected(true));
        assert_eq!(PROBE_STATUS.load(Ordering::Relaxed), 1);

        leds.react_to_host_status(HostStatus::Running(true));
        assert_eq!(PROBE_STATUS.load(Ordering::Relaxed), 2);

        leds.react_to_host_status(HostStatus::Running(false));
        assert_eq!(PROBE_STATUS.load(Ordering::Relaxed), 1);

        leds.react_to_host_status(HostStatus::Connected(false));
        assert_eq!(PROBE_STATUS.load(Ordering::Relaxed), 0);
    }
}




