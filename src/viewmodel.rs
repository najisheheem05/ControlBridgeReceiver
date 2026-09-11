/*
 * Copyright (C) 2026 Ishan
 *
 * This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3 only.
 *
 * This program is distributed without any warranty. See the GNU General Public License for more details.
 */

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use byteorder::{LittleEndian, WriteBytesExt};
use log::info;

use crate::data::GamepadState;
use crate::input::xinput::InputExecutor;
#[cfg(target_os = "windows")]
use crate::input::xinput::XInputExecutor;
use crate::session::{SessionManager, SlotAssignment, MAX_PLAYERS};
use crate::utils::network::{DiscoveryServer, UdpReceiver, FEATURE_LATENCY};
#[cfg(target_os = "windows")]
use crate::utils::network::FEATURE_RUMBLE;

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
        let discovery = Arc::new(DiscoveryServer::new(8083));
        let receiver = Arc::new(UdpReceiver::new(8082));
        let is_full = Arc::new(AtomicBool::new(false));

        let executors: Arc<Vec<Mutex<Option<Box<dyn InputExecutor>>>>> =
            Arc::new((0..MAX_PLAYERS).map(|_| Mutex::new(None)).collect());

        // --- Start UDP receiver ---
        {
            let session = Arc::clone(&session);
            let receiver_ref = Arc::clone(&receiver);
            let executors = Arc::clone(&executors);
            let ui_cb = Arc::clone(&on_ui_update);
            let is_full = Arc::clone(&is_full);

            receiver.start(move |src, state, sent_time| {
                match session.get_or_assign(src) {
                    SlotAssignment::Existing(slot) => {
                        session.touch(slot);

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
fn setup_slot(
    slot: usize,
    executors: &Arc<Vec<Mutex<Option<Box<dyn InputExecutor>>>>>,
    receiver: &Arc<UdpReceiver>,
    session: &Arc<SessionManager>,
) {
    #[cfg(target_os = "windows")]
    {
        let mut guard = executors[slot].lock().unwrap();
        let mut executor = Box::new(XInputExecutor::new().expect("ViGEm failed"));

        let receiver_clone = Arc::clone(receiver);
        let session_clone = Arc::clone(session);

        executor.set_rumble_callback(Box::new(move |large, small| {
            if session_clone.get_features(slot) & FEATURE_RUMBLE != 0 {
                if let Some(addr) = session_clone.get_addr(slot) {
                    let packet = [1u8, large, small];
                    receiver_clone.send_to(&packet, addr);
                }
            }
        }));

        *guard = Some(executor);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (slot, executors, receiver, session);
        unimplemented!("Linux executor is not implemented yet");
    }
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