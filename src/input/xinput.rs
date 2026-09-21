/*
 * Copyright (C) 2026 Ishan
 *
 * This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3 only.
 *
 * This program is distributed without any warranty. See the GNU General Public License for more details.
 */

use crate::data::GamepadState;
use log::{warn, debug, trace};

pub trait InputExecutor: Send {
    fn submit(&mut self, state: &GamepadState);
    fn shutdown(&mut self);
    fn set_rumble_callback(&mut self, callback: Box<dyn Fn(u8, u8) + Send>);
}

/// Download page for the ViGEmBus driver (Windows only).
#[cfg(target_os = "windows")]
pub const VIGEM_DOWNLOAD_URL: &str = "https://github.com/nefarius/ViGEmBus/releases/latest";

/// Returns `true` if the error means the ViGEmBus driver is not installed.
#[cfg(target_os = "windows")]
pub fn is_bus_missing(e: &vigem_client::Error) -> bool {
    matches!(
        e,
        vigem_client::Error::BusNotFound
            | vigem_client::Error::BusAccessFailed(_)
            | vigem_client::Error::BusVersionMismatch
    )
}

/// Prints a clear "ViGEmBus not found" message with a download link to the
/// console (cmd) and opens the download page in the default browser.
///
/// The plain URL is clickable in Windows Terminal / modern consoles.
#[cfg(target_os = "windows")]
pub fn report_vigem_missing(context: &str, e: &vigem_client::Error) {
    use log::error;

    error!("{}: ViGEmBus not found ({:?})", context, e);

    eprintln!();
    eprintln!("================================================================");
    eprintln!("  ERROR: ViGEmBus driver not found");
    eprintln!("  {} failed: {:?}", context, e);
    eprintln!();
    eprintln!("  Virtual controllers need the ViGEmBus driver to work.");
    eprintln!("  Download from here: {}", VIGEM_DOWNLOAD_URL);
    eprintln!("  Install it, then restart ControlBridgeReceiver.");
    eprintln!("  Opening the download page in your browser in 10 seconds...");
    eprintln!("================================================================");
    eprintln!();

    // Give the user time to read the message before the browser pops up.
    // Runs on a background thread so the app itself never blocks.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(10));
        open_download_page();
    });
}

/// Opens the ViGEmBus download page in the default browser (Windows).
#[cfg(target_os = "windows")]
pub fn open_download_page() {
    // `start "" <url>` opens the URL with the default browser.
    let started = std::process::Command::new("cmd")
        .args(["/C", "start", "", VIGEM_DOWNLOAD_URL])
        .spawn()
        .is_ok();

    if !started {
        // Fallback that also delegates to the default browser.
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", VIGEM_DOWNLOAD_URL])
            .spawn();
    }
}

#[cfg(target_os = "windows")]
pub struct XInputExecutor {
    // Kept alive for the executor's lifetime: dropping it would close the
    // underlying bus handle out from under the cloned target handle.
    #[allow(dead_code)]
    client: vigem_client::Client,
    target: vigem_client::Xbox360Wired<vigem_client::Client>,
}

#[cfg(target_os = "windows")]
impl XInputExecutor {
    pub fn new() -> Result<Self, vigem_client::Error> {
        let client = vigem_client::Client::connect()?;
        let mut target = vigem_client::Xbox360Wired::new(
            client.try_clone().unwrap(), 
            vigem_client::TargetId::XBOX360_WIRED
        );
        target.plugin()?;
        
        Ok(Self { client, target })
    }

    fn deadzone(v: i16) -> i16 {
        if v.abs() < 4000 { 0 } else { v }
    }
}

#[cfg(target_os = "windows")]
impl InputExecutor for XInputExecutor {
    fn submit(&mut self, state: &GamepadState) {
        let report = vigem_client::XGamepad {
            buttons: vigem_client::XButtons(state.buttons),
            left_trigger: state.lt,
            right_trigger: state.rt,
            thumb_lx: Self::deadzone(state.lx),
            thumb_ly: Self::deadzone(state.ly),
            thumb_rx: Self::deadzone(state.rx),
            thumb_ry: Self::deadzone(state.ry),
        };
        let _ = self.target.update(&report);
    }

    fn shutdown(&mut self) {
        let _ = self.target.unplug();
    }

    fn set_rumble_callback(&mut self, callback: Box<dyn Fn(u8, u8) + Send>) {
        match self.target.request_notification() {
            Ok(notification) => {
                debug!("Rumble notification thread registered");
                notification.spawn_thread(move |_notif, data| {
                    trace!("Rumble: large={} small={}", data.large_motor, data.small_motor);
                    callback(data.large_motor, data.small_motor);
                });
            }
            Err(e) => {
                warn!("Failed to request rumble notification: {:?}", e);
            }
        }
    }
}
