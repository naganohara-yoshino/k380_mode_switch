use std::cmp::Ordering;

use hidapi::{DeviceInfo, HidApi, HidError};
use thiserror::Error;

const VID: u16 = 0x046d;
const PID: u16 = 0xb342;
const USAGE: u16 = 0x0001;
const USAGE_PAGE: u16 = 0xff00;

const FUNCTION_KEYS_REPORT: [u8; 7] = [0x10, 0xff, 0x0b, 0x1e, 0x00, 0x00, 0x00];

const MEDIA_KEYS_REPORT: [u8; 7] = [0x10, 0xff, 0x0b, 0x1e, 0x01, 0x00, 0x00];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    FunctionKeys,
    MediaKeys,
}

impl KeyMode {
    fn report(self) -> &'static [u8; 7] {
        match self {
            Self::FunctionKeys => &FUNCTION_KEYS_REPORT,
            Self::MediaKeys => &MEDIA_KEYS_REPORT,
        }
    }
}

#[derive(Debug, Error)]
pub enum ModeSwitchError {
    #[error("Logitech K380 HID interface not found")]
    DeviceNotFound,

    #[error("HID operation failed: {0}")]
    Hid(#[from] HidError),

    #[error("unexpected HID write length: expected {expected} bytes, wrote {actual}")]
    UnexpectedWriteLength { expected: usize, actual: usize },
}

pub type ModeSwitchResult<T> = std::result::Result<T, ModeSwitchError>;

pub struct K380ModeSwitcher {
    api: HidApi,
}

impl K380ModeSwitcher {
    pub fn new() -> ModeSwitchResult<Self> {
        Ok(Self {
            api: HidApi::new()?,
        })
    }

    pub fn set_key_mode(&mut self, mode: KeyMode) -> ModeSwitchResult<()> {
        self.api.refresh_devices()?;

        let info = self
            .api
            .device_list()
            .find(|device| is_k380_config_interface(device))
            .ok_or(ModeSwitchError::DeviceNotFound)?;

        let device = info.open_device(&self.api)?;
        let report = mode.report();
        let actual = device.write(report)?;

        match actual.cmp(&report.len()) {
            Ordering::Equal => Ok(()),
            _ => Err(ModeSwitchError::UnexpectedWriteLength {
                expected: report.len(),
                actual,
            }),
        }
    }
}

fn is_k380_config_interface(device: &DeviceInfo) -> bool {
    device.vendor_id() == VID
        && device.product_id() == PID
        && device.usage() == USAGE
        && device.usage_page() == USAGE_PAGE
}
