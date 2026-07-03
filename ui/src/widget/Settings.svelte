<script>
  import {
    getConfig,
    saveConfig,
    loadWarning,
    emitAppearancePreview,
    onStatus,
    quit,
    closeWindow,
  } from './ipc.js';

  // Draft/preview model (Task 9, D10): `saved` is the config as loaded, never
  // mutated — it is the baseline [뒤로가기] restores. `draft` is the editable
  // working copy every input binds to. Only [적용] persists `draft` and
  // rebases `saved` onto it; [뒤로가기] discards `draft` entirely.
  let saved = null;
  let draft = $state(null);
  let warning = $state(null);
  let appliedFlash = $state(false);
  let applyError = $state(false);
  // Per-target connection status mirrored from the palette's xrt://status
  // feed (D13, P3): ip -> LinkStatus string ('Connected'|'Lost'|'Unknown').
  // Reflects the SAVED/running config — the engine only pings applied targets.
  let statusByIp = $state({});

  // Clamp bounds for the free-typed numeric fields (review fix). Below-min or
  // above-max entries are pulled into range on commit so a mistyped value can
  // never preview/persist an unusable window or grid. logical px for window,
  // grid-cell counts for layout.
  const WINDOW_MIN = 120;
  const WINDOW_MAX = 4000;
  const GRID_MIN = 1;
  const GRID_MAX = 24;

  $effect(() => {
    let unsubs = [];
    (async () => {
      const config = await getConfig();
      saved = structuredClone(config);
      draft = structuredClone(config);
      warning = await loadWarning();
      // Mirror the palette's per-target status dots for ACTIVE rows (D13/P3).
      unsubs.push(
        await onStatus((list) => {
          const map = {};
          for (const s of list) map[s.ip] = s.status;
          statusByIp = map;
        }),
      );
    })();
    return () => unsubs.forEach((u) => u());
  });

  /** Status-dot class for a target row, or null for NO dot. Only ACTIVE
   *  targets that are present in the running status map get a dot; a
   *  draft-only target (ip not yet applied/pinged) has no dot until applied.
   *  Colors match the palette: Connected=green, Lost=red, Unknown/other=grey. */
  function activeStatus(t) {
    if (!t.active) return null;
    const st = statusByIp[t.ip];
    if (st === undefined) return null;
    if (st === 'Connected') return 'connected';
    if (st === 'Lost') return 'lost';
    return 'unknown';
  }

  /** Extract the {appearance, layout, window} slice a preview event carries,
   *  as a plain (non-proxy) object safe to hand to Tauri's emit. Each slice is
   *  snapshotted individually so this — and any $effect that calls it — reads
   *  ONLY those three subtrees, never draft.targets/draft.buttons. */
  function previewPayload(cfg) {
    return {
      appearance: $state.snapshot(cfg.appearance),
      layout: $state.snapshot(cfg.layout),
      window: $state.snapshot(cfg.window),
    };
  }

  // Live preview (D10): any appearance/layout/window field change in `draft`
  // broadcasts immediately, non-persistently, so the palette reflects it
  // before [적용]. Because previewPayload deep-reads only draft.appearance,
  // draft.layout and draft.window, this effect depends on exactly those three
  // subtrees — target/button list edits never re-fire it. It also runs once
  // when `draft` first loads, re-broadcasting the just-loaded (already
  // current) state — harmless, the palette already matches it.
  $effect(() => {
    if (!draft) return;
    emitAppearancePreview(previewPayload(draft));
  });

  /** Commit a free-typed number field on blur/Enter (NOT per keystroke), so
   *  the preview $effect fires once per committed value instead of thrashing
   *  the on-air palette mid-typing, and `draft` never holds null/NaN.
   *  `obj` is a reactive draft sub-object (draft.window | draft.layout);
   *  empty/NaN/out-of-range input falls back to the current draft value,
   *  everything else is clamped into [min, max]. The committed value is
   *  written back to the input element too, since a fallback that equals the
   *  current draft value won't re-render the (now-cleared) input on its own. */
  function commitNumber(event, obj, key, min, max) {
    const el = event.currentTarget;
    const parsed = parseInt(el.value, 10);
    const next = Number.isNaN(parsed) ? obj[key] : Math.min(max, Math.max(min, parsed));
    obj[key] = next;
    el.value = String(next);
  }

  function addTarget() {
    draft.targets.push({ name: '', ip: '', active: false });
  }
  function removeTarget(i) {
    draft.targets.splice(i, 1);
  }
  function addButton() {
    draft.buttons.push({ label: '', graphic_id: '', type: 'trigger' });
  }
  function removeButton(i) {
    draft.buttons.splice(i, 1);
  }
  function moveButton(i, delta) {
    const j = i + delta;
    if (j < 0 || j >= draft.buttons.length) return;
    [draft.buttons[i], draft.buttons[j]] = [draft.buttons[j], draft.buttons[i]];
  }

  /** [적용]: persist the draft (also emits xrt://config-changed via Rust, so
   *  the palette re-applies the FULL config), then rebase `saved` onto it so
   *  a later [뒤로가기] in this same session restores the applied state. */
  async function apply() {
    const snapshot = $state.snapshot(draft);
    // Defense-in-depth (C3): a button with a blank/whitespace-only graphic_id
    // triggers nothing on-air. Block the apply and flash the existing error
    // affordance so the operator notices the missing graphic_id before it goes
    // live. Duplicates are crash-safe (the palette keys its {#each} by index)
    // and may be intentional aliasing, so ONLY blanks are blocked here.
    if (snapshot.buttons.some((b) => !b.graphic_id || !b.graphic_id.trim())) {
      applyError = true;
      setTimeout(() => (applyError = false), 2000);
      return;
    }
    try {
      await saveConfig(snapshot);
    } catch (e) {
      // saveConfig can reject (e.g. serde refusing a bad field on the Rust
      // side). The operator must never get a silent no-op on a failed apply,
      // so surface a brief visible error state instead of swallowing it.
      console.error('save_config failed', e);
      applyError = true;
      setTimeout(() => (applyError = false), 2000);
      return;
    }
    saved = structuredClone(snapshot);
    appliedFlash = true;
    setTimeout(() => (appliedFlash = false), 1200);
  }

  /** [뒤로가기]: discard the draft — restore the palette's preview to the
   *  last-saved state, then close. Never persists. */
  async function back() {
    await emitAppearancePreview(previewPayload(saved));
    await closeWindow();
  }

  async function quitApp() {
    await quit();
  }
