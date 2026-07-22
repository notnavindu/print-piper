use crate::state::AppState;
use serde::Serialize;
use std::collections::VecDeque;
use tauri::{AppHandle, Emitter, Manager};

pub const RING_CAPACITY: usize = 2000;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub seq: u64,
    pub ts: String,
    pub level: String, // "debug" | "info" | "warn" | "error"
    pub area: String,  // "capture" | "discovery" | "dispatch" | "store" | "system"
    pub msg: String,
    pub detail: serde_json::Value,
}

#[derive(Default)]
pub struct LogRing {
    entries: VecDeque<LogEntry>,
    next_seq: u64,
}

impl LogRing {
    pub fn push(&mut self, level: &str, area: &str, msg: String, detail: serde_json::Value) -> LogEntry {
        self.next_seq += 1;
        let entry = LogEntry {
            seq: self.next_seq,
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: level.into(),
            area: area.into(),
            msg,
            detail,
        };
        if self.entries.len() >= RING_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.clone());
        entry
    }

    pub fn list_after(&self, after_seq: u64) -> Vec<LogEntry> {
        self.entries.iter().filter(|e| e.seq > after_seq).cloned().collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Log to the ring (→ UI), the `log` crate (→ file via tauri-plugin-log), and emit live.
pub fn plog(app: &AppHandle, level: &str, area: &str, msg: impl Into<String>, detail: serde_json::Value) {
    let msg = msg.into();
    match level {
        "error" => log::error!(target: "piper", "[{area}] {msg}"),
        "warn" => log::warn!(target: "piper", "[{area}] {msg}"),
        "debug" => log::debug!(target: "piper", "[{area}] {msg}"),
        _ => log::info!(target: "piper", "[{area}] {msg}"),
    }
    let state = app.state::<AppState>();
    let entry = state.logs.lock().unwrap().push(level, area, msg, detail);
    let _ = app.emit("log:appended", &entry);
}
