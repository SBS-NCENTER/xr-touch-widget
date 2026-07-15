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
fn press(state: State<AppState>, index: usize) {
    // D16: the UI sends only the button INDEX; the engine resolves the
    // action list from its own (running) config, so a press can never fire
    // a stale action list from the UI's copy.
    let _ = state.engine_tx.send(engine::EngineCmd::Press { index });
}

#[tauri::command]
fn load_warning(state: State<AppState>) -> Option<String> {
    state.load_warning.clone()
}

/// Build the settings window, created HIDDEN. It is pre-created once during
/// `setup()` and kept alive for the app's lifetime — `open_settings` only
/// show()s it. This is deliberate: a WebView2 window created at RUNTIME (from
/// the `open_settings` command, in response to the ⚙ tap) renders blank/white
/// on Windows even though it navigates to the correct URL, whereas a window
/// created during `setup()` paints correctly. So we create it up front and
/// toggle visibility instead of create/destroy.
fn build_settings_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    // Transparent window whose rounded OPAQUE grey panel (Settings.svelte
    // .panel) provides the visible surface — the same trick the palette uses.
    // D9's "opaque/readable content" still holds: the panel is solid grey with
    // no blur/translucency. Transparency only lets the window's own corners sit
    // OUTSIDE the panel's rounded edge, so no square opaque backing peeks past
    // the CSS/objc corner radius. Requires macOSPrivateApi (tauri.conf.json).
    // Transparent on every platform so the rounded opaque panel's corners fall
    // OUTSIDE the window edge (no square backing peeks past the CSS radius). The
    // old Windows blank was the runtime-creation bug, not transparency, so
    // Windows now matches macOS here.
    let transparent = true;
    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("XRT Settings")
        .inner_size(660.0, 800.0)
        .decorations(false)
        .transparent(transparent)
        .always_on_top(true)
        .visible(false)
        .build()?;
    // Clip the contentView to the same 14px radius as the CSS panel so the
    // transparent window's corners match `border-radius: var(--radius)` (14px).
    // macOS-only (the helper is macOS-gated); a no-op elsewhere.
    #[cfg(target_os = "macos")]
    round_content_view_corners(&win, 14.0);
    // Windows: round the window via DWM so the opaque settings panel's corners
    // aren't framed by the square transparent window backing.
    #[cfg(target_os = "windows")]
    round_window_corners(&win);
    Ok(win)
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    // The settings window is pre-created (hidden) in setup(); here we just show
    // and focus it. Fall back to building it on demand if it somehow doesn't
    // exist (it always should) so the ⚙ tap is never a dead end.
    let win = match app.get_webview_window("settings") {
        Some(win) => win,
        None => match build_settings_window(&app) {
            Ok(win) => win,
            Err(e) => {
                eprintln!("failed to build settings window: {e}");
                return;
            }
        },
    };
    let _ = win.show();
    let _ = win.set_focus();
    // Ask the settings UI to reload its draft from the current saved config, so
    // reopening never shows a stale/discarded edit from a previous session
    // (the webview persists across hide/show, unlike the old create/close).
    let _ = app.emit_to("settings", "xrt://settings-shown", ());
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            press,
            load_warning,
            open_settings,
            quit_app
        ])
        .setup(|app| {
            let win = app.get_webview_window("palette").expect("palette window exists");
            apply_glass(&win);

            // Resolve the DATA BASE dir, then place config.toml under it. The
            // mode is chosen at COMPILE time: a PORTABLE build
            // (`--features portable`) keeps config in the exe's own folder so
            // the whole folder is copy-portable; the default (installed) build
            // keeps it in the per-user OS config dir. Portable and installed are
            // separate binaries. Neither branch panics (§8 — the app must come
            // up); both fall back to a temp dir if their primary lookup fails.
            let base_dir = if cfg!(feature = "portable") {
                match std::env::current_exe() {
                    Ok(exe) => exe
                        .parent()
                        .map(|d| d.to_path_buf())
                        .unwrap_or_else(|| std::env::temp_dir().join("xr-touch-to-osc")),
                    Err(e) => {
                        eprintln!("failed to resolve current exe, using temp fallback: {e}");
                        std::env::temp_dir().join("xr-touch-to-osc")
                    }
                }
            } else {
                match app.path().app_config_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!("failed to resolve app config dir, using temp fallback: {e}");
                        std::env::temp_dir().join("xr-touch-to-osc")
                    }
                }
            };
            let config_path = base_dir.join("config.toml");
            let (config, outcome) = config::load(&config_path);
            let mut load_warning = match outcome {
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

            // Bind the OSC socket WITHOUT panicking on a taken port (§8 — the
            // app must come up). If listen_port is already in use (2nd launch /
            // stale instance), fall back to an ephemeral port so the trigger
            // send-path stays alive. Pong reception then degrades — pongs sent
            // to the real listen_port won't arrive here — but heartbeat is
            // display-only, so this is acceptable graceful degradation.
            let socket = match OscSocket::bind(config.network.listen_port) {
                Ok(s) => s,
                Err(_) => {
                    let s = OscSocket::bind(0).map_err(|e| {
                        // Even an ephemeral bind failed (OS out of sockets/FDs).
                        // Without any socket the engine can't run at all, so this
                        // is the one case we surface as a hard error.
                        format!(
                            "failed to bind any OSC socket (listen port {} and ephemeral both failed): {e}",
                            config.network.listen_port
                        )
                    })?;
                    let bind_warning = format!(
                        "pong 포트 {} 사용 중 — heartbeat 표시 부정확 (트리거는 정상)",
                        config.network.listen_port
                    );
                    // Do NOT clobber a config-parse warning if one is already set
                    // — combine both so neither is lost.
                    load_warning = Some(match load_warning {
                        Some(existing) => format!("{existing} / {bind_warning}"),
                        None => bind_warning,
                    });
                    s
                }
            };
            let engine_tx = engine::spawn(app.handle().clone(), config.clone(), socket);

            app.manage(AppState {
                config_path,
                config: Mutex::new(config),
                engine_tx,
                load_warning,
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Pre-create the hidden settings window AFTER the app is ready, not
            // during setup(): setup() blocks the palette's first paint, so
            // building the second webview there delayed startup. Creating it on
            // the first Ready event lets the palette paint first, then warms the
            // settings webview in the background. Still created up front (before
            // any ⚙ tap), so it renders correctly — a settings window created
            // on-demand from the open_settings command paints blank on Windows.
            if let tauri::RunEvent::Ready = event {
                if app_handle.get_webview_window("settings").is_none() {
                    if let Err(e) = build_settings_window(app_handle) {
                        eprintln!("failed to pre-create settings window: {e}");
                    }
                }
            }
        });
}

