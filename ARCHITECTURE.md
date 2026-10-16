# ARCHITECTURE.md — the codemap

What is where, and what may depend on what. A map of the present: a change
rewrites the cell it touches.

```text
a Windows PC with a touchscreen (the target)                 the XR engine
┌──────────────────────────────────────────────────┐   OSC/UDP   ┌──────────┐
│ app/  xrt-app, the Tauri shell: main.rs, engine.rs│────────────►│  engine  │
│       (frameless, always on top, vibrancy;        │  HTTP GET   │ or a URL │
│        portable or installed build)               │             └──────────┘
│ ui/  (Svelte + Vite, served inside the shell)     │
│   widget/   Palette.svelte, Settings.svelte       │             └──────────┘
│   shared/   GlassPanel.svelte, tokens.css         │                  ▲
│   demo/, harness/  the demo page and the test     │                  │ on this box:
│             harness                               │                  │ crates/mock-xr
│   widget.html, settings.html, demo.html,          │                  │ (loopback tests)
│   harness.html                                    │
└──────────────────────┬───────────────────────────┘
                       │ the shell's commands (ipc.js)
                       ▼
┌──────────────────────────────────────────────────┐
│ crates/core  (xrt-core)                           │
│   config.rs   the settings: targets, buttons as   │
│               ordered action lists                │
│   osc.rs      the OSC encoding; http.rs the GET   │
│   net.rs      the UDP sender                      │
│   heartbeat.rs the engine's heartbeat             │
└──────────────────────────────────────────────────┘
```

| Directory | Holds | Depends on |
| --- | --- | --- |
| `crates/core` | the logic above, its unit tests | the workspace's dependencies |
| `crates/mock-xr` | the engine's stand-in and the loopback test | `xrt-core` |
| `app/` | the Tauri shell `xrt-app`: the window, the engine thread, the commands, `tauri.conf.json`, the capabilities | `xrt-core`, tauri 2, window-vibrancy |
| `ui/` | the pages and components, built by Vite to `ui/dist/`; `package.json` at the root delegates to it | svelte, vite, `@tauri-apps/api` |
| `.github/workflows/build.yml`, `release.yml` | the Windows installer on a push to `main`, the Release on a `v*` tag | windows-latest |
| `doc/` | the design in force, the plan | — |
| `scripts/`, `.claude/`, `roles/`, `git-hooks/` | the harness and the check | `chun-harness`, their source |
