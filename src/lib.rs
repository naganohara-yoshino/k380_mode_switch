use hidapi::{HidApi, HidError};
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("Logitech K380 HID interface not found")]
    DeviceNotFound,

    #[error("HID operation failed: {0}")]
    Hid(#[from] HidError),

    #[error("incomplete HID write: expected {expected} bytes, wrote {actual}")]
    ShortWrite { expected: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn set_key_mode(mode: KeyMode) -> Result<()> {
    let api = HidApi::new()?;

    let info = api
        .device_list()
        .find(|device| {
            device.vendor_id() == VID
                && device.product_id() == PID
                && device.usage() == USAGE
                && device.usage_page() == USAGE_PAGE
        })
        .ok_or(Error::DeviceNotFound)?;

    let device = info.open_device(&api)?;

    let report = match mode {
        KeyMode::FunctionKeys => &FUNCTION_KEYS_REPORT,
        KeyMode::MediaKeys => &MEDIA_KEYS_REPORT,
    };

    let actual = device.write(report)?;

    if actual != report.len() {
        return Err(Error::ShortWrite {
            expected: report.len(),
            actual,
        });
    }

    Ok(())
}

pub fn set_function_keys() -> Result<()> {
    set_key_mode(KeyMode::FunctionKeys)
}

pub fn set_media_keys() -> Result<()> {
    set_key_mode(KeyMode::MediaKeys)
}
