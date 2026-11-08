use crate::data::GamepadState;
use crate::utils::network::FEATURE_LATENCY;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::utils::network::FEATURE_RUMBLE;

/// Platform-specific backend that turns received input states into a virtual
/// gamepad device.
pub trait InputExecutor: Send {
    fn submit(&mut self, state: &GamepadState);
    fn shutdown(&mut self);
    fn set_rumble_callback(&mut self, callback: Box<dyn Fn(u8, u8) + Send>);
}

/// Discovery features the current platform backend can honour.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub const SUPPORTED_FEATURES: i32 = FEATURE_RUMBLE | FEATURE_LATENCY;

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub const SUPPORTED_FEATURES: i32 = FEATURE_LATENCY;

pub mod xinput;

#[cfg(target_os = "linux")]
pub mod uinput;
