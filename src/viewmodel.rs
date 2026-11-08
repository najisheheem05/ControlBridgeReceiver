use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use byteorder::{LittleEndian, WriteBytesExt};
use log::info;

use crate::data::GamepadState;
use crate::input::InputExecutor;
#[cfg(target_os = "linux")]
use crate::input::uinput::{report_uinput_missing, UinputExecutor, UINPUT_HELP};
#[cfg(target_os = "windows")]
use crate::input::xinput::{report_vigem_missing, XInputExecutor};
use crate::session::{SessionManager, SlotAssignment, MAX_PLAYERS};
use crate::utils::network::{DiscoveryServer, UdpReceiver, FEATURE_LATENCY, FEATURE_RUMBLE};

pub struct ReceiverViewModel {
    discovery: Arc<DiscoveryServer>,
    receiver: Arc<UdpReceiver>,
    session: Arc<SessionManager>,
    executors: Arc<Vec<Mutex<Option<Box<dyn InputExecutor>>>>>,
}

impl ReceiverViewModel {
    pub fn new<F>(on_ui_update: F) -> Self
    where
        F: Fn(usize, GamepadState, bool) + Send + Sync + 'static,
    {
        let on_ui_update = Arc::new(on_ui_update);

        let session = Arc::new(SessionManager::new());
        let discovery = Arc::new(DiscoveryServer::new(8083, crate::input::SUPPORTED_FEATURES));
        let receiver = Arc::new(UdpReceiver::new(8082));
        let is_full = Arc::new(AtomicBool::new(false));

        let executors: Arc<Vec<Mutex<Option<Box<dyn InputExecutor>>>>> =
            Arc::new((0..MAX_PLAYERS).map(|_| Mutex::new(None)).collect());

        // Last executor (re)connect attempt per slot, in millis since epoch.
        // Throttles retries so a missing driver/device doesn't trigger a
        // device enumeration + log line on every incoming packet.
        let executor_retry_ms: Arc<Mutex<[u64; MAX_PLAYERS]>> =
            Arc::new(Mutex::new([0; MAX_PLAYERS]));

        // --- Start UDP receiver ---
        {
            let session = Arc::clone(&session);
            let receiver_ref = Arc::clone(&receiver);
            let executors = Arc::clone(&executors);
            let ui_cb = Arc::clone(&on_ui_update);
            let is_full = Arc::clone(&is_full);
            let executor_retry_ms = Arc::clone(&executor_retry_ms);

            receiver.start(move |src, state, sent_time| {
                match session.get_or_assign(src) {
                    SlotAssignment::Existing(slot) => {
                        session.touch(slot);

                        // Retry executor setup if a previous attempt failed
                        // (e.g. ViGEmBus was installed or uinput became
                        // accessible after the first packet), throttled to one
                        // attempt per slot every 5 seconds.
                        if executors[slot].lock().unwrap().is_none() {
                            let now = now_ms();
                            let mut retry = executor_retry_ms.lock().unwrap();
                            if now.saturating_sub(retry[slot]) >= 5000 {
                                retry[slot] = now;
                                drop(retry);
                                setup_slot(slot, &executors, &receiver_ref, &session);
                            }
                        }
                        if let Some(exec) = executors[slot].lock().unwrap().as_mut() {
                            exec.submit(&state);
                        }

                        // Latency echo (per-slot feature check)
                        if let Some(ts) = sent_time {
                            if session.get_features(slot) & FEATURE_LATENCY != 0 {
                                send_latency_response(&receiver_ref, src, ts);
                            }
                        }

                        ui_cb(slot, state, true);
                    }
                    SlotAssignment::New { slot, features } => {
                        info!("Setting up executor for Player {}", slot + 1);
                        setup_slot(slot, &executors, &receiver_ref, &session);

                        // Submit first state
                        if let Some(exec) = executors[slot].lock().unwrap().as_mut() {
                            exec.submit(&state);
                        }

                        // Update is_full flag
                        is_full.store(session.is_full(), Ordering::SeqCst);

                        // Latency echo
                        if let Some(ts) = sent_time {
                            if features & FEATURE_LATENCY != 0 {
                                send_latency_response(&receiver_ref, src, ts);
                            }
                        }

                        ui_cb(slot, state, true);
                    }
                    SlotAssignment::Full => {
                        // All slots occupied — drop packet silently
                    }
                }
            });
        }

        // --- Start discovery server (runs continuously) ---
        {
            let session = Arc::clone(&session);
            discovery.start(Arc::clone(&is_full), move |ip, features| {
                session.register_pending_features(ip, features);
                info!("Discovery agreed features for {}: {}", ip, features);
            });
        }

        let vm = Self {
            discovery,
            receiver,
            session,
            executors,
        };

        vm.start_connection_monitor(Arc::clone(&on_ui_update), is_full);
        vm
    }

