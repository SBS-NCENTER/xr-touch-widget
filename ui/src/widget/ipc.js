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
  buttons: [
    { label: '그래픽 A', graphic_id: 'graphic_a', type: 'trigger' },
    { label: '그래픽 B', graphic_id: 'graphic_b', type: 'trigger' },
    { label: 'CLEAR', graphic_id: 'clear_all', type: 'trigger' },
  ],
  // Mirrors crates/core/src/config.rs AppearanceConfig/WindowConfig spec
  // defaults (D8/D9) so the appearance-application code path in
  // Palette.svelte runs identically in the browser harness and in Tauri.
  appearance: { bg_opacity: 0.55, button_opacity: 0.07, accent: '#4da3ff', bg_tint: '#141820' },
  window: { width: 720, height: 96 },
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

export async function trigger(graphicId) {
  if (!inTauri) return console.log('[mock] trigger', graphicId);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('trigger', { graphicId });
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

/** Current window content size in logical (CSS) pixels, or null outside Tauri. */
export async function innerSize() {
  if (!inTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const win = getCurrentWindow();
  const [physical, scaleFactor] = await Promise.all([win.innerSize(), win.scaleFactor()]);
  const logical = physical.toLogical(scaleFactor);
  return { width: Math.round(logical.width), height: Math.round(logical.height) };
}
