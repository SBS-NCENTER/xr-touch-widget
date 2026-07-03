<script>
  import GlassPanel from '../shared/GlassPanel.svelte';
  import {
    getConfig,
    saveConfig,
    trigger,
    openSettings,
    loadWarning,
    onStatus,
    onConfigChanged,
    setSize,
    innerSize,
  } from './ipc.js';

  const LONG_PRESS_MS = 600;
  // Movement past this many px cancels the long-press. Chosen to sit above
  // ordinary finger/pointer jitter (a few px) but well below any deliberate
  // drag gesture, so a window-drag-in-progress reliably cancels edit mode
  // instead of letting it fire mid-broadcast (finding 1).
  const MOVE_CANCEL_PX = 8;
  // Smallest useful palette: one handle + a couple of buttons still fit.
  const RESIZE_MIN_W = 240;
  const RESIZE_MIN_H = 64;

  let buttons = $state([]);
  let statuses = $state([]);
  let warning = $state(null);
  let flashId = $state(null);
  let editMode = $state(false);

  // Full config kept around outside Svelte state (edit-mode exit needs to
  // patch just `window` and save the whole object back — it never drives
  // a render by itself).
  let latestConfig = null;
  let longPressTimer = null;
  let longPressStartX = 0;
  let longPressStartY = 0;

  // Manual grip-resize session state — plain (non-reactive) lets, nothing
  // here drives a render. `resizing` is null outside an active drag.
  let resizing = null;
  let resizeRafId = null;
  let pendingW = 0;
  let pendingH = 0;

  function hexToRgb(hex) {
    const n = parseInt(hex.replace('#', ''), 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }

  /** Reflect config.appearance onto CSS custom properties (D9). Same code
   *  path runs in Tauri and in the browser harness — mock config carries
   *  the same shape as the real one. */
  function applyAppearance(appearance) {
    if (!appearance) return;
    const { r, g, b } = hexToRgb(appearance.bg_tint);
    const root = document.documentElement.style;
    root.setProperty('--glass-bg', `rgba(${r}, ${g}, ${b}, ${appearance.bg_opacity})`);
    root.setProperty('--accent', appearance.accent);
    root.setProperty('--btn-fill', `rgba(255, 255, 255, ${appearance.button_opacity})`);
  }

  function applyConfig(config) {
    latestConfig = config;
    buttons = config.buttons;
    statuses = config.targets.map((t) => ({ ...t, status: 'Unknown' }));
    applyAppearance(config.appearance);
  }

  $effect(() => {
    let unsubs = [];
    (async () => {
      const config = await getConfig();
      applyConfig(config);
      warning = await loadWarning();
      unsubs.push(await onStatus((list) => (statuses = list)));
      unsubs.push(await onConfigChanged((config) => applyConfig(config)));
    })();
    return () => unsubs.forEach((u) => u());
  });

  async function press(btn) {
    flashId = btn.graphic_id;
    setTimeout(() => (flashId = null), 250);
    await trigger(btn.graphic_id);
  }

  function dotClass(s) {
    if (s.status === 'Lost') return 'lost';
    return s.active ? 'active' : 'inactive';
  }

  // --- Edit mode (D8): long-press the handle toggles it. The window is
  // NEVER OS-resizable — resize happens only via programmatic setSize from
  // the grips, and the grips only exist in edit mode, so a live-broadcast
  // mis-touch can't reshape the palette. ---
  function handlePointerDown(event) {
    // Guard against a leaked timer from a prior (e.g. multi-touch double-fire)
    // pointerdown that never got a matching cancel (finding 2).
    cancelLongPress();
    longPressStartX = event.clientX;
    longPressStartY = event.clientY;
    longPressTimer = setTimeout(() => {
      longPressTimer = null;
      toggleEditMode();
    }, LONG_PRESS_MS);
  }

  // Cancels the long-press once the pointer has moved past the threshold.
  // Tauri's native drag region commonly stops delivering pointer events to
  // the webview once an OS drag session starts, so pointerup/pointercancel
  // can never be relied on to arrive — a movement threshold checked on
  // whatever pointermove events do land is the robust signal that this is
  // a drag, not a long-press-in-place (finding 1).
  function handlePointerMove(event) {
    if (!longPressTimer) return;
    const dx = event.clientX - longPressStartX;
    const dy = event.clientY - longPressStartY;
    if (Math.hypot(dx, dy) > MOVE_CANCEL_PX) {
      cancelLongPress();
    }
  }

  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  }

  async function toggleEditMode() {
    if (editMode) {
      // Exiting edit mode just hides the grips (no OS window flag to undo —
      // the window is never resizable) and persists the final size.
      editMode = false;
      const size = await innerSize();
      if (size && latestConfig) {
        const config = { ...latestConfig, window: { width: size.width, height: size.height } };
        latestConfig = config;
        await saveConfig(config);
      }
    } else {
      editMode = true;
    }
  }

  // --- Manual grip resize. The OS-native resize session
  // (startResizeDragging) is unreliable for undecorated windows on macOS,
  // so the grips drive programmatic setSize from captured pointer deltas —
  // identical behavior on macOS and Windows. clientX/Y are CSS (logical)
  // px, same unit as innerSize()/setSize(), and the window's top-left stays
  // anchored during setSize so client coords remain a stable reference. ---
  async function startResize(event, corner) {
    // Capture synchronously (currentTarget is only valid during dispatch);
    // moves/ups are then retargeted to the grip even outside the window.
    event.currentTarget.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startY = event.clientY;
    const start = await innerSize();
    if (!start) return; // browser harness: no real window to resize
    resizing = { corner, startX, startY, startW: start.width, startH: start.height };
  }

  function moveResize(event) {
    if (!resizing) return;
    const dx = event.clientX - resizing.startX;
    const dy = event.clientY - resizing.startY;
    // Direction-aware: a west grip grows width when dragging left, etc.
    const west = resizing.corner.includes('w');
    const north = resizing.corner.includes('n');
    pendingW = Math.max(RESIZE_MIN_W, Math.round(resizing.startW + (west ? -dx : dx)));
    pendingH = Math.max(RESIZE_MIN_H, Math.round(resizing.startH + (north ? -dy : dy)));
    // Throttle the IPC to one setSize per frame — never await per move.
    if (resizeRafId === null) {
      resizeRafId = requestAnimationFrame(() => {
        resizeRafId = null;
        setSize(pendingW, pendingH);
      });
    }
  }

  function endResize(event) {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (resizeRafId !== null) {
      cancelAnimationFrame(resizeRafId);
      resizeRafId = null;
      if (resizing) setSize(pendingW, pendingH); // flush the last pending frame
    }
    resizing = null;
  }
