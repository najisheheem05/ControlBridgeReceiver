//! Linux virtual gamepad backend built on top of `uinput`.
//!
//! The virtual device mimics a wired Xbox 360 controller (Microsoft vendor and
//! product IDs, USB bus) so that SDL, Steam, Wine/Proton and games pick the
//! proper Xbox mapping automatically. Force feedback is handled on the worker
//! thread and forwarded to the phone through the existing rumble callback.

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError, bounded};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventSummary, EventType, FFEffectCode,
    FFEffectKind, InputEvent, InputId, KeyCode, UInputCode, UinputAbsSetup, uinput::VirtualDevice,
};
use log::{debug, info, warn};

use crate::data::GamepadState;

use super::InputExecutor;

/// Xbox 360 wired controller identification.
const XBOX360_VENDOR: u16 = 0x045e;
const XBOX360_PRODUCT: u16 = 0x028e;
const XBOX360_VERSION: u16 = 0x0114;

/// Maximum number of simultaneously uploaded force-feedback effects.
const FF_EFFECTS_MAX: u32 = 16;

/// Stick values below this magnitude are reported as centred.
const STICK_DEADZONE: i16 = 4000;

/// Maximum value used by Linux force-feedback magnitudes.
const FF_MAX: u32 = 0xffff;

// XInput button bit masks (same values sent by the Android app).
const BTN_DPAD_UP: u16 = 0x0001;
const BTN_DPAD_DOWN: u16 = 0x0002;
const BTN_DPAD_LEFT: u16 = 0x0004;
const BTN_DPAD_RIGHT: u16 = 0x0008;
const BTN_START: u16 = 0x0010;
const BTN_BACK: u16 = 0x0020;
const BTN_LEFT_THUMB: u16 = 0x0040;
const BTN_RIGHT_THUMB: u16 = 0x0080;
const BTN_LEFT_SHOULDER: u16 = 0x0100;
const BTN_RIGHT_SHOULDER: u16 = 0x0200;
const BTN_GUIDE: u16 = 0x0400;
const BTN_A: u16 = 0x1000;
const BTN_B: u16 = 0x2000;
const BTN_X: u16 = 0x4000;
const BTN_Y: u16 = 0x8000;

const KEY_MAP: &[(u16, KeyCode)] = &[
    (BTN_A, KeyCode::BTN_SOUTH),
    (BTN_B, KeyCode::BTN_EAST),
    (BTN_X, KeyCode::BTN_NORTH),
    (BTN_Y, KeyCode::BTN_WEST),
    (BTN_LEFT_SHOULDER, KeyCode::BTN_TL),
    (BTN_RIGHT_SHOULDER, KeyCode::BTN_TR),
    (BTN_BACK, KeyCode::BTN_SELECT),
    (BTN_START, KeyCode::BTN_START),
    (BTN_GUIDE, KeyCode::BTN_MODE),
    (BTN_LEFT_THUMB, KeyCode::BTN_THUMBL),
    (BTN_RIGHT_THUMB, KeyCode::BTN_THUMBR),
];

type RumbleCallback = Arc<Mutex<Option<Box<dyn Fn(u8, u8) + Send>>>>;

pub const UINPUT_HELP: &str = "Load the `uinput` kernel module and allow access to /dev/uinput. \
Example udev rule: KERNEL==\"uinput\", SUBSYSTEM==\"misc\", OPTIONS+=\"static_node=uinput\", TAG+=\"uaccess\"";

/// Checks that `/dev/uinput` exists and can be opened read-write.
pub fn check_uinput() -> io::Result<()> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .map(|_| ())
}

