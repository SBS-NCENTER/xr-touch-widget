use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use xrt_core::config::{Config, Target};
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;

pub enum EngineCmd {
    Trigger(String),
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

/// Owns the socket and all timing. UI talks to it via EngineCmd only.
pub fn spawn(app: AppHandle, mut config: Config, socket: OscSocket) -> Sender<EngineCmd> {
    let (tx, rx): (Sender<EngineCmd>, Receiver<EngineCmd>) = mpsc::channel();
    std::thread::spawn(move || {
        let mut hb = Heartbeat::new(config.network.heartbeat_timeout_misses);
        let mut last_tick = Instant::now();
        loop {
            // 1) handle a pending command (50ms poll keeps loop responsive)
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(EngineCmd::Trigger(id)) => {
                    for report in socket.send_trigger(&id, &config.targets, config.network.ue_port) {
                        if !report.ok {
                            let ip = &report.ip;
                            let e = report.error.as_deref().unwrap_or("unknown error");
                            eprintln!("OSC send to {ip} failed: {e}");
                            hb.mark_lost(&report.ip);
                        }
                    }
                }
                Ok(EngineCmd::UpdateConfig(new_config)) => {
                    hb = Heartbeat::new(new_config.network.heartbeat_timeout_misses);
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
                for report in socket.send_ping_all(&active, config.network.ue_port) {
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
