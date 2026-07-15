use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use xrt_core::config::{Action, Config, Target};
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::http;
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;

pub enum EngineCmd {
    /// One button press (D16). The engine resolves `buttons[index]` from ITS
    /// copy of the config — the running truth — so a press can never fire a
    /// stale action list from the UI's copy. Actions fire in list order:
    /// Osc inline (UDP is effectively instant), Http on a spawned thread so
    /// a dead gateway's timeout can never stall this loop (heartbeat
    /// included). Responses are NOT awaited between actions.
    Press { index: usize },
    UpdateConfig(Config),
}

#[derive(Serialize, Clone)]
pub struct StatusEntry {
    pub name: String,
    pub ip: String,
    pub active: bool,
    pub status: LinkStatus,
}

pub const STATUS_EVENT: &str = "xrt://status";

/// Per-press failure feed for the palette's red flash (D16). Emitted for
/// EVERY failed action of a press — OSC send errors and HTTP failures alike.
pub const PRESS_ERROR_EVENT: &str = "xrt://press-error";

/// How long an HTTP action waits (connect + response) before reporting
/// failure. Constant by design (YAGNI — spec §5).
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize, Clone)]
pub struct PressError {
    pub button_index: usize,
    pub detail: String,
}

/// Owns the socket and all timing. UI talks to it via EngineCmd only.
pub fn spawn(app: AppHandle, mut config: Config, socket: OscSocket) -> Sender<EngineCmd> {
    let (tx, rx): (Sender<EngineCmd>, Receiver<EngineCmd>) = mpsc::channel();
    std::thread::spawn(move || {
        let mut hb = Heartbeat::new(config.network.heartbeat_timeout_misses);
        let mut last_tick = Instant::now();
        loop {
            // 1) handle a pending command (50ms poll keeps loop responsive)
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(EngineCmd::Press { index }) => {
                    if let Some(btn) = config.buttons.get(index) {
                        for action in btn.actions.clone() {
                            match action {
                                Action::Osc { address, value_type, value } => {
                                    for report in socket.send_trigger(
                                        &address,
                                        value_type,
                                        &value,
                                        &config.targets,
                                        config.network.ue_port,
                                    ) {
                                        if !report.ok {
                                            let ip = &report.ip;
                                            let e = report.error.as_deref().unwrap_or("unknown error");
                                            eprintln!("OSC send to {ip} failed: {e}");
                                            hb.mark_lost(&report.ip);
                                            // D16: a failed send now also flashes
                                            // the pressed button red, not just logs.
                                            let _ = app.emit(
                                                PRESS_ERROR_EVENT,
                                                &PressError {
                                                    button_index: index,
                                                    detail: format!("OSC → {ip}: {e}"),
                                                },
                                            );
                                        }
                                    }
                                }
                                Action::Http { url } => {
                                    // Fire on a fresh thread: blocking here would
                                    // stall heartbeat + subsequent presses for up
                                    // to HTTP_TIMEOUT per dead gateway. Presses
                                    // are operator-paced, so thread cost is nil.
                                    let app = app.clone();
                                    std::thread::spawn(move || {
                                        if let Err(e) = http::get(&url, HTTP_TIMEOUT) {
                                            eprintln!("HTTP GET {url} failed: {e}");
                                            let _ = app.emit(
                                                PRESS_ERROR_EVENT,
                                                &PressError {
                                                    button_index: index,
                                                    detail: format!("HTTP: {e}"),
                                                },
                                            );
                                        }
                                    });
                                }
                            }
                        }
                    } else {
                        // Stale UI (button list changed mid-press) — drop it.
                        eprintln!(
                            "press: button index {index} out of range ({} buttons)",
                            config.buttons.len()
                        );
                    }
                }
                Ok(EngineCmd::UpdateConfig(new_config)) => {
                    // Only rebuild the Heartbeat when the timeout threshold
                    // actually changed. Rebuilding wipes all link state, so an
                    // unconditional rebuild flickers every active status dot to
                    // grey for ~1s on EVERY [적용] (even appearance-only edits).
                    // Its on_tick retain already reconciles added/removed target
                    // IPs, so keeping the existing hb leaves established targets'
                    // status stable across applies.
                    if new_config.network.heartbeat_timeout_misses
                        != config.network.heartbeat_timeout_misses
                    {
                        hb = Heartbeat::new(new_config.network.heartbeat_timeout_misses);
                    }
                    config = new_config;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }

            // 2) drain incoming pongs
            while let Some((incoming, from)) = socket.try_recv() {
                if matches!(incoming, Incoming::Pong) {
                    hb.on_pong(&from.ip().to_string());
                }
            }

            // 3) heartbeat tick when interval elapsed
            let interval = Duration::from_millis(config.network.heartbeat_interval_ms);
            if last_tick.elapsed() >= interval {
                last_tick = Instant::now();
                // Heartbeat policy (D13): ping ONLY active targets. Inactive
                // targets are never contacted, so they carry no live status and
                // on_tick forgets any ip not in this list (an active→inactive
                // flip drops out of tracking, so it can't report a stale state).
                let active: Vec<Target> =
                    config.targets.iter().filter(|t| t.active).cloned().collect();
                let ips: Vec<String> = active.iter().map(|t| t.ip.clone()).collect();
                for report in socket.send_ping(&active, config.network.ue_port) {
                    if !report.ok {
                        let ip = &report.ip;
                        let e = report.error.as_deref().unwrap_or("unknown error");
                        eprintln!("OSC send to {ip} failed: {e}");
                        hb.mark_lost(&report.ip);
                    }
                }
                hb.on_tick(&ips);
                // Still emit an entry per target so the palette shows a dot for
                // each (inactive = empty). Inactive targets read Unknown — never
                // pinged, so their heartbeat status must not surface as stale.
                let payload: Vec<StatusEntry> = config
                    .targets
                    .iter()
                    .map(|t| StatusEntry {
                        name: t.name.clone(),
                        ip: t.ip.clone(),
                        active: t.active,
                        status: if t.active {
                            hb.status(&t.ip)
                        } else {
                            LinkStatus::Unknown
                        },
                    })
                    .collect();
                let _ = app.emit(STATUS_EVENT, &payload);
            }
        }
    });
    tx
}