/// Prints a clear diagnostic when the uinput device cannot be used.
pub fn report_uinput_missing(context: &str, e: &io::Error) {
    log::error!("{}: cannot open /dev/uinput ({})", context, e);

    eprintln!();
    eprintln!("================================================================");
    eprintln!("  ERROR: Linux virtual gamepad support is unavailable");
    eprintln!("  {} failed: {}", context, e);
    eprintln!();
    eprintln!("  ControlBridgeReceiver needs the kernel uinput module to");
    eprintln!("  create virtual controllers.");
    eprintln!();
    eprintln!("  Load it with:  sudo modprobe uinput");
    eprintln!();
    eprintln!("  Your user also needs access to /dev/uinput. Example udev rule:");
    eprintln!(
        "    KERNEL==\"uinput\", SUBSYSTEM==\"misc\", OPTIONS+=\"static_node=uinput\", TAG+=\"uaccess\""
    );
    eprintln!();
    eprintln!("  The rule is not needed on most modern distributions, since");
    eprintln!("  systemd already grants the active session access via uaccess.");
    eprintln!("================================================================");
    eprintln!();
}

/// Linux implementation of [`InputExecutor`] backed by `/dev/uinput`.
///
/// Each executor owns one virtual Xbox 360 pad. State updates are sent to a
/// worker thread so gamepad events and force feedback can be serviced from the
/// same `VirtualDevice` without blocking the UDP receiver.
pub struct UinputExecutor {
    sender: Sender<GamepadState>,
    drain: Receiver<GamepadState>,
    stop: Arc<AtomicBool>,
    callback: RumbleCallback,
    worker: Option<JoinHandle<()>>,
}

impl UinputExecutor {
    pub fn new(slot: usize) -> io::Result<Self> {
        let mut device = build_device(slot)?;
        set_nonblocking(device.as_raw_fd())?;

        let (sender, receiver) = bounded(1);
        let drain = receiver.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let callback: RumbleCallback = Arc::new(Mutex::new(None));

        let worker = {
            let stop = Arc::clone(&stop);
            let callback = Arc::clone(&callback);
            thread::Builder::new()
                .name(format!("uinput-player-{}", slot + 1))
                .spawn(move || worker_loop(&mut device, receiver, stop, callback))?
        };

        info!("Created virtual Xbox 360 pad for Player {}", slot + 1);

        Ok(Self {
            sender,
            drain,
            stop,
            callback,
            worker: Some(worker),
        })
    }

    fn stop_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl InputExecutor for UinputExecutor {
    fn submit(&mut self, state: &GamepadState) {
        // Keep only the freshest state: replace a stale queued packet instead
        // of letting a backlog build up between worker wake-ups.
        match self.sender.try_send(*state) {
            Ok(()) => {}
            Err(TrySendError::Full(state)) => {
                let _ = self.drain.try_recv();
                let _ = self.sender.try_send(state);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    fn shutdown(&mut self) {
        self.stop_worker();
    }

    fn set_rumble_callback(&mut self, callback: Box<dyn Fn(u8, u8) + Send>) {
        if let Ok(mut slot) = self.callback.lock() {
            *slot = Some(callback);
        }
    }
}

impl Drop for UinputExecutor {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

fn build_device(slot: usize) -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for (_, key) in KEY_MAP {
        keys.insert(*key);
    }

    let mut ff = AttributeSet::<FFEffectCode>::new();
    for effect in [
        FFEffectCode::FF_RUMBLE,
        FFEffectCode::FF_CONSTANT,
        FFEffectCode::FF_PERIODIC,
        FFEffectCode::FF_SINE,
        FFEffectCode::FF_RAMP,
        FFEffectCode::FF_GAIN,
    ] {
        ff.insert(effect);
    }

    let stick = AbsInfo::new(0, -32768, 32767, 16, 128, 0);
    let trigger = AbsInfo::new(0, 0, 255, 0, 0, 0);
    let hat = AbsInfo::new(0, -1, 1, 0, 0, 0);

    let name = format!("ControlBridge X-Box 360 pad (Player {})", slot + 1);

    VirtualDevice::builder()?
        .name(&name)
        .input_id(InputId::new(
            BusType::BUS_USB,
            XBOX360_VENDOR,
            XBOX360_PRODUCT,
            XBOX360_VERSION,
        ))
        .with_keys(&keys)?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RX, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RY, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Z, trigger))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RZ, trigger))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0X, hat))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0Y, hat))?
        .with_ff(&ff)?
        .with_ff_effects_max(FF_EFFECTS_MAX)
        .build()
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