/// OS-level behind-window blur (spec §7). CSS cannot blur what is behind
/// the window — only the compositor can.
fn apply_glass(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        // OS-level acrylic behind-blur (spec §7), the Windows counterpart to
        // macOS vibrancy. The earlier "transparent WebView2 renders blank on
        // Win11" diagnosis was wrong — that blank was the runtime settings
        // window (now pre-created in setup, see build_settings_window). With a
        // transparent window created up front, acrylic composites correctly.
        // The tint is a low-alpha dark so the configurable CSS --glass-bg
        // (appearance.bg_opacity) still controls the visible darkness on top.
        use window_vibrancy::apply_acrylic;
        if let Err(e) = apply_acrylic(win, Some((18, 22, 28, 120))) {
            eprintln!("acrylic failed (falling back to plain transparency): {e}");
        }
        // Round the whole window (incl. the acrylic backing) so its square
        // corners don't poke out around the CSS glass panel — the Windows
        // counterpart to macOS's round_content_view_corners.
        round_window_corners(win);
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

/// Round a Windows window's corners via DWM (Win11). Clips the whole composited
/// window — including the acrylic backing — to the OS rounded-corner radius, so
/// the square window corners don't show around the rounded CSS glass panel.
/// Auto-tracks size changes (unlike a SetWindowRgn region), so it survives
/// edit-mode resizes. macOS uses round_content_view_corners for the same goal.
#[cfg(target_os = "windows")]
fn round_window_corners(win: &tauri::WebviewWindow) {
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attr: u32,
            pv: *const core::ffi::c_void,
            cb: u32,
        ) -> i32;
    }
    // DWMWA_WINDOW_CORNER_PREFERENCE = 33 ; DWMWCP_ROUND = 2 (dwmapi.h).
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    match win.hwnd() {
        Ok(hwnd) => {
            let pref = DWMWCP_ROUND;
            unsafe {
                DwmSetWindowAttribute(
                    hwnd.0 as isize,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const u32 as *const core::ffi::c_void,
                    core::mem::size_of::<u32>() as u32,
                );
            }
        }
        Err(e) => eprintln!("failed to get hwnd for corner rounding: {e}"),
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
