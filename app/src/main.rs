#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod engine;

use std::path::PathBuf;
use std::sync::{mpsc::Sender, Mutex};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WebviewUrl, WebviewWindowBuilder};
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
fn save_config(app: AppHandle, state: State<AppState>, config: Config) -> Result<(), String> {
    config::save(&state.config_path, &config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config.clone();
    let _ = state.engine_tx.send(engine::EngineCmd::UpdateConfig(config.clone()));
    // Settings (Task 9) needs the palette to re-apply the FULL saved config
    // (buttons, appearance, layout, window) on every [적용] — Palette.svelte
    // already subscribes to this event (Task 8).
    let _ = app.emit("xrt://config-changed", &config);
    Ok(())
}

/// Whole-app exit, invoked from the settings window's [프로그램 종료] button
/// (Task 9, D10). No confirmation dialog — the operator already discards or
/// applies unsaved changes via [적용]/[뒤로가기] before reaching for this.
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
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
        .inner_size(660.0, 800.0)
        .decorations(false)
        .always_on_top(true);
    match builder.build() {
        Ok(win) => {
            // Borderless opaque window → same square-corner issue the palette
            // has. Reuse the palette's contentView corner rounding so the
            // window corners match the CSS `border-radius: var(--radius)`
            // (14px) on the settings panel. macOS-only (the helper is
            // macOS-gated); a no-op elsewhere.
            #[cfg(target_os = "macos")]
            round_content_view_corners(&win, 14.0);
            #[cfg(not(target_os = "macos"))]
            let _ = win;
        }
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
            open_settings,
            quit_app
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

        // window-vibrancy's `radius` arg above only rounds the private
        // NSVisualEffectView it inserts BEHIND the webview (window-vibrancy
        // 0.6.0, src/macos/vibrancy.rs: `blurred_view.setCornerRadius(...)`
        // is called on that inserted subview only). It never touches the
        // window's own contentView, which stays a plain rectangular,
        // unclipped NSView. A normal titled NSWindow gets its rendered
        // corners auto-rounded by AppKit, but this palette is borderless
        // (tauri.conf.json: decorations: false, shadow: false), so that
        // auto-rounding never applies — the contentView's square edge is
        // what shows through as a faint rectangular sliver at the 4 corners
        // around the CSS-rounded glass panel.
        //
        // Fix: explicitly clip the contentView's own CALayer to a rounded
        // rect matching the CSS `--radius` (ui/src/shared/tokens.css) — keep
        // these two values in sync.
        round_content_view_corners(win, 14.0);
    }
    #[cfg(target_os = "linux")]
    {
        // Dev fallback: plain transparency, no blur (spec §7).
        let _ = win;
    }
}

/// Clips the macOS window's contentView layer to rounded corners so the
/// vibrancy blur (which sits BEHIND the transparent webview — see
/// `apply_glass` above) is masked to the same rounded shape as the CSS
/// glass panel, instead of bleeding out to the window's square edge.
///
/// Uses `objc2` + `objc2-app-kit` directly (already resolved in Cargo.lock
/// at these exact versions via tauri/wry/window-vibrancy) rather than the
/// typed `NSView::layer()` accessor, so we don't need to also pull in
/// `objc2-quartz-core` just to name the `CALayer` type: the two selectors
/// we need (`setCornerRadius:`, `setMasksToBounds:`) are sent dynamically
/// through `AnyObject` either way.
#[cfg(target_os = "macos")]
fn round_content_view_corners(win: &tauri::WebviewWindow, radius: f64) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::msg_send;
    use objc2_app_kit::NSWindow;
    use std::ptr::NonNull;

    let ns_window_ptr = match win.ns_window() {
        Ok(ptr) => ptr,
        Err(e) => {
            eprintln!("failed to get ns_window for corner rounding: {e}");
            return;
        }
    };
    let Some(ns_window_ptr) = NonNull::new(ns_window_ptr.cast::<NSWindow>()) else {
        eprintln!("ns_window pointer was null, skipping corner rounding");
        return;
    };
    // Safety: `ns_window_ptr` is a live NSWindow* handed to us by Tauri for
    // this exact window (same cast-raw-handle-to-typed-reference pattern
    // window-vibrancy itself uses for `&NSView` in apply_vibrancy).
    let ns_window: &NSWindow = unsafe { ns_window_ptr.as_ref() };

    let Some(content_view) = ns_window.contentView() else {
        eprintln!("window has no contentView, skipping corner rounding");
        return;
    };

    // Ensure the view is layer-backed. Idempotent/harmless if it already is
    // (wry's WKWebView-hosting content view normally already is).
    content_view.setWantsLayer(true);

    // SAFETY: `layer` and `setCornerRadius:`/`setMasksToBounds:` are all
    // standard, always-available AppKit/Core Animation selectors on the
    // NSView/CALayer objects we hold live references to.
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![&content_view, layer] };
    let Some(layer) = layer else {
        eprintln!("contentView has no backing layer, skipping corner rounding");
        return;
    };

    unsafe {
        let _: () = msg_send![&layer, setCornerRadius: radius];
        let _: () = msg_send![&layer, setMasksToBounds: true];
    }
}
