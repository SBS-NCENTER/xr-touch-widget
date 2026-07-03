#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod engine;

use std::path::PathBuf;
use std::sync::{mpsc::Sender, Mutex};

use tauri::{AppHandle, LogicalSize, Manager, State, WebviewUrl, WebviewWindowBuilder};
use xrt_core::config::{self, Config, LoadOutcome};
use xrt_core::net::OscSocket;

struct AppState {
    config_path: PathBuf,
    config: Mutex<Config>,
    engine_tx: Sender<engine::EngineCmd>,
    load_warning: Option<String>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(state: State<AppState>, config: Config) -> Result<(), String> {
    config::save(&state.config_path, &config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config.clone();
    let _ = state.engine_tx.send(engine::EngineCmd::UpdateConfig(config));
    Ok(())
}

#[tauri::command]
fn trigger(state: State<AppState>, graphic_id: String) {
    let _ = state.engine_tx.send(engine::EngineCmd::Trigger(graphic_id));
}

#[tauri::command]
fn load_warning(state: State<AppState>) -> Option<String> {
    state.load_warning.clone()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        return;
    }
    // Settings window is opaque by design (D9, 2026-07-03) — readability first,
    // so no transparent flag and no vibrancy here.
    let builder = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
        .title("XRT Settings")
        .inner_size(460.0, 620.0)
        .decorations(false)
        .always_on_top(true);
    match builder.build() {
        Ok(_) => {}
        Err(e) => eprintln!("failed to open settings window: {e}"),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            trigger,
            load_warning,
            open_settings
        ])
        .setup(|app| {
            let win = app.get_webview_window("palette").expect("palette window exists");
            apply_glass(&win);

            let config_path = app
                .path()
                .app_config_dir()
                .expect("app config dir resolvable")
                .join("config.toml");
            let (config, outcome) = config::load(&config_path);
            let load_warning = match outcome {
                LoadOutcome::Loaded => None,
                LoadOutcome::MissingUsedDefault => None, // first run is not an error
                LoadOutcome::ParseErrorUsedDefault(e) => {
                    Some(format!("config.toml is broken, started with defaults: {e}"))
                }
            };

            // Re-apply the size the operator left the palette at during a
            // previous edit-mode session (D8). Called from Rust, so it
            // needs no ACL entry — only the JS-invoked edit-mode resize
            // calls (setResizable/startResizeDragging) do.
            let size = LogicalSize::new(config.window.width as f64, config.window.height as f64);
            if let Err(e) = win.set_size(size) {
                eprintln!("failed to apply saved window size: {e}");
            }

            let socket = OscSocket::bind(config.network.listen_port)
                .expect("failed to bind OSC listen port");
            let engine_tx = engine::spawn(app.handle().clone(), config.clone(), socket);

            app.manage(AppState {
                config_path,
                config: Mutex::new(config),
                engine_tx,
                load_warning,
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// OS-level behind-window blur (spec §7). CSS cannot blur what is behind
/// the window — only the compositor can.
fn apply_glass(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        // Acrylic works on Win10 and Win11. Mica (Win11-only) is an
        // alternative to evaluate during final on-device tuning.
        if let Err(e) = window_vibrancy::apply_acrylic(win, Some((20, 24, 32, 120))) {
            eprintln!("acrylic failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
        if let Err(e) = apply_vibrancy(win, NSVisualEffectMaterial::HudWindow, None, Some(14.0)) {
            eprintln!("vibrancy failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Dev fallback: plain transparency, no blur (spec §7).
        let _ = win;
    }
}
