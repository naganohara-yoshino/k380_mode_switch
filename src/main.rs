#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use k380_mode_switch::{K380ModeSwitcher, KeyMode};

use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice},
        Enumeration::{DeviceInformation, DeviceInformationUpdate, DeviceWatcher},
    },
    Foundation::TypedEventHandler,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::Result,
};

const TARGET_MODE: KeyMode = KeyMode::FunctionKeys;
const MAX_ATTEMPTS: usize = 10;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const UPDATE_THROTTLE: Duration = Duration::from_secs(30);

struct WinRtGuard;

impl WinRtGuard {
    fn new() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }

        Ok(Self)
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

fn request_apply(worker: &SyncSender<()>) {
    let _ = worker.try_send(());
}

fn request_apply_throttled(
    worker: &SyncSender<()>,
    last_request: &Mutex<Option<Instant>>,
    throttle: Duration,
) {
    let now = Instant::now();

    let Ok(mut last_request) = last_request.lock() else {
        return;
    };

    let should_request = match *last_request {
        Some(last) => now.duration_since(last) >= throttle,
        None => true,
    };

    if should_request {
        *last_request = Some(now);
        request_apply(worker);
    }
}

fn start_worker(mode: KeyMode) -> SyncSender<()> {
    let (sender, receiver) = sync_channel::<()>(1);

    thread::spawn(move || {
        let mut switcher = match K380ModeSwitcher::new() {
            Ok(switcher) => switcher,
            Err(error) => {
                #[cfg(debug_assertions)]
                eprintln!("failed to initialize K380 mode switcher: {error}");
                return;
            }
        };

        while receiver.recv().is_ok() {
            apply_with_retry(&mut switcher, mode);
        }
    });

    sender
}

fn apply_with_retry(switcher: &mut K380ModeSwitcher, mode: KeyMode) {
    for attempt in 1..=MAX_ATTEMPTS {
        match switcher.set_key_mode(mode) {
            Ok(()) => {
                #[cfg(debug_assertions)]
                println!("Successfully set K380 mode to {mode:?}");
                return;
            }
            Err(error) => {
                #[cfg(debug_assertions)]
                eprintln!("failed to set K380 mode, attempt {attempt}/{MAX_ATTEMPTS}: {error}");
            }
        }

        thread::sleep(RETRY_DELAY);
    }
}

fn run() -> Result<()> {
    let _winrt = WinRtGuard::new()?;

    let worker = start_worker(TARGET_MODE);
    let last_update_request = Arc::new(Mutex::new(None));

    let selector = BluetoothDevice::GetDeviceSelectorFromConnectionStatus(
        BluetoothConnectionStatus::Connected,
    )?;

    let watcher = DeviceInformation::CreateWatcherAqsFilter(&selector)?;

    let added_worker = worker.clone();
    let added = TypedEventHandler::<DeviceWatcher, DeviceInformation>::new(move |_, _| {
        request_apply(&added_worker);
        Ok(())
    });

    let updated_worker = worker.clone();
    let updated_last_request = Arc::clone(&last_update_request);
    let updated = TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(move |_, _| {
        request_apply_throttled(&updated_worker, &updated_last_request, UPDATE_THROTTLE);
        Ok(())
    });

    let _added_token = watcher.Added(&added)?;
    let _updated_token = watcher.Updated(&updated)?;

    watcher.Start()?;

    request_apply(&worker);

    loop {
        thread::park();
    }
}

fn main() {
    if let Err(error) = run() {
        #[cfg(debug_assertions)]
        eprintln!("{error:?}");
    }
}
