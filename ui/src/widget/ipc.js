// Single gateway to Tauri. In a plain browser (dev harness, LAN preview)
// there is no Tauri runtime, so every function falls back to a mock —
// the UI must never crash outside Tauri.
const inTauri = '__TAURI_INTERNALS__' in window;

const mockConfig = {
  network: { ue_port: 8000, listen_port: 8001, heartbeat_interval_ms: 1000, heartbeat_timeout_misses: 3 },
  targets: [
    { name: 'XR-1', ip: '192.168.0.10', active: true },
    { name: 'XR-2', ip: '192.168.0.11', active: false },
  ],
  // D16: each button is an ORDERED action list — osc (D14 message spec) or
  // http (full URL, GET). Mirrors crates/core/src/config.rs ButtonDef/Action
  // so the browser harness/preview matches production.
  buttons: [
    {
      label: 'CAM 1',
      actions: [
        { type: 'http', url: 'http://10.10.204.184:16208/gateway/25.2.4/publish?Type=Call&Target=Store&Method=SetCameraSet&ParamNumber=0' },
      ],
    },
    {
      label: '그래픽 A',
      actions: [{ type: 'osc', address: '/xrt/graphic', value: 'graphic_a', value_type: 'string' }],
    },
    {
      label: 'CLEAR',
      actions: [{ type: 'osc', address: '/xrt/graphic', value: 'clear_all', value_type: 'string' }],
    },
  ],
  // Mirrors crates/core/src/config.rs AppearanceConfig/WindowConfig spec
  // defaults (D8/D9) so the appearance-application code path in
  // Palette.svelte runs identically in the browser harness and in Tauri.
  appearance: { bg_opacity: 0.55, button_opacity: 0.07, accent: '#4da3ff', bg_tint: '#141820', highlight_last: false, highlight_color: '#4da3ff', highlight_opacity: 1.0 },
  window: { width: 240, height: 400 },
  // Mirrors crates/core/src/config.rs LayoutConfig spec defaults (Task 8b:
  // vertical-by-default — a single button column).
  layout: { horizontal: false, vertical: true, cols: 3, rows: 2 },
};

export async function getConfig() {
  if (!inTauri) return structuredClone(mockConfig);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('get_config');
}

export async function saveConfig(config) {
  if (!inTauri) return console.log('[mock] saveConfig', config);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('save_config', { config });
}

export async function press(index) {
  if (!inTauri) return console.log('[mock] press', index);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('press', { index });
}

/** cb receives {button_index, detail} when any action of a press fails
 *  (OSC send error or HTTP failure) — returns unlisten fn. Drives the
 *  palette's 1.5s red flash (D16). */
export async function onPressError(cb) {
  if (!inTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen('xrt://press-error', (e) => cb(e.payload));
}

export async function openSettings() {
  if (!inTauri) return console.log('[mock] openSettings');
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('open_settings');
}

export async function loadWarning() {
  if (!inTauri) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('load_warning');
}

/** cb receives [{name, ip, active, status}] — returns unlisten fn */
export async function onStatus(cb) {
  if (!inTauri) {
    const id = setInterval(
      () => cb(mockConfig.targets.map((t, i) => ({ ...t, status: i === 0 ? 'Connected' : 'Lost' }))),
      1000,
    );
    return () => clearInterval(id);
  }
  const { listen } = await import('@tauri-apps/api/event');
  return listen('xrt://status', (e) => cb(e.payload));
}

/** fires when settings saved a new config — returns unlisten fn */
export async function onConfigChanged(cb) {
  if (!inTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen('xrt://config-changed', (e) => cb(e.payload));
}

// --- Edit-mode window controls (D8, 2026-07-03) ---
// Outside Tauri there is no real OS window to resize, so these are silent
// (console-logged) no-ops — the edit-mode UI still renders in the browser
// harness, it just can't move real window chrome that doesn't exist there.
//
// Resize is done by programmatic setSize driven from grip pointer deltas —
// NOT the OS-native resize session (startResizeDragging), which is
// unreliable for undecorated windows on macOS. setSize works without the
// window ever being resizable, so the window stays resizable=false forever
// (stronger mis-touch protection) and no setResizable call exists.

/** Set the window content size in logical (CSS) pixels. */
export async function setSize(width, height) {
  if (!inTauri) return console.log('[mock] setSize', width, height);
  const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
  return getCurrentWindow().setSize(new LogicalSize(width, height));
}

/** Fires whenever the OS window actually MOVES (native drag). Returns the
 *  unlisten fn (browser/no-Tauri fallback: a no-op unlisten, like the other
 *  wrappers). The palette uses this to cancel an armed edit-mode long-press
 *  when a window-drag starts: once the OS drag session begins the webview
 *  stops receiving pointermove, so the pointer-based MOVE_CANCEL_PX guard
 *  can't see it — actual window movement is the reliable cancel signal. */
export async function onWindowMoved(cb) {
  if (!inTauri) return () => {};
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow().onMoved(cb);
}

/** Current window content size in logical (CSS) pixels, or null outside Tauri. */
export async function innerSize() {
  if (!inTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const win = getCurrentWindow();
  const [physical, scaleFactor] = await Promise.all([win.innerSize(), win.scaleFactor()]);
  const logical = physical.toLogical(scaleFactor);
  return { width: Math.round(logical.width), height: Math.round(logical.height) };
}

// --- Settings live preview (D10, 2026-07-03) ---
// The settings window edits a local draft and broadcasts it, non-persistently,
// so the palette can reflect appearance/layout/window changes before the
// operator commits to [적용]. Payload shape: { appearance, layout, window }.

/** cb receives {appearance, layout, window} — returns unlisten fn */
export async function onAppearancePreview(cb) {
  if (!inTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen('xrt://appearance-preview', (e) => cb(e.payload));
}

/** Broadcast a non-persistent preview payload from the settings window. */
export async function emitAppearancePreview(payload) {
  if (!inTauri) return console.log('[mock] emitAppearancePreview', payload);
  const { emit } = await import('@tauri-apps/api/event');
  return emit('xrt://appearance-preview', payload);
}

/** Exit the whole application (settings window's [프로그램 종료]). */
export async function quit() {
  if (!inTauri) return console.log('[mock] quit');
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('quit_app');
}

/** Hide the current (settings) window without saving — used by [뒤로가기].
 *  The settings window is PERSISTENT (pre-created hidden at startup and only
 *  ever shown/hidden — a runtime-created WebView2 window renders blank on
 *  Windows), so [뒤로가기] hides it rather than closing/destroying it. */
export async function hideWindow() {
  if (!inTauri) return console.log('[mock] hideWindow');
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow().hide();
}

/** Fires when the settings window is (re-)shown via ⚙ — Settings.svelte uses
 *  it to reload its draft from the current saved config, since the persistent
 *  webview isn't remounted on reopen. Returns the unlisten fn. */
export async function onSettingsShown(cb) {
  if (!inTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen('xrt://settings-shown', () => cb());
}