fn worker_loop(
    device: &mut VirtualDevice,
    receiver: Receiver<GamepadState>,
    stop: Arc<AtomicBool>,
    callback: RumbleCallback,
) {
    let mut last = GamepadState::default();
    let mut effects: HashMap<i32, RumbleEffect> = HashMap::new();
    let mut gain: u32 = FF_MAX;
    let mut last_rumble = (0u8, 0u8);

    while !stop.load(Ordering::SeqCst) {
        match receiver.recv_timeout(Duration::from_millis(5)) {
            Ok(state) => {
                emit_state(device, &last, &state);
                last = state;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        handle_ff_events(device, &mut effects, &mut gain);

        let rumble = current_rumble(&effects, gain, Instant::now());
        if rumble != last_rumble {
            last_rumble = rumble;
            if let Ok(callback) = callback.lock()
                && let Some(callback) = callback.as_ref()
            {
                callback(rumble.0, rumble.1);
            }
        }
    }

    debug!("Virtual gamepad worker stopped");
}

fn deadzone(value: i16) -> i16 {
    if value.unsigned_abs() < STICK_DEADZONE as u16 {
        0
    } else {
        value
    }
}

fn hat_x(buttons: u16) -> i32 {
    i32::from(buttons & BTN_DPAD_RIGHT != 0) - i32::from(buttons & BTN_DPAD_LEFT != 0)
}

fn hat_y(buttons: u16) -> i32 {
    i32::from(buttons & BTN_DPAD_DOWN != 0) - i32::from(buttons & BTN_DPAD_UP != 0)
}

fn emit_state(device: &mut VirtualDevice, last: &GamepadState, state: &GamepadState) {
    let mut events = Vec::with_capacity(16);

    let lx = deadzone(state.lx);
    let ly = -i32::from(deadzone(state.ly));
    let rx = deadzone(state.rx);
    let ry = -i32::from(deadzone(state.ry));

    if lx != deadzone(last.lx) {
        events.push(axis(AbsoluteAxisCode::ABS_X, i32::from(lx)));
    }
    // Android reports Y positive-up; evdev expects positive-down.
    if ly != -i32::from(deadzone(last.ly)) {
        events.push(axis(AbsoluteAxisCode::ABS_Y, ly));
    }
    if rx != deadzone(last.rx) {
        events.push(axis(AbsoluteAxisCode::ABS_RX, i32::from(rx)));
    }
    if ry != -i32::from(deadzone(last.ry)) {
        events.push(axis(AbsoluteAxisCode::ABS_RY, ry));
    }

    if state.lt != last.lt {
        events.push(axis(AbsoluteAxisCode::ABS_Z, i32::from(state.lt)));
    }
    if state.rt != last.rt {
        events.push(axis(AbsoluteAxisCode::ABS_RZ, i32::from(state.rt)));
    }

    if hat_x(state.buttons) != hat_x(last.buttons) {
        events.push(axis(AbsoluteAxisCode::ABS_HAT0X, hat_x(state.buttons)));
    }
    if hat_y(state.buttons) != hat_y(last.buttons) {
        events.push(axis(AbsoluteAxisCode::ABS_HAT0Y, hat_y(state.buttons)));
    }

    let changed = state.buttons ^ last.buttons;
    if changed != 0 {
        for (mask, key) in KEY_MAP {
            if changed & mask != 0 {
                let pressed = state.buttons & mask != 0;
                events.push(InputEvent::new(EventType::KEY.0, key.0, i32::from(pressed)));
            }
        }
    }

    if events.is_empty() {
        return;
    }

    if let Err(e) = device.emit(&events) {
        warn!("Failed to emit virtual gamepad state: {}", e);
    }
}

fn axis(code: AbsoluteAxisCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE.0, code.0, value)
}

#[derive(Clone, Copy)]
struct RumbleEffect {
    strong: u16,
    weak: u16,
    length: Duration,
    delay: Duration,
    started: Option<Instant>,
}

fn handle_ff_events(
    device: &mut VirtualDevice,
    effects: &mut HashMap<i32, RumbleEffect>,
    gain: &mut u32,
) {
    loop {
        let events: Vec<InputEvent> = match device.fetch_events() {
            Ok(events) => events.collect(),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => {
                warn!("Failed to read uinput force-feedback events: {}", e);
                break;
            }
        };

        if events.is_empty() {
            break;
        }

        for event in events {
            handle_ff_event(device, event, effects, gain);
        }
    }
}

fn handle_ff_event(
    device: &mut VirtualDevice,
    event: InputEvent,
    effects: &mut HashMap<i32, RumbleEffect>,
    gain: &mut u32,
) {
    match event.destructure() {
        EventSummary::UInput(event, UInputCode::UI_FF_UPLOAD, _) => {
            let upload = device.process_ff_upload(event);
            match upload {
                Ok(mut upload) => {
                    let id = i32::from(upload.effect_id());
                    match rumble_effect(&upload.effect()) {
                        Some(mut effect) => {
                            // Re-uploading an effect must not restart it.
                            if let Some(existing) = effects.get(&id) {
                                effect.started = existing.started;
                            }
                            effects.insert(id, effect);
                        }
                        None => {
                            effects.remove(&id);
                        }
                    }
                    upload.set_retval(0);
                }
                Err(e) => warn!("Force-feedback upload failed: {}", e),
            }
        }
        EventSummary::UInput(event, UInputCode::UI_FF_ERASE, _) => {
            match device.process_ff_erase(event) {
                Ok(mut erase) => {
                    effects.remove(&(erase.effect_id() as i32));
                    erase.set_retval(0);
                }
                Err(e) => warn!("Force-feedback erase failed: {}", e),
            }
        }
        EventSummary::ForceFeedback(_, code, value) => {
            if code == FFEffectCode::FF_GAIN {
                *gain = value.clamp(0, FF_MAX as i32) as u32;
            } else if let Some(effect) = effects.get_mut(&i32::from(code.0)) {
                if value != 0 {
                    // Delay and length use milliseconds, as in the kernel API.
                    effect.started = Some(Instant::now() + effect.delay);
                } else {
                    effect.started = None;
                }
            }
        }
        _ => {}
    }
}

fn rumble_effect(data: &evdev::FFEffectData) -> Option<RumbleEffect> {
    let (strong, weak) = match data.kind {
        FFEffectKind::Rumble {
            strong_magnitude,
            weak_magnitude,
        } => (strong_magnitude, weak_magnitude),
        FFEffectKind::Constant { level, .. } => {
            let magnitude = level.unsigned_abs();
            (magnitude, magnitude)
        }
        FFEffectKind::Periodic { magnitude, .. } => {
            let magnitude = magnitude.unsigned_abs();
            (magnitude, magnitude)
        }
        FFEffectKind::Ramp {
            start_level,
            end_level,
            ..
        } => {
            let magnitude = start_level.unsigned_abs().max(end_level.unsigned_abs());
            (magnitude, magnitude)
        }
        _ => return None,
    };

    Some(RumbleEffect {
        strong,
        weak,
        length: Duration::from_millis(u64::from(data.replay.length)),
        delay: Duration::from_millis(u64::from(data.replay.delay)),
        started: None,
    })
}

fn current_rumble(effects: &HashMap<i32, RumbleEffect>, gain: u32, now: Instant) -> (u8, u8) {
    let mut strong: u32 = 0;
    let mut weak: u32 = 0;

    for effect in effects.values() {
        let Some(started) = effect.started else {
            continue;
        };

        let finished = !effect.length.is_zero() && now >= started + effect.length;
        if now >= started && !finished {
            strong = strong.saturating_add(u32::from(effect.strong));
            weak = weak.saturating_add(u32::from(effect.weak));
        }
    }

    let scale = |value: u32| -> u8 {
        let value = value.min(FF_MAX);
        ((u64::from(value) * u64::from(gain) * 255) / (u64::from(FF_MAX) * u64::from(FF_MAX))) as u8
    };

    (scale(strong), scale(weak))
}
