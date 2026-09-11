/*
 * Copyright (C) 2026 Ishan
 *
 * This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3 only.
 *
 * This program is distributed without any warranty. See the GNU General Public License for more details.
 */

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::info;

pub const MAX_PLAYERS: usize = 4;
pub const TIMEOUT_MS: u64 = 2000;

/// Result of attempting to assign a slot for an incoming client address.
pub enum SlotAssignment {
    /// Client already has a slot assigned.
    Existing(usize),
    /// Client was assigned a new slot with the given agreed features.
    New { slot: usize, features: i32 },
    /// All slots are occupied.
    Full,
}

struct SlotData {
    addr: SocketAddr,
    last_seen_ms: u64,
    features: i32,
}

struct SessionInner {
    slots: [Option<SlotData>; MAX_PLAYERS],
    /// Features negotiated during discovery, keyed by client IP.
    /// Consumed when the client's first data packet arrives and a slot is assigned.
    pending_features: HashMap<IpAddr, i32>,
}

/// Manages up to [`MAX_PLAYERS`] concurrent client sessions.
///
/// Maps each client's `SocketAddr` to a player slot (0..3), tracks
/// per-slot heartbeat timestamps, and stores per-slot feature flags
/// negotiated during discovery.
pub struct SessionManager {
    inner: Mutex<SessionInner>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SessionInner {
                slots: [None, None, None, None],
                pending_features: HashMap::new(),
            }),
        }
    }

    /// Stores the agreed feature flags for a client IP, to be consumed
    /// when that client's first data packet arrives on the receiver port.
    pub fn register_pending_features(&self, ip: IpAddr, features: i32) {
        self.inner.lock().unwrap().pending_features.insert(ip, features);
    }

    /// Looks up or assigns a slot for the given client address.
    ///
    /// - If the address already has a slot, returns [`SlotAssignment::Existing`].
    /// - If a free slot is available, assigns it and returns [`SlotAssignment::New`]
    ///   with the features consumed from the pending map (or 0 if none).
    /// - If all slots are full, returns [`SlotAssignment::Full`].
    pub fn get_or_assign(&self, addr: SocketAddr) -> SlotAssignment {
        let mut inner = self.inner.lock().unwrap();

        // Check if already assigned
        for (i, slot) in inner.slots.iter().enumerate() {
            if let Some(data) = slot {
                if data.addr == addr {
                    return SlotAssignment::Existing(i);
                }
            }
        }

        // Find first free slot index
        let free_index = inner.slots.iter().position(|s| s.is_none());

        if let Some(i) = free_index {
            let now = now_ms();
            let features = inner.pending_features.remove(&addr.ip()).unwrap_or(0);
            inner.slots[i] = Some(SlotData {
                addr,
                last_seen_ms: now,
                features,
            });
            info!("Player {} assigned to {}", i + 1, addr);
            return SlotAssignment::New { slot: i, features };
        }

        SlotAssignment::Full
    }

    /// Updates the heartbeat timestamp for the given slot.
    pub fn touch(&self, slot: usize) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(data) = &mut inner.slots[slot] {
            data.last_seen_ms = now_ms();
        }
    }

    /// Returns the indices of all slots that have exceeded [`TIMEOUT_MS`]
    /// since their last received packet.
    pub fn check_timeouts(&self) -> Vec<usize> {
        let inner = self.inner.lock().unwrap();
        let now = now_ms();
        let mut timed_out = Vec::new();
        for (i, slot) in inner.slots.iter().enumerate() {
            if let Some(data) = slot {
                if now.saturating_sub(data.last_seen_ms) > TIMEOUT_MS {
                    timed_out.push(i);
                }
            }
        }
        timed_out
    }

    /// Frees a slot, returning the client address that was occupying it.
    pub fn disconnect(&self, slot: usize) -> Option<SocketAddr> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(data) = inner.slots[slot].take() {
            info!("Player {} disconnected (was {})", slot + 1, data.addr);
            Some(data.addr)
        } else {
            None
        }
    }

    /// Number of currently occupied slots.
    pub fn active_count(&self) -> usize {
        self.inner.lock().unwrap().slots.iter().filter(|s| s.is_some()).count()
    }

    /// Returns `true` if all [`MAX_PLAYERS`] slots are occupied.
    pub fn is_full(&self) -> bool {
        self.active_count() >= MAX_PLAYERS
    }

    /// Returns the client address for a given slot, if occupied.
    pub fn get_addr(&self, slot: usize) -> Option<SocketAddr> {
        self.inner.lock().unwrap().slots[slot].as_ref().map(|d| d.addr)
    }

    /// Returns the agreed feature flags for a given slot (0 if empty).
    pub fn get_features(&self, slot: usize) -> i32 {
        self.inner
            .lock()
            .unwrap()
            .slots[slot]
            .as_ref()
            .map(|d| d.features)
            .unwrap_or(0)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
