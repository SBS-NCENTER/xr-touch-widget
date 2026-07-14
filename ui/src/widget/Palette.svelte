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
    onAppearancePreview,
    onWindowMoved,
    setSize,
    innerSize,
  } from './ipc.js';

  const LONG_PRESS_MS = 1000;
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
  // Press-flash + last-pressed emphasis key off the button INDEX (D14): the
  // button identity is no longer a unique graphic_id, and the {#each} is
  // index-keyed, so the index is the stable per-button handle here.
  let flashIndex = $state(null);
  let editMode = $state(false);

  // Button-grid layout (D11) and last-press emphasis (D12) — both driven by
  // config, with sane fallbacks so the browser harness (whose mock config
  // may lag the real schema) still renders something reasonable. Initialized
  // to the product default (vertical, Task 8b) so the pre-config-load frame
  // matches — applyConfig overwrites it on mount anyway (M-2).
  let layout = $state({ horizontal: false, vertical: true, cols: 3, rows: 2 });
  let highlightLast = $state(false);
  // Runtime-only (D12): which button INDEX was pressed last, for the optional
  // font-weight emphasis. Never saved to config, never restored on launch.
  let lastPressedIndex = $state(null);

  // Full config kept around outside Svelte state (edit-mode exit needs to
  // patch just `window` and save the whole object back — it never drives
  // a render by itself).
  let latestConfig = null;
  let longPressTimer = null;
  let longPressStartX = 0;
  let longPressStartY = 0;
  // Single-pointer gesture state for the handle press (Task 8b), keyed off
  // pointerId so a concurrent second finger can't disturb the primary gesture
  // (I-1). `gesturePointerId` is the active pointer (null = no gesture);
  // `enteredThisGesture` marks that the long-press just entered edit mode this
  // gesture (so its OWN pointerup must not immediately exit again);
  // `movedThisGesture` marks a drag past MOVE_CANCEL_PX (so a drag-release
  // never toggles — and is also the only state in which a native-window-drag
  // can swallow the primary pointerup and strand gesturePointerId, so it
  // doubles as the "a fresh pointerdown may take over" signal). Plain lets.
  let gesturePointerId = null;
  let enteredThisGesture = false;
  let movedThisGesture = false;

  // Manual grip-resize session state — plain (non-reactive) lets, nothing
  // here drives a render. `resizing` is null outside an active drag.
  let resizing = null;
  let resizeRafId = null;
  let pendingW = 0;
  let pendingH = 0;
  // Bound to the control-cluster element so the resize path can measure its
  // natural cross-axis extent (Task 8b) and let the window shrink to hug it.
  let clusterEl;

  // --- Button grid (Task 8b/D11): config.layout drives cols/rows as CSS
  // custom properties consumed by the .grid rule below. `grid-auto-rows: 1fr`
  // there lets the browser add further implicit rows (sized the same as the
  // templated ones) whenever there are more buttons than the configured
  // cols x rows slots — so `rows` behaves as a MINIMUM row count (actual
  // rows = max(rows, ceil(buttonCount / cols))) and no button is ever
  // hidden or clipped, without any row-count math needed here in JS. ---
  let gridTemplate = $derived.by(() => {
    const { horizontal, vertical, cols, rows } = layout;
    if (horizontal && vertical) {
      return { cols: Math.max(1, cols), rows: Math.max(1, rows) };
    }
    if (vertical) {
      // Vertical-only: a single column. Extra rows beyond the first come
      // from grid-auto-rows (stylesheet), so only 1 needs templating here.
      return { cols: 1, rows: 1 };
    }
    // Horizontal-only, or neither checkbox set (fallback): a single row,
    // one column per button. cols/rows numbers are ignored either way.
    return { cols: Math.max(1, buttons.length), rows: 1 };
  });

  // Cluster orientation is always PERPENDICULAR to the button flow (Task 8b).
  // Vertical-only button mode flows buttons as a column, so the cluster sits
  // on TOP and lays out horizontally (.layout = column). Every other mode
  // (horizontal-only, both/grid, neither/fallback) flows buttons horizontally,
  // so the cluster sits on the LEFT and stacks vertically (.layout = row) —
  // this is the intended default. Drives both directions via a class.
  let clusterOnTop = $derived(layout.vertical && !layout.horizontal);

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
    // Last-press underline color+opacity (Task 9 P2), composed the same way as
    // --glass-bg. Falls back to the accent / full opacity if a pre-P2 config
    // (or a lagging mock) omits the fields, so the underline never goes blank.
    const hl = hexToRgb(appearance.highlight_color ?? appearance.accent);
    const hlOpacity = appearance.highlight_opacity ?? 1;
    root.setProperty('--highlight-underline', `rgba(${hl.r}, ${hl.g}, ${hl.b}, ${hlOpacity})`);
  }

  function applyConfig(config) {
    latestConfig = config;
    buttons = config.buttons;
    statuses = config.targets.map((t) => ({ ...t, status: 'Unknown' }));
    applyAppearance(config.appearance);
    layout = config.layout ?? layout;
    highlightLast = config.appearance?.highlight_last ?? false;
    // Task 9: settings' [적용] fires onConfigChanged with the FULL saved
    // config, which must include the window size taking effect live — same
    // as a preview, but persistent. Harmless no-op on the initial mount call
    // (Rust already applied this exact size at setup) since setSize with an
    // unchanged size is idempotent.
    if (config.window) setSize(config.window.width, config.window.height);
  }

  /** Task 9/D10: non-persistent live preview from the settings window while
   *  it is open. Only appearance/layout/window ever ride this event — never
   *  buttons/targets (those only take effect through onConfigChanged/Apply),
   *  so `buttons`/`statuses`/`latestConfig` are deliberately untouched here.
   *  Reusing applyAppearance/layout-assignment/setSize keeps this on the same
   *  code path applyConfig uses, per Task 8b's single-render-path design. */
  function applyPreview(payload) {
    if (!payload) return;
    applyAppearance(payload.appearance);
    if (payload.layout) layout = payload.layout;
    highlightLast = payload.appearance?.highlight_last ?? highlightLast;
    if (payload.window) setSize(payload.window.width, payload.window.height);
  }

  $effect(() => {
    let unsubs = [];
    (async () => {
      const config = await getConfig();
      applyConfig(config);
      warning = await loadWarning();
      unsubs.push(await onStatus((list) => (statuses = list)));
      unsubs.push(await onConfigChanged((config) => applyConfig(config)));
      unsubs.push(await onAppearancePreview((payload) => applyPreview(payload)));
      // Cancel an armed enter-timer the moment the window actually moves
      // (native drag). This is the ONE case the 8px pointermove guard can't
      // catch, because the OS drag session stops delivering pointermove to
      // the webview. It only CANCELS an armed enter-timer — never fires an
      // exit or toggle — and marks the gesture moved so the eventual release
      // can't spuriously toggle either. Everything else in the gesture state
      // machine (gesturePointerId gating, enteredThisGesture, exit-on-tap)
      // is left untouched.
      unsubs.push(
        await onWindowMoved(() => {
          if (longPressTimer) {
            cancelLongPress();
            movedThisGesture = true;
          }
        }),
      );
    })();
    return () => unsubs.forEach((u) => u());
  });

  async function press(btn, i) {
    flashIndex = i;
    lastPressedIndex = i;
    setTimeout(() => (flashIndex = null), 250);
    // D14: send the button's full OSC message spec (address + typed value).
    await trigger(btn.address, btn.value_type, btn.value);
  }

  function dotClass(s) {
    if (s.status === 'Lost') return 'lost';
    return s.active ? 'active' : 'inactive';
  }

  // --- Edit mode (D8, Task 8b): NORMAL mode → a 1s long-press on the
  // handle ENTERS edit mode; EDIT mode → a short stationary tap on the handle
  // EXITS it. A drag (movement ≥ MOVE_CANCEL_PX) always just moves the window
  // (native drag region) and never toggles, in either mode. The window is
  // NEVER OS-resizable — resize happens only via programmatic setSize from the
  // SE grip, which only exists in edit mode, so a live-broadcast mis-touch
  // can't reshape the palette. ---
  function handlePointerDown(event) {
    // Single-pointer gating (I-1): a genuine concurrent second finger landing
    // on the small handle while a stationary press is active (movedThisGesture
    // false) is IGNORED — otherwise its flag-reset could make the primary
    // pointer's release spuriously exit edit mode. A DRAG in progress
    // (movedThisGesture true) is the only state where the native window-drag
    // can swallow the primary pointerup and strand gesturePointerId, so once
    // dragging a fresh pointerdown is allowed to take over rather than be
    // blocked forever. (When no gesture is active, gesturePointerId is null
    // and we always start fresh.)
    if (gesturePointerId !== null && event.pointerId !== gesturePointerId && !movedThisGesture) {
      return;
    }
    // Start (or take over) a fresh gesture keyed to this pointer. cancelLongPress
    // also clears any leaked timer from a prior gesture (finding 2).
    gesturePointerId = event.pointerId;
    cancelLongPress();
    enteredThisGesture = false;
    movedThisGesture = false;
    longPressStartX = event.clientX;
    longPressStartY = event.clientY;
    // Only arm the enter-edit long-press when NOT already in edit mode; in
    // edit mode the exit is a tap decided at pointerup, no timer needed.
    if (!editMode) {
      longPressTimer = setTimeout(() => {
        longPressTimer = null;
        editMode = true;
        enteredThisGesture = true; // this gesture's pointerup must not exit
      }, LONG_PRESS_MS);
    }
  }

  // Tracks drag intent. Tauri's native drag region commonly stops delivering
  // pointer events to the webview once an OS drag session starts, so
  // pointerup/pointercancel can never be relied on to arrive — a movement
  // threshold checked on whatever pointermove events do land is the robust
  // signal that this is a drag, not a press-in-place (finding 1). Setting
  // `movedThisGesture` also makes the pointerup exit-check below fail, so a
  // drag-release never toggles edit mode.
  function handlePointerMove(event) {
    if (event.pointerId !== gesturePointerId) return; // ignore non-primary pointers
    const dx = event.clientX - longPressStartX;
    const dy = event.clientY - longPressStartY;
    if (Math.hypot(dx, dy) > MOVE_CANCEL_PX) {
      movedThisGesture = true;
      cancelLongPress(); // a drag in progress must not fire the enter timer
    }
  }

  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  }

  async function handlePointerUp(event) {
    if (event.pointerId !== gesturePointerId) return; // ignore non-primary pointers
    cancelLongPress();
    // Exit only on a genuine, separate short tap while in edit mode: NOT the
    // same pointerup that ended the entering long-press (enteredThisGesture),
    // and NOT the release of a drag (movedThisGesture). A later stationary tap
    // then satisfies both and exits.
    const shouldExit = editMode && !enteredThisGesture && !movedThisGesture;
    gesturePointerId = null; // gesture over — clear before any await
    if (shouldExit) await exitEditMode();
  }

  function handlePointerCancel(event) {
    if (event.pointerId !== gesturePointerId) return; // ignore non-primary pointers
    // Ambiguous end of gesture (cancel / left the handle): never toggles.
    cancelLongPress();
    gesturePointerId = null;
  }

  async function exitEditMode() {
    // Exiting edit mode just hides the grip (no OS window flag to undo — the
    // window is never resizable) and persists the final size.
    editMode = false;
    const size = await innerSize();
    if (size && latestConfig) {
      const config = { ...latestConfig, window: { width: size.width, height: size.height } };
      latestConfig = config;
      await saveConfig(config);
    }
  }

  // Natural extent of the control cluster on the given axis ('x' | 'y') in
  // logical/CSS px, PLUS the fixed chrome outside the cluster on that axis
  // (the glass border). The cluster is stretched to fill its cross axis, so
  // its own box can't be measured directly for a natural size — instead we
  // take the bounding span of its children (their natural, centered
  // positions), then add the cluster's own padding and the outer chrome
  // (viewport minus the stretched cluster box). getBoundingClientRect and
  // innerWidth/Height are CSS px, matching the logical-px resize math, so no
  // scaleFactor conversion is needed. Returns null if the cluster ref or its
  // children aren't ready (caller falls back to the fixed floor).
  function clusterMinExtent(axis) {
    const el = clusterEl;
    if (!el || el.children.length === 0) return null;
    let lo = Infinity;
    let hi = -Infinity;
    for (const child of el.children) {
      const r = child.getBoundingClientRect();
      lo = Math.min(lo, axis === 'x' ? r.left : r.top);
      hi = Math.max(hi, axis === 'x' ? r.right : r.bottom);
    }
    const cs = getComputedStyle(el);
    const pad =
      axis === 'x'
        ? parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight)
        : parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const box = el.getBoundingClientRect();
    const viewport = axis === 'x' ? window.innerWidth : window.innerHeight;
    const outerChrome =
      axis === 'x' ? box.left + (viewport - box.right) : box.top + (viewport - box.bottom);
    return Math.ceil(hi - lo + pad + outerChrome);
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
    // Lower bounds: on the axis PERPENDICULAR to the button flow, let the
    // window shrink to hug the cluster's measured natural size; keep the
    // fixed floor on the other axis. Measured now so it reflects the current
    // dot count / gear size / orientation. Vertical mode (cluster on top) →
    // clamp WIDTH; horizontal mode (cluster on left) → clamp HEIGHT. Falls
    // back to the fixed floor if the cluster ref isn't ready.
    let minW = RESIZE_MIN_W;
    let minH = RESIZE_MIN_H;
    if (clusterOnTop) {
      minW = clusterMinExtent('x') ?? RESIZE_MIN_W;
    } else {
      minH = clusterMinExtent('y') ?? RESIZE_MIN_H;
    }
    resizing = { corner, startX, startY, startW: start.width, startH: start.height, minW, minH };
  }

  function moveResize(event) {
    if (!resizing) return;
    const dx = event.clientX - resizing.startX;
    const dy = event.clientY - resizing.startY;
    // Direction-aware: a west grip grows width when dragging left, etc.
    const west = resizing.corner.includes('w');
    const north = resizing.corner.includes('n');
    pendingW = Math.max(resizing.minW, Math.round(resizing.startW + (west ? -dx : dx)));
    pendingH = Math.max(resizing.minH, Math.round(resizing.startH + (north ? -dy : dy)));
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
    <div class="layout" class:cluster-top={clusterOnTop}>
      <!-- Control cluster (Task 8b): handle + gear + status dots, kept
           together as one group, always PERPENDICULAR to the button flow —
           a vertical stack on the LEFT by default, a horizontal strip on TOP
           in vertical-only button mode (class:cluster-top). flex-shrink:0
           keeps it compact regardless of grid dimensions or window resize. -->
      <div class="cluster" bind:this={clusterEl}>
        <div
          class="handle"
          data-tauri-drag-region
          role="button"
          tabindex="0"
          aria-label="드래그로 이동 · 길게 눌러 편집 모드 진입 · 편집 중 탭하여 종료"
          onpointerdown={handlePointerDown}
          onpointermove={handlePointerMove}
          onpointerup={handlePointerUp}
          onpointercancel={handlePointerCancel}
          onpointerleave={handlePointerCancel}
        >☰</div>
        <button class="gear" onclick={openSettings} title="설정">⚙</button>
        <div class="dots" title="active targets">
          {#each statuses as s, i (i)}
            <span class="dot {dotClass(s)}" title="{s.name} ({s.ip}) — {s.status}"></span>
          {/each}
        </div>
      </div>
      <div class="grid" style="--cols: {gridTemplate.cols}; --rows: {gridTemplate.rows};">
        {#each buttons as btn, i (i)}
          <button
            class="trig"
            class:flash={flashIndex === i}
            class:last-pressed={highlightLast && lastPressedIndex === i}
            onclick={() => press(btn, i)}
          >
            <span class="label">{btn.label}</span>
          </button>
        {/each}
      </div>
    </div>
    {#if warning}
      <div class="warning">{warning}</div>
    {/if}
  </GlassPanel>
  {#if editMode}
    <!-- SE-only (Task 8b): the palette is anchored at its top-left (the
         handle drags the window; setSize keeps that corner fixed), so a
         single bottom-right grip is enough to grow/shrink it. -->
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
    /* The palette has no text inputs — accidental text selection (e.g. a
       drag that grabs a button label) is never wanted on the live widget. */
    user-select: none;
    -webkit-user-select: none;
  }
  /* Stretch the shared GlassPanel to the wrapper without touching the
     component itself (demo/harness reuse it unstretched). The glass is a
     flex column [layout][warning]; .layout is a flex row OR column
     [cluster][grid] depending on button flow (see .layout below).
     overflow:hidden keeps a still-growing grid from poking out past the
     panel's rounded corners. */
  .palette-root > :global(.glass) {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .palette-root.editing {
    /* Inset ring: the root now spans the whole viewport, so an outset
       outline would be clipped off-screen. */
    outline: 2px solid var(--accent);
    outline-offset: -3px;
    border-radius: var(--radius);
  }

  /* Layout direction follows the button flow (Task 8b). Default (horizontal /
     grid / neither) flows buttons horizontally → cluster stacks vertically on
     the LEFT → .layout is a row. Vertical-only button mode flows buttons as a
     column → cluster goes horizontal on TOP → .layout is a column. The cluster
     is always perpendicular to the button flow. min-size:0 on both axes lets
     the grid child shrink with the window whichever axis it grows along. */
  .layout {
    display: flex;
    flex-direction: row;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .layout.cluster-top {
    flex-direction: column;
  }

  /* Compact, fixed-size control group (flex-shrink:0 — the grid, not the
     cluster, absorbs resizing). Its own flex-direction is perpendicular to
     the button flow: a vertical stack on the LEFT by default, a horizontal
     strip on TOP in vertical-only mode. justify/align center keeps the group
     centered along the window edge it is pinned to (and, since the cluster
     stretches to the cross-axis full size, it never looks stretched). */
  .cluster {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 6px 10px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .layout.cluster-top .cluster {
    flex-direction: row;
  }
  .handle {
    cursor: grab;
    color: var(--text-dim);
    font-size: 18px;
    line-height: 1;
    padding: 6px 4px;
    user-select: none;
    touch-action: none;
  }
  .dots { display: flex; gap: 6px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; }
  .dot.active { background: var(--status-active); }
  .dot.inactive { background: transparent; border: 2px solid var(--status-inactive); }
  .dot.lost { background: var(--status-lost); }
  /* Gear matches the ☰ handle's size (Task 8b): no touch-min floor, same glyph
     font + padding, borderless/transparent so the three items read as one
     uniform group and the vertical stack fits within the 96px-tall default
     bar. Still a <button>, so click + keyboard activation are unchanged. */
  .gear {
    padding: 6px 4px;
    border: none;
    background: none;
    appearance: none;
    color: var(--text-dim);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
  }

  /* Button grid (Step 3/D11). --cols/--rows come from the inline style
     above (gridTemplate). grid-auto-rows lets item overflow beyond the
     templated rows grow extra rows automatically, sized the same way —
     see the gridTemplate derivation in the script for the full rationale. */
  .grid {
    --cols: 1;
    --rows: 1;
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: grid;
    grid-template-columns: repeat(var(--cols), 1fr);
    grid-template-rows: repeat(var(--rows), 1fr);
    grid-auto-rows: 1fr;
    gap: 8px;
    padding: 10px 14px;
  }
  .trig {
    /* Step 4/D11: establishes a query container sized by the grid track
       (not by its own content), so the label below can scale its font off
       the cell's actual size rather than the viewport. This is the fix for
       labels overflowing when the window (and so each cell) gets small. */
    container-type: size;
    display: flex;
    align-items: center;
    justify-content: center;
    /* No touch-min floor here (unlike .gear): a hard minimum would stop the
       grid track from shrinking below it, defeating the shrink-then-scale
       behavior this step exists to deliver. Cell size is the user's call
       once they've picked cols/rows/window size in edit mode. */
    min-width: 0;
    min-height: 0;
    padding: 4px 10px;
    border: 1px solid var(--glass-border);
    border-radius: calc(var(--radius) - 4px);
    background: var(--btn-fill);
    color: var(--text);
    font-weight: 400;
    cursor: pointer;
    overflow: hidden;
    transition: transform 0.08s ease, background 0.15s ease;
  }
  .trig:active, .trig.flash { background: var(--accent); transform: scale(0.96); }
  /* D12: persists until another button is pressed, only when
     appearance.highlight_last is on (class only applied then). Calm,
     persistent emphasis (NOT the press-flash): a heavy weight plus an accent
     underline UNDER THE LABEL TEXT (spanning only the text, not the button
     width) so the last-pressed button reads at a glance without mimicking the
     active/flash state. */
  .trig.last-pressed { font-weight: 800; }
  .trig.last-pressed .label {
    text-decoration: underline;
    /* Configurable color+opacity (Task 9 P2), set by applyAppearance from
       appearance.highlight_color/highlight_opacity. Falls back to --accent
       if the var is never set. */
    text-decoration-color: var(--highlight-underline, var(--accent));
    text-decoration-thickness: 2px;
    /* Offset kept small (2px) and the label carries a padding-bottom (below)
       so the underline lands INSIDE the label's non-clipped region — see the
       .label rule for why a larger offset gets clipped. */
    text-underline-offset: 2px;
  }
  .trig .label {
    display: block;
    max-width: 100%;
    /* cqmin = % of the smaller of the cell's own width/height, so the label
       shrinks together with the cell in either dimension. clamp() bounds it
       so a huge cell doesn't blow the label up and a tiny one doesn't
       shrink it to unreadable. */
    font-size: clamp(10px, 22cqmin, 18px);
    /* overflow:hidden clips at the PADDING box edge. The last-pressed accent
       underline sits a couple px below the text baseline — with a tight line
       box it would fall past the content box and get clipped, showing only
       the bold weight. This padding-bottom extends the padding box downward,
       so the underline lands within the visible (non-clipped) region. It is
       on the BASE label (not just last-pressed) so text position is identical
       whether or not the underline is shown — no vertical jump on press.
       ellipsis still clips horizontally (white-space:nowrap) as before. */
    padding-bottom: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warning {
    padding: 6px 14px 10px;
    color: var(--status-lost);
    font-size: 12px;
    white-space: normal;
    flex-shrink: 0;
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
  /* Inset corner: the root spans the whole viewport, so the old negative
     offset would hang off-screen and shrink the touch target. */
  .grip.se { bottom: 3px; right: 3px; cursor: nwse-resize; }
</style>
