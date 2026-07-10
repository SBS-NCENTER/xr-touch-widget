<!-- Glass container. In the Tauri window the OS supplies the behind-blur
     (window-vibrancy); backdrop-filter below only matters in browser
     contexts (demo page, dev harness) where it blurs page content. -->
<div class="glass">
  <slot />
</div>

<style>
  .glass {
    background: var(--glass-bg);
    border: 1px solid var(--glass-border);
    border-radius: var(--radius);
    box-shadow:
      inset 0 1px 0 var(--glass-highlight),
      0 8px 32px rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(18px) saturate(1.3);
    -webkit-backdrop-filter: blur(18px) saturate(1.3);
    color: var(--text);
  }

  /* Windows runs an OPAQUE window (transparent WebView2 fails to composite its
     content on Win11 — the palette rendered blank). Swap the translucent glass
     for a solid tint filling the whole window: no behind-blur to reveal, no
     rounded corners exposing the opaque window backing. macOS keeps the glass. */
  :global(html.platform-windows) .glass {
    background: var(--glass-bg-opaque, #141820);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    border: none;
    border-radius: 0;
    box-shadow: none;
  }
</style>
