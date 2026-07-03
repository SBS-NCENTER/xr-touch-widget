#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let win = app.get_webview_window("palette").expect("palette window exists");
            apply_glass(&win);
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