</script>

<div class="palette-root" class:editing={editMode}>
  <GlassPanel>
    <div class="row">
      <div
        class="handle"
        data-tauri-drag-region
        role="button"
        tabindex="0"
        aria-label="드래그로 이동, 길게 눌러 편집 모드"
        onpointerdown={handlePointerDown}
        onpointermove={handlePointerMove}
        onpointerup={cancelLongPress}
        onpointercancel={cancelLongPress}
        onpointerleave={cancelLongPress}
      >☰</div>
      <div class="dots" title="active targets">
        {#each statuses as s (s.ip)}
          <span class="dot {dotClass(s)}" title="{s.name} ({s.ip}) — {s.status}"></span>
        {/each}
      </div>
      {#each buttons as btn (btn.graphic_id)}
        <button class="trig" class:flash={flashId === btn.graphic_id} onclick={() => press(btn)}>
          {btn.label}
        </button>
      {/each}
      <button class="gear" onclick={openSettings} title="설정">⚙</button>
    </div>
    {#if warning}
      <div class="warning">{warning}</div>
    {/if}
  </GlassPanel>
  {#if editMode}
    <!-- role="presentation" (no tabindex): these grips are pointer/touch-only
         drag targets, not keyboard-operable controls, so removing them from
         the accessibility tree is the honest description — not "button". -->
    <div class="grip nw" role="presentation" onpointerdown={(e) => startResize(e, 'nw')} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} title="드래그로 크기 조절"></div>
    <div class="grip ne" role="presentation" onpointerdown={(e) => startResize(e, 'ne')} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} title="드래그로 크기 조절"></div>
    <div class="grip sw" role="presentation" onpointerdown={(e) => startResize(e, 'sw')} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} title="드래그로 크기 조절"></div>
    <div class="grip se" role="presentation" onpointerdown={(e) => startResize(e, 'se')} onpointermove={moveResize} onpointerup={endResize} onpointercancel={endResize} title="드래그로 크기 조절"></div>
  {/if}
</div>

<style>
  /* The palette fills the OS window exactly, so the vibrancy/blur area and
     the rounded glass panel are always the same rectangle — no pale blurry
     margin sticking out beyond the panel. */
  .palette-root {
    position: relative;
    width: 100vw;
    height: 100vh;
    box-sizing: border-box;
  }
  /* Stretch the shared GlassPanel to the wrapper without touching the
     component itself (demo/harness reuse it unstretched). Flex keeps the
     row vertically centered; overflow in a small window clips (body has
     overflow:hidden — no scrollbars). */
  .palette-root > :global(.glass) {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .palette-root.editing {
    /* Inset ring: the root now spans the whole viewport, so an outset
       outline would be clipped off-screen. */
    outline: 2px solid var(--accent);
    outline-offset: -3px;
    border-radius: var(--radius);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    white-space: nowrap;
  }
  .handle {
    cursor: grab;
    color: var(--text-dim);
    font-size: 18px;
    padding: 6px 4px;
    user-select: none;
    touch-action: none;
  }
  .dots { display: flex; gap: 6px; padding-right: 4px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; }
  .dot.active { background: var(--status-active); }
  .dot.inactive { background: transparent; border: 2px solid var(--status-inactive); }
  .dot.lost { background: var(--status-lost); }
  .trig, .gear {
    min-height: var(--touch-min);
    min-width: var(--touch-min);
    padding: 0 20px;
    border: 1px solid var(--glass-border);
    border-radius: calc(var(--radius) - 4px);
    background: var(--btn-fill);
    color: var(--text);
    font-size: 16px;
    cursor: pointer;
    transition: transform 0.08s ease, background 0.15s ease;
  }
  .trig:active, .trig.flash { background: var(--accent); transform: scale(0.96); }
  .gear { padding: 0 14px; color: var(--text-dim); }
  .warning {
    padding: 6px 14px 10px;
    color: var(--status-lost);
    font-size: 12px;
    white-space: normal;
  }
  .grip {
    position: absolute;
    width: 22px;
    height: 22px;
    background: var(--accent);
    border-radius: 5px;
    opacity: 0.9;
    touch-action: none;
  }
  /* Inset corners: the root spans the whole viewport now, so the old
     negative offsets would hang off-screen and shrink the touch target. */
  .grip.nw { top: 3px; left: 3px; cursor: nwse-resize; }
  .grip.ne { top: 3px; right: 3px; cursor: nesw-resize; }
  .grip.sw { bottom: 3px; left: 3px; cursor: nesw-resize; }
  .grip.se { bottom: 3px; right: 3px; cursor: nwse-resize; }
</style>
