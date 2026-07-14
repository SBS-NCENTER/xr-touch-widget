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

  /* Windows: the WINDOW is rounded by DWM (round_window_corners in main.rs), so
     fill the glass to the window edge — a CSS radius here would leave a
     two-tone acrylic sliver between the panel's rounder corner and the window
     edge. macOS keeps the CSS radius (its contentView is clipped to match). */
  :global(html.platform-windows) .glass {
    border-radius: 0;
  }
</style>