</script>

{#if draft}
  <div class="panel">
    <div class="drag-bar">
      <!-- Title is the drag handle (flex:1 fills the empty strip area);
           data-tauri-drag-region is on the title ONLY, never on the action
           buttons beside it, so they stay clickable. -->
      <h1 data-tauri-drag-region>설정</h1>
      <div class="actions">
        <button class="apply" class:done={appliedFlash} class:error={applyError} onclick={apply}>
          {appliedFlash ? '적용됨 ✓' : applyError ? '적용 실패' : '적용'}
        </button>
        <button class="back" onclick={back}>뒤로가기</button>
        <button class="quit" onclick={quitApp}>프로그램 종료</button>
      </div>
    </div>
    <div class="scroll">
      {#if warning}<div class="warning">{warning}</div>{/if}

      <h2>XR 장비</h2>
      {#each draft.targets as t, i}
        {@const dot = activeStatus(t)}
        <div class="grid-row">
          {#if dot}
            <span class="dot {dot}" title="연결 상태"></span>
          {/if}
          <input placeholder="이름" bind:value={t.name} />
          <input placeholder="IP" bind:value={t.ip} />
          <label class="chk"><input type="checkbox" bind:checked={t.active} /> 활성</label>
          <button class="del" onclick={() => removeTarget(i)}>✕</button>
        </div>
      {/each}
      <button class="add" onclick={addTarget}>+ 장비 추가</button>

      <h2>버튼</h2>
      {#each draft.buttons as b, i}
        <div class="grid-row">
          <input placeholder="라벨" bind:value={b.label} />
          <input placeholder="graphic_id" bind:value={b.graphic_id} />
          <span class="order">
            <button onclick={() => moveButton(i, -1)}>▲</button>
            <button onclick={() => moveButton(i, 1)}>▼</button>
          </span>
          <button class="del" onclick={() => removeButton(i)}>✕</button>
        </div>
      {/each}
      <button class="add" onclick={addButton}>+ 버튼 추가</button>

      <h2>외형·레이아웃</h2>
      <label class="field">
        배경 투명도 ({Math.round(draft.appearance.bg_opacity * 100)}%)
        <input type="range" min="0" max="1" step="0.01" bind:value={draft.appearance.bg_opacity} />
      </label>
      <label class="field">
        버튼 투명도 ({Math.round(draft.appearance.button_opacity * 100)}%)
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={draft.appearance.button_opacity}
        />
      </label>
      <div class="grid-row">
        <label class="field-inline">
          강조색
          <input type="color" bind:value={draft.appearance.accent} />
        </label>
        <label class="field-inline">
          배경 색조
          <input type="color" bind:value={draft.appearance.bg_tint} />
        </label>
      </div>
      <label class="chk">
        <input type="checkbox" bind:checked={draft.appearance.highlight_last} /> 마지막 누른 버튼 강조
      </label>
      <div class="grid-row">
        <label class="field-inline">
          강조 밑줄 색
          <input type="color" bind:value={draft.appearance.highlight_color} />
        </label>
      </div>
      <label class="field">
        강조 밑줄 투명도 ({Math.round(draft.appearance.highlight_opacity * 100)}%)
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={draft.appearance.highlight_opacity}
        />
      </label>

      <div class="grid-row">
        <label class="field-inline">
          창 너비
          <input
            type="number"
            min={WINDOW_MIN}
            max={WINDOW_MAX}
            step="1"
            value={draft.window.width}
            onchange={(e) => commitNumber(e, draft.window, 'width', WINDOW_MIN, WINDOW_MAX)}
          />
        </label>
        <label class="field-inline">
          창 높이
          <input
            type="number"
            min={WINDOW_MIN}
            max={WINDOW_MAX}
            step="1"
            value={draft.window.height}
            onchange={(e) => commitNumber(e, draft.window, 'height', WINDOW_MIN, WINDOW_MAX)}
          />
        </label>
      </div>

      <div class="grid-row">
        <label class="chk"><input type="checkbox" bind:checked={draft.layout.horizontal} /> 가로</label>
        <label class="chk"><input type="checkbox" bind:checked={draft.layout.vertical} /> 세로</label>
      </div>
      <div class="grid-row">
        <label class="field-inline">
          가로 수
          <input
            type="number"
            min={GRID_MIN}
            max={GRID_MAX}
            step="1"
            value={draft.layout.cols}
            onchange={(e) => commitNumber(e, draft.layout, 'cols', GRID_MIN, GRID_MAX)}
          />
        </label>
        <label class="field-inline">
          세로 수
          <input
            type="number"
            min={GRID_MIN}
            max={GRID_MAX}
            step="1"
            value={draft.layout.rows}
            onchange={(e) => commitNumber(e, draft.layout, 'rows', GRID_MIN, GRID_MAX)}
          />
        </label>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Task 9/D9: opaque solid panel, NOT the shared translucent GlassPanel —
     the settings window has no transparency/vibrancy at all, readability
     first. Border/radius/text still borrow the shared tokens for a
     consistent look; background is a hardcoded opaque hex (tokens.css only
     exposes a translucent --glass-bg). */
  .panel {
    width: 100vw;
    height: 100vh;
    box-sizing: border-box;
    /* Neutral dark grey (no blue tint), opaque (D9 — settings never uses
       transparency/blur). Matches settings.html body so the window's rounded
       corners never reveal a square background of a different color. */
    background: #2d2d2d;
    border: 1px solid var(--glass-border);
    /* Matches round_content_view_corners(&win, 14.0) in open_settings so the
       web content corners align with the rounded window (--radius = 14px). */
    border-radius: var(--radius);
    color: var(--text);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    /* Disable accidental text selection across the whole settings chrome
       (labels, headings, drag bar). Editable fields re-enable it below. */
    user-select: none;
    -webkit-user-select: none;
  }
  /* Exception: the name/IP/label/graphic_id/number fields must stay
     selectable and editable — user-select is inherited, so re-enable it
     explicitly on the inputs. (Settings has no <textarea>; that selector was
     dead and flagged unused by the Svelte compiler, so it's dropped here.) */
  input {
    user-select: text;
    -webkit-user-select: text;
  }
  /* Full-width header strip: the "설정" title (left, draggable) + the compact
     적용/뒤로가기/종료 action group (right, clickable). data-tauri-drag-region
     lives ONLY on the <h1> title (flex:1, so it also covers the empty strip
     area), NEVER on the buttons or any form control — those stay clickable.
     The divider separates the strip from the scrolling body. */
  .drag-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 46px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--glass-border);
  }
  h1 {
    flex: 1;
    font-size: 16px;
    margin: 0;
    cursor: grab; /* only the title strip is the drag handle */
  }
  /* Compact action buttons in the title row (smaller than the old full-width
     footer buttons — they no longer need to fill the width). */
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .actions button {
    min-height: 32px;
    padding: 0 10px;
    font-size: 13px;
  }
  .actions .apply { background: var(--accent); }
  .actions .apply.done { background: var(--status-active); }
  .actions .apply.error { background: var(--status-lost); }
  .actions .back { color: var(--text-dim); }
  .actions .quit { color: var(--status-lost); }
  /* Only this body scrolls — the .drag-bar title strip (flex-shrink:0, above)
     stays fixed. overflow-y:scroll (not auto) permanently reserves the track
     so the scrollbar is ALWAYS present, not auto-hidden by macOS/WKWebView.
     .scroll lives INSIDE the rounded .panel (which clips with border-radius +
     overflow:hidden), so the scrollbar is masked to the panel's rounded
     bottom-right corner and never pokes past it. */
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: scroll;
    padding: 10px 22px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* Always-visible, touch-friendly scrollbar — scoped to the settings body
     only (Svelte scopes these to .scroll in THIS component, so the palette /
     demo / harness are unaffected). WKWebView honors ::-webkit-scrollbar. The
     thumb is a light grey that reads clearly at rest against the #2d2d2d
     settings bg (not hover-only); the transparent border + background-clip
     insets the thumb so it looks rounded inside the track. */
  .scroll::-webkit-scrollbar {
    width: 24px;
  }
  .scroll::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.05);
  }
  .scroll::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.3);
    border-radius: 10px;
    border: 4px solid transparent;
    background-clip: padding-box;
  }
  .scroll::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.45);
    background-clip: padding-box;
  }
  .scroll::-webkit-scrollbar-thumb:active {
    background: rgba(255, 255, 255, 0.6);
    background-clip: padding-box;
  }
  h2 { font-size: 13px; color: var(--text-dim); margin: 12px 0 2px; }
  .grid-row { display: flex; gap: 8px; align-items: center; }
  input {
    min-height: 40px;
    flex: 1;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text);
    padding: 0 10px;
    font-size: 14px;
  }
  input[type='checkbox'] { min-height: 0; flex: none; width: 20px; height: 20px; }
  input[type='range'] { min-height: 0; padding: 0; }
  input[type='color'] { min-height: 32px; flex: none; width: 48px; padding: 2px; }
  input[type='number'] { flex: 1; min-width: 0; }
  .chk { display: flex; align-items: center; gap: 4px; color: var(--text); font-size: 13px; }
  /* Per-target connection dot (D13/P3), same color semantics as the palette
     dots. Only rendered for active rows present in the running status map. */
  .dot { flex: none; width: 10px; height: 10px; border-radius: 50%; }
  .dot.connected { background: var(--status-active); }
  .dot.lost { background: var(--status-lost); }
  .dot.unknown { background: var(--status-inactive); }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text);
    font-size: 13px;
  }
  .field-inline {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
    font-size: 13px;
  }
  .order { display: flex; gap: 2px; }
  button {
    min-height: 40px;
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text);
    cursor: pointer;
    padding: 0 12px;
  }
  .del { color: var(--status-lost); }
  .add { color: var(--text-dim); }
  .warning { color: var(--status-lost); font-size: 12px; }
</style>
