/*
 * Copyright (C) 2026 Ishan
 *
 * This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3 only.
 *
 * This program is distributed without any warranty. See the GNU General Public License for more details.
 */

use std::io::Cursor;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use byteorder::{LittleEndian, ReadBytesExt};
use log::info;

use crate::data::GamepadState;

pub const MIN_SUPPORTED_VERSION: i32 = 2;
pub const FEATURE_RUMBLE: i32 = 1 << 0;
pub const FEATURE_LATENCY: i32 = 1 << 1;

pub struct DiscoveryServer {
    port: u16,
    is_running: Arc<AtomicBool>,
}

impl DiscoveryServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts the discovery listener.
    ///
    /// The server runs continuously until [`stop`](Self::stop) is called.
    /// When `is_full` is `true`, incoming discovery requests are silently
    /// ignored (the client will timeout and can retry later when a slot opens).
    ///
    /// `on_responded` is called with the client's IP and the agreed feature
    /// flags after a successful handshake, so the caller can pre-register
    /// per-client features before data packets arrive.
    pub fn start(
        &self,
        is_full: Arc<AtomicBool>,
        on_responded: impl Fn(IpAddr, i32) + Send + 'static,
    ) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let socket = UdpSocket::bind(("0.0.0.0", self.port)).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .unwrap();
        let running = Arc::clone(&self.is_running);

        thread::spawn(move || {
            let mut buf = [0u8; 256];
            while running.load(Ordering::SeqCst) {
                if let Ok((amt, src)) = socket.recv_from(&mut buf) {
                    // Don't respond when all player slots are occupied
                    if is_full.load(Ordering::SeqCst) {
                        continue;
                    }

                    let msg = String::from_utf8_lossy(&buf[..amt]);
                    if msg.starts_with("PADCONNECT_DISCOVER") {
                        let parts: Vec<&str> = msg.split(':').collect();
                        let client_version =
                            parts.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
                        let client_features =
                            parts.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);

                        if client_version < MIN_SUPPORTED_VERSION {
                            info!("App Update Required for client at {}", src);
                        }

                        let agreed_version = client_version.min(MIN_SUPPORTED_VERSION);
                        let agreed_features = client_features & (FEATURE_RUMBLE | FEATURE_LATENCY);

                        let response = format!(
                            "PADCONNECT_HERE:8082:{}:{}",
                            agreed_version, agreed_features
                        );
                        let _ = socket.send_to(response.as_bytes(), src);

                        on_responded(src.ip(), agreed_features);
                    }
                }
            }
        });
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

pub struct UdpReceiver {
    port: u16,
    is_running: Arc<AtomicBool>,
    socket: Arc<Mutex<Option<UdpSocket>>>,
}

impl UdpReceiver {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            is_running: Arc::new(AtomicBool::new(false)),
            socket: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts receiving gamepad packets.
    ///
    /// For each valid input packet (type 0), calls `on_event` with:
    /// - The source `SocketAddr` (for session routing)
    /// - The parsed `GamepadState`
    /// - An optional latency timestamp (`i64` nanos) if the packet contained one
    ///
    /// Feature checks (rumble, latency) are NOT handled here — the caller
    /// (viewmodel) decides per-slot behavior via the [`SessionManager`].
    pub fn start(
        &self,
        on_event: impl Fn(SocketAddr, GamepadState, Option<i64>) + Send + 'static,
    ) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let socket = UdpSocket::bind(("0.0.0.0", self.port)).unwrap();
        *self.socket.lock().unwrap() = Some(socket.try_clone().expect("Failed to clone socket"));
        let running = Arc::clone(&self.is_running);

        thread::spawn(move || {
            let mut buf = [0u8; 21];
            while running.load(Ordering::SeqCst) {
                if let Ok((amt, src)) = socket.recv_from(&mut buf) {
                    let mut cursor = Cursor::new(&buf[..amt]);
                    if let Ok(packet_type) = cursor.read_u8() {
                        if packet_type == 0 {
                            let state = GamepadState {
                                buttons: cursor.read_u16::<LittleEndian>().unwrap_or(0),
                                lx: cursor.read_i16::<LittleEndian>().unwrap_or(0),
                                ly: cursor.read_i16::<LittleEndian>().unwrap_or(0),
                                rx: cursor.read_i16::<LittleEndian>().unwrap_or(0),
                                ry: cursor.read_i16::<LittleEndian>().unwrap_or(0),
                                lt: cursor.read_u8().unwrap_or(0),
                                rt: cursor.read_u8().unwrap_or(0),
                            };

                            // Read the optional latency timestamp (present if packet has remaining bytes)
                            let sent_time = cursor.read_i64::<LittleEndian>().ok();

                            on_event(src, state, sent_time);
                        }
                    }
                }
            }
        });
    }

    /// Sends a raw packet to the given target address via the bound socket.
    /// Used by the viewmodel for latency echo and rumble responses.
    pub fn send_to(&self, data: &[u8], target: SocketAddr) {
        if let Some(socket) = self.socket.lock().unwrap().as_ref() {
            let _ = socket.send_to(data, target);
        }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}