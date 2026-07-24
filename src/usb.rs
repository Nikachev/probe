//! CMSIS-DAP USB device (v1 HID + v2 bulk + WinUSB + CDC serial).
//!
//! Ported from the original RP2040 firmware; only the `UsbBus` backend changed
//! (now the nRF52840 USBD peripheral via `nrf-usbd`).

use crate::UsbBus;
use dap_rs::usb::{dap_v1::CmsisDapV1, dap_v2::CmsisDapV2, winusb::MicrosoftDescriptors, Request};
use dap_rs::usb_device::{class_prelude::*, prelude::*};
use defmt::*;
use usbd_serial::SerialPort;

/// Implements the CMSIS-DAP descriptors and USB polling.
pub struct ProbeUsb {
    device: UsbDevice<'static, UsbBus>,
    device_state: UsbDeviceState,
    winusb: MicrosoftDescriptors,
    dap_v1: CmsisDapV1<'static, UsbBus>,
    dap_v2: CmsisDapV2<'static, UsbBus>,
    serial: SerialPort<'static, UsbBus>,
}

impl ProbeUsb {
    #[inline(always)]
    pub fn new(usb_bus: &'static UsbBusAllocator<UsbBus>) -> Self {
        let winusb = MicrosoftDescriptors;

        let dap_v1 = CmsisDapV1::new(64, usb_bus);
        let dap_v2 = CmsisDapV2::new(64, usb_bus);
        let serial = SerialPort::new(usb_bus);

        let id = crate::device_signature::device_id_hex();
        info!("Device ID: {}", id);

        let descriptors_en = StringDescriptors::new(LangID::EN)
            .manufacturer(crate::config::USB_MANUFACTURER)
            .product(crate::config::USB_PRODUCT)
            .serial_number(id);

        let descriptors_en_us = StringDescriptors::new(LangID::EN_US)
            .manufacturer(crate::config::USB_MANUFACTURER)
            .product(crate::config::USB_PRODUCT)
            .serial_number(id);

        let device = UsbDeviceBuilder::new(usb_bus, UsbVidPid(crate::config::USB_VID, crate::config::USB_PID))
            .strings(&[descriptors_en, descriptors_en_us])
            .expect("Failed to set USB string descriptors")
            .device_class(0)
            .max_packet_size_0(64)
            .expect("Failed to set USB max_packet_size_0 to 64")
            .max_power(500)
            .expect("Failed to set USB max_power to 500 mA")
            .build();

        let device_state = device.state();

        ProbeUsb {
            device,
            device_state,
            winusb,
            dap_v1,
            dap_v2,
            serial,
        }
    }

    /// Poll the USB stack. Returns a pending CMSIS-DAP request, if any.
    pub fn interrupt(&mut self) -> Option<Request> {
        if self.device.poll(&mut [
            &mut self.winusb,
            &mut self.dap_v1,
            &mut self.dap_v2,
            &mut self.serial,
        ]) {
            let old_state = self.device_state;
            let new_state = self.device.state();
            self.device_state = new_state;

            if (old_state != new_state) && (new_state != UsbDeviceState::Configured) {
                return Some(Request::Suspend);
            }

            // Check for 1200 baud touch reboot (Adafruit / Nordic DFU trigger standard).
            if self.serial.line_coding().data_rate() == 1200 {
                defmt::info!("1200 baud touch detected -> resetting to UF2 bootloader");
                crate::reset_to_bootloader();
            }

            self.check_cdc_commands();

            let r = self.dap_v1.process();
            if r.is_some() {
                return r;
            }

            let r = self.dap_v2.process();
            if r.is_some() {
                return r;
            }
        }
        None
    }

    #[inline(always)]
    fn check_cdc_commands(&mut self) {
        let mut buf = [0u8; 64];
        if let Ok(count) = self.serial.read(&mut buf) {
            if (3..=32).contains(&count) {
                if let Ok(s) = core::str::from_utf8(&buf[..count]) {
                    let cmd = s.trim();
                    if cmd.eq_ignore_ascii_case("dfu")
                        || cmd.eq_ignore_ascii_case("bootloader")
                        || cmd.eq_ignore_ascii_case("reset")
                        || cmd.eq_ignore_ascii_case("reboot")
                        || cmd.eq_ignore_ascii_case("boot")
                    {
                        crate::swd::PROBE_STATUS.store(2, core::sync::atomic::Ordering::Relaxed);
                        defmt::info!("CDC DFU command received ('{}') -> resetting to UF2 bootloader", cmd);
                        let _ = self.serial.write(b"Resetting to UF2 bootloader...\r\n");
                        crate::reset_to_bootloader();
                    } else if cmd.eq_ignore_ascii_case("reset_target")
                        || cmd.eq_ignore_ascii_case("target_reset")
                        || cmd.eq_ignore_ascii_case("target-reset")
                    {
                        crate::swd::PROBE_STATUS.store(2, core::sync::atomic::Ordering::Relaxed);
                        defmt::info!("CDC target reset command received ('{}') -> pulsing nRESET", cmd);
                        let _ = self.serial.write(b"Pulsing nRESET on target MCU...\r\n");
                        crate::swd::pulse_target_nreset();
                    }
                }
            }
        }
    }

    /// Transmit a DAP report back over the DAPv1 HID interface.
    pub fn dap1_reply(&mut self, data: &[u8]) {
        let _ = self.dap_v1.write_packet(data);
    }

    /// Transmit a DAP report back over the DAPv2 bulk interface.
    pub fn dap2_reply(&mut self, data: &[u8]) {
        let _ = self.dap_v2.write_packet(data);
    }
}

/// Enable USBD peripheral interrupts required by `nrf-usbd`.
#[cfg(target_arch = "arm")]
pub fn enable_usbd_interrupts() {
    unsafe {
        let usbd = &*nrf52840_hal::pac::USBD::ptr();
        usbd.intenset.write(|w| {
            w.usbreset()
                .set_bit()
                .usbevent()
                .set_bit()
                .ep0setup()
                .set_bit()
                .ep0datadone()
                .set_bit()
                .epdata()
                .set_bit()
                .sof()
                .set_bit()
        });
    }
}