    /// Monitors all slots for heartbeat timeouts every 500ms.
    /// When a slot times out, shuts down its executor and notifies the UI.
    /// Also keeps the `is_full` flag in sync so discovery resumes when a slot opens.
    fn start_connection_monitor<F>(&self, on_ui_update: Arc<F>, is_full: Arc<AtomicBool>)
    where
        F: Fn(usize, GamepadState, bool) + Send + Sync + 'static,
    {
        let session = Arc::clone(&self.session);
        let executors = Arc::clone(&self.executors);

        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(500));

            let timed_out = session.check_timeouts();
            for slot in timed_out {
                session.disconnect(slot);

                // Shut down the executor for this slot
                if let Some(mut exec) = executors[slot].lock().unwrap().take() {
                    exec.shutdown();
                }

                info!("Player {} timed out, slot freed", slot + 1);
                on_ui_update(slot, GamepadState::default(), false);
            }

            // Keep is_full in sync — discovery automatically resumes responding
            is_full.store(session.is_full(), Ordering::SeqCst);
        });
    }

    pub fn shutdown(&self) {
        self.receiver.stop();
        self.discovery.stop();
        for exec_lock in self.executors.iter() {
            if let Some(mut exec) = exec_lock.lock().unwrap().take() {
                exec.shutdown();
            }
        }
    }
}

/// Creates a new platform-specific input executor for the given slot and
/// wires up its rumble callback to send packets back through the UDP receiver.
///
/// Returns `false` when the executor could not be created (e.g. ViGEmBus is
/// missing or `/dev/uinput` is inaccessible), so callers can retry later
/// instead of panicking.
fn setup_slot(
    slot: usize,
    executors: &Arc<Vec<Mutex<Option<Box<dyn InputExecutor>>>>>,
    receiver: &Arc<UdpReceiver>,
    session: &Arc<SessionManager>,
) -> bool {
    let mut guard = executors[slot].lock().unwrap();
    // Don't recreate an executor that already exists.
    if guard.is_some() {
        return true;
    }

    let mut executor = match create_executor(slot) {
        Ok(executor) => executor,
        Err(error) => {
            log_executor_error(slot, &error);
            return false;
        }
    };

    let receiver_clone = Arc::clone(receiver);
    let session_clone = Arc::clone(session);

    executor.set_rumble_callback(Box::new(move |large, small| {
        if session_clone.get_features(slot) & FEATURE_RUMBLE != 0
            && let Some(addr) = session_clone.get_addr(slot)
        {
            let packet = [1u8, large, small];
            receiver_clone.send_to(&packet, addr);
        }
    }));

    *guard = Some(executor);
    true
}

#[cfg(target_os = "windows")]
fn create_executor(slot: usize) -> Result<Box<dyn InputExecutor>, vigem_client::Error> {
    let _ = slot;
    XInputExecutor::new().map(|executor| Box::new(executor) as Box<dyn InputExecutor>)
}

#[cfg(target_os = "linux")]
fn create_executor(slot: usize) -> Result<Box<dyn InputExecutor>, std::io::Error> {
    UinputExecutor::new(slot).map(|executor| Box::new(executor) as Box<dyn InputExecutor>)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn create_executor(_slot: usize) -> Result<Box<dyn InputExecutor>, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "No virtual gamepad backend for this platform",
    ))
}

#[cfg(target_os = "windows")]
fn log_executor_error(slot: usize, error: &vigem_client::Error) {
    // Only open the browser once per process; repeated controller packets
    // must not spam browser tabs.
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::SeqCst) {
        report_vigem_missing("ViGEmBus driver not found", error);
    } else {
        log::error!("Player {}: ViGEmBus driver not found ({:?})", slot + 1, error);
        log::error!(
            "Download from here: {}",
            crate::input::xinput::VIGEM_DOWNLOAD_URL
        );
    }
}

#[cfg(target_os = "linux")]
fn log_executor_error(slot: usize, error: &std::io::Error) {
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::SeqCst) {
        report_uinput_missing("Virtual gamepad creation failed", error);
    } else {
        log::error!(
            "Player {}: cannot create virtual gamepad ({})",
            slot + 1,
            error
        );
        log::error!("{}", UINPUT_HELP);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn log_executor_error(slot: usize, error: &std::io::Error) {
    log::error!(
        "Player {}: cannot create virtual gamepad ({})",
        slot + 1,
        error
    );
}

/// Builds and sends a latency echo packet (type 2) back to the client.
fn send_latency_response(receiver: &UdpReceiver, target: SocketAddr, sent_time: i64) {
    let mut response = Vec::with_capacity(17);
    response.push(2u8); // Packet type 2 = latency echo

    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    let _ = response.write_i64::<LittleEndian>(sent_time);
    let _ = response.write_i64::<LittleEndian>(now_nanos);

    receiver.send_to(&response, target);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}