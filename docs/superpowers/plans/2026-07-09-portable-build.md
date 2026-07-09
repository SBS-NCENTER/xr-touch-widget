# Portable Build (D15) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a no-install portable Windows build (single exe + adjacent config) alongside the existing installers, and unify the user-facing name to `xr-touch-widget`.

**Architecture:** Add a pure "data base directory" resolver to `xrt-core`: if a `portable.txt` marker sits next to the executable, config (and future logs) live in the exe's own folder (portable); otherwise the per-user OS config dir (installed behavior, unchanged). `main.rs` computes `exe_dir` + `installed_dir` and delegates the choice to that resolver. The portable artifact is the *same* compiled exe + an empty marker, zipped in CI. `productName`/`mainBinaryName` are set so the exe and installer read `xr-touch-widget`.

**Tech Stack:** Rust (edition 2024) + Tauri 2 + Svelte. Config = TOML via `xrt-core::config`. CI = GitHub Actions (`tauri-apps/tauri-action`).

## Global Constraints

- **Single-binary invariant:** the portable exe and the installed exe are the *same* compiled artifact. NO compile-time feature split, NO second binary. Portable vs installed is decided at runtime by the marker file only.
- **`identifier` is frozen:** keep `kr.co.sbs.ncenter.xrt` exactly. It drives `app_config_dir()` (`%APPDATA%\<identifier>\`); changing it orphans existing config. Installed-mode config behavior must stay byte-for-byte as today.
- **Internal crate names unchanged:** `xrt-app`, `xrt-core` stay (never user-visible; `mainBinaryName` decouples the exe name from the crate name).
- **User-facing name = `xr-touch-widget`** everywhere it shows: `productName`, `mainBinaryName`, the exe, the portable zip.
- **Marker = `portable.txt`**, decided by **existence only** — contents ignored.
- **base-dir logic lives in `xrt-core`** (so `cargo test -p xrt-core` covers it on Mac/Linux CI), as a pure function with no Tauri dependency.
- **Dev machine is macOS.** Mac can verify: `xrt-core` unit tests + `xrt-app` compilation. Windows exe naming, the zip, and the on-device run are verified in CI / the pending "Windows 실장비 검증" session — NOT on Mac. Do not claim on-device success from Mac.
- **Commits are manual (project rule):** the implementer does NOT commit. Each task ends with STOP; the user reviews the working-tree diff and runs the commit themselves. Commit commands below are FOR THE USER.

---

## File Structure

- `crates/core/src/paths.rs` — **[create]** — pure data-base-dir resolver: `PORTABLE_MARKER`, `is_portable`, `base_dir`. Owns the portable-vs-installed decision + its unit tests. No Tauri, no I/O beyond a marker existence probe.
- `crates/core/src/lib.rs` — **[modify]** — add `pub mod paths;`.
- `app/src/main.rs` — **[modify]** — replace the config-path resolution block (~line 120) to compute `exe_dir`/`installed_dir` and call `paths::base_dir`.
- `app/tauri.conf.json` — **[modify]** — `productName` → `xr-touch-widget`, add `mainBinaryName`.
- `.github/workflows/release.yml` — **[modify]** — add a Windows-only step that zips `exe + portable.txt` and uploads it to the Release; refresh release name/body.

*(Pre-existence confirmed on 2026-07-09: `crates/core/src/` holds config/heartbeat/net/osc only — `paths.rs` is genuinely new. The other four already exist → modify.)*

---

## Task 1: Data-base-dir resolver in `xrt-core`

**Files:**
- Create: `crates/core/src/paths.rs`
- Modify: `crates/core/src/lib.rs:1-4`
- Test: `crates/core/src/paths.rs` (inline `#[cfg(test)] mod tests`, matching the crate's existing test style in `config.rs`)

**Interfaces:**
- Consumes: nothing (leaf module). `tempfile` is already a dev-dependency of `xrt-core`.
- Produces (later tasks rely on these exact signatures):
  - `pub const PORTABLE_MARKER: &str` (= `"portable.txt"`)
  - `pub fn is_portable(exe_dir: &std::path::Path) -> bool`
  - `pub fn base_dir(exe_dir: &std::path::Path, installed_dir: &std::path::Path) -> std::path::PathBuf`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/paths.rs` with ONLY the module doc + tests (functions not yet defined, so it won't compile — that is the intended "fail"):

```rust
//! Where the app keeps its data (config now, logs later).
//!
//! Two modes, chosen at runtime with zero build-time divergence:
//! - **Portable**: a `portable.txt` marker sits next to the executable →
//!   data lives in the exe's own folder, so the whole folder is copy-portable.
//! - **Installed**: no marker → data lives in the per-user OS config dir
//!   (`app_config_dir()`), the original behavior.
//!
//! The decision is a pure function of two paths, so it unit-tests on any OS
//! without Tauri.

use std::path::{Path, PathBuf};

/// Marker file whose PRESENCE next to the exe switches on portable mode.
/// Contents are ignored. Shipped (empty) inside the portable zip; never
/// installed, so an installed build keeps its `%APPDATA%` behavior.
pub const PORTABLE_MARKER: &str = "portable.txt";

/// True when a `portable.txt` marker sits next to the executable.
pub fn is_portable(exe_dir: &Path) -> bool {
    exe_dir.join(PORTABLE_MARKER).exists()
}

/// The data base directory: the exe's own folder in portable mode, else the
/// installed per-user config dir. `config.toml` (and future logs) go under it.
pub fn base_dir(exe_dir: &Path, installed_dir: &Path) -> PathBuf {
    if is_portable(exe_dir) {
        exe_dir.to_path_buf()
    } else {
        installed_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn not_portable_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_portable(dir.path()));
    }

    #[test]
    fn portable_with_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(PORTABLE_MARKER), "").unwrap();
        assert!(is_portable(dir.path()));
    }

    #[test]
    fn marker_content_is_ignored() {
        // Existence-only: a marker with arbitrary content still counts.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(PORTABLE_MARKER), "anything at all").unwrap();
        assert!(is_portable(dir.path()));
    }

    #[test]
    fn base_dir_is_exe_dir_in_portable_mode() {
        let exe = tempfile::tempdir().unwrap();
        let installed = tempfile::tempdir().unwrap();
        fs::write(exe.path().join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(base_dir(exe.path(), installed.path()).as_path(), exe.path());
    }

    #[test]
    fn base_dir_is_installed_dir_without_marker() {
        let exe = tempfile::tempdir().unwrap();
        let installed = tempfile::tempdir().unwrap();
        assert_eq!(
            base_dir(exe.path(), installed.path()).as_path(),
            installed.path()
        );
    }
}
```

> NOTE: The code block above already contains the finished implementation *and* the tests together. To honor red→green, do Step 1 by pasting ONLY the `#[cfg(test)] mod tests { ... }` block plus the `use std::path::{Path, PathBuf};` line into the new file — omit the three `pub` items — so the tests reference undefined symbols and fail to compile. Then add the `pub` items in Step 3.

- [ ] **Step 2: Register the module and run the tests to verify they fail**

Add `paths` to `crates/core/src/lib.rs` (keep the list alphabetical as it is now):

```rust
pub mod config;
pub mod heartbeat;
pub mod net;
pub mod osc;
pub mod paths;
```

Run: `cargo test -p xrt-core paths`
Expected: FAIL — compile error, `cannot find function 'is_portable'` / `cannot find value 'PORTABLE_MARKER'` (the `pub` items aren't there yet).

- [ ] **Step 3: Add the implementation**

Add the three `pub` items (the doc comment, `PORTABLE_MARKER`, `is_portable`, `base_dir`) shown in Step 1 above the `#[cfg(test)]` block.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xrt-core paths`
Expected: PASS — 5 tests (`not_portable_without_marker`, `portable_with_marker`, `marker_content_is_ignored`, `base_dir_is_exe_dir_in_portable_mode`, `base_dir_is_installed_dir_without_marker`).

Also run the full crate to confirm no regression: `cargo test -p xrt-core`
Expected: PASS — all existing config/net/etc. tests plus the 5 new ones.

- [ ] **Step 5: STOP for review + commit (user runs)**

Implementer stops here — do NOT commit. After the user reviews the working-tree diff, the USER runs:

```bash
git add crates/core/src/paths.rs crates/core/src/lib.rs   # 새 모듈 + lib 등록만 스테이지
git commit -m "feat(core): portable-mode data base dir resolver (D15)"   # -m: 커밋 메시지 인라인
```

---

## Task 2: Wire the resolver into `main.rs` config path

**Files:**
- Modify: `app/src/main.rs:9-10` (imports) and `app/src/main.rs:120-128` (config-path block)

**Interfaces:**
- Consumes: `xrt_core::paths::base_dir(exe_dir: &Path, installed_dir: &Path) -> PathBuf` (Task 1).
- Produces: `config_path: PathBuf` fed into the existing `config::load(&config_path)` (unchanged downstream — `AppState.config_path`, `save_config`, etc. keep working verbatim).

- [ ] **Step 1: Add the `paths` import**

In `app/src/main.rs`, next to the existing core imports (currently lines 9-10):

```rust
use xrt_core::config::{self, Config, LoadOutcome, ValueType};
use xrt_core::net::OscSocket;
use xrt_core::paths;
```

- [ ] **Step 2: Replace the config-path resolution block**

Replace the current block (lines 120-128):

```rust
            let config_path = match app.path().app_config_dir() {
                Ok(dir) => dir.join("config.toml"),
                Err(e) => {
                    eprintln!("failed to resolve app config dir, using temp fallback: {e}");
                    std::env::temp_dir()
                        .join("xr-touch-to-osc")
                        .join("config.toml")
                }
            };
```

with:

```rust
            // Resolve the DATA BASE dir, then place config.toml under it.
            // PORTABLE mode (a `portable.txt` marker next to the exe) keeps
            // config in the exe's own folder so the whole folder is
            // copy-portable; otherwise config lives in the per-user OS config
            // dir (installed behavior, unchanged). Neither branch panics
            // (§8 — the app must come up).
            let installed_dir = match app.path().app_config_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("failed to resolve app config dir, using temp fallback: {e}");
                    std::env::temp_dir().join("xr-touch-to-osc")
                }
            };
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| installed_dir.clone());
            let config_path = paths::base_dir(&exe_dir, &installed_dir).join("config.toml");
```

> Why no new unit test here: this is Tauri glue (needs a live `AppHandle`); the *decision* logic is already unit-tested in Task 1. This task's gate is a clean compile + the deferred on-device smoke, not a new test.

- [ ] **Step 3: Verify it compiles on Mac**

Run: `cargo build -p xrt-app`
Expected: SUCCESS — no errors. (Confirms the import, the `current_exe()`/`parent()` chain, and the `paths::base_dir` call all type-check against Task 1's signatures.)

- [ ] **Step 4: STOP for review + commit (user runs)**

Implementer stops — do NOT commit. After review, the USER runs:

```bash
git add app/src/main.rs                                              # main.rs 변경만 스테이지
git commit -m "feat(app): resolve config via portable-aware base dir (D15)"   # -m: 커밋 메시지 인라인
```

---

## Task 3: Unify the user-facing name to `xr-touch-widget`

**Files:**
- Modify: `app/tauri.conf.json:3` (productName) + add `mainBinaryName`

**Interfaces:**
- Produces: a release binary named `xr-touch-widget.exe` (Task 4's zip step depends on this exact filename) and installers/display named `xr-touch-widget`. `identifier` is untouched, so `app_config_dir()` and all config paths are unchanged.

- [ ] **Step 1: Edit `productName` and add `mainBinaryName`**

Replace the top of `app/tauri.conf.json`:

```json
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "xrt-widget",
  "version": "0.1.0",
  "identifier": "kr.co.sbs.ncenter.xrt",
```

with:

```json
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "xr-touch-widget",
  "mainBinaryName": "xr-touch-widget",
  "version": "0.1.0",
  "identifier": "kr.co.sbs.ncenter.xrt",
```

(Only `productName` changed and `mainBinaryName` added. `version` and `identifier` MUST stay exactly as shown — `identifier` is frozen per Global Constraints.)

- [ ] **Step 2: Verify config still builds**

Run: `cargo build -p xrt-app`
Expected: SUCCESS — `tauri-build` parses the config at compile time; a bad key or malformed JSON would fail here. (The actual exe *rename* to `xr-touch-widget` is applied by the Tauri bundler and is verified on Windows/CI in Task 4 / the on-device session, not by this compile.)

- [ ] **Step 3: STOP for review + commit (user runs)**

Implementer stops — do NOT commit. After review, the USER runs:

```bash
git add app/tauri.conf.json                                         # 이름 변경만 스테이지
git commit -m "chore(app): unify user-facing name to xr-touch-widget (D15)"   # -m: 커밋 메시지 인라인
```

---

## Task 4: Portable zip step in `release.yml`

**Files:**
- Modify: `.github/workflows/release.yml` (add a Windows-only packaging step; refresh release name/body)

**Interfaces:**
- Consumes: `target/release/xr-touch-widget.exe` (produced by the existing `tauri-action` step once Task 3 sets `mainBinaryName`); the Release for the tag (already created by `tauri-action`, `releaseDraft: false`).
- Produces: a Release asset `xr-touch-widget-portable_<version>_x64.zip` containing an `xr-touch-widget/` folder with the exe + an empty `portable.txt`.

- [ ] **Step 1: Refresh the release name/body**

In `.github/workflows/release.yml`, update the `tauri-action` `with:` block:

```yaml
          releaseName: 'XR Touch Widget ${{ github.ref_name }}'
          releaseBody: 'Windows installer (.exe / .msi), Windows portable (.zip), and macOS (.dmg) builds are attached below.'
```

(These two lines replace the existing `releaseName: 'XRT Widget ...'` and `releaseBody: 'Windows (.exe / .msi) and macOS ...'` lines. Leave `projectPath`, `tagName`, `releaseDraft`, `prerelease` as they are.)

- [ ] **Step 2: Add the portable packaging step**

Append this step to the `release` job's `steps:` list, AFTER the `tauri-apps/tauri-action@v0` step (so the Release already exists):

```yaml
      # Portable Windows build: the SAME compiled exe (no installer) + an empty
      # `portable.txt` marker, zipped and attached to the Release. The marker
      # makes the app keep config.toml next to the exe (portable mode), so the
      # unzipped folder is copy-portable. Windows-only; runs after tauri-action
      # has created the Release for this tag.
      - name: Package portable Windows zip
        if: matrix.platform == 'windows-latest'
        shell: pwsh
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          $ver = "${{ github.ref_name }}".TrimStart('v')
          $staging = "portable/xr-touch-widget"
          New-Item -ItemType Directory -Force -Path $staging | Out-Null
          Copy-Item "target/release/xr-touch-widget.exe" "$staging/"
          New-Item -ItemType File -Force -Path "$staging/portable.txt" | Out-Null
          $zip = "xr-touch-widget-portable_${ver}_x64.zip"
          Compress-Archive -Path $staging -DestinationPath $zip -Force
          gh release upload ${{ github.ref_name }} $zip --clobber
```

- [ ] **Step 3: Verify structurally (Mac-side limit)**

This step cannot run on Mac (Windows runner + a real tag push required). Verify what IS checkable here:
- YAML is well-formed and the new step is correctly nested under `steps:` (2-space indent matching siblings). Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml')); print('yaml ok')"`
  Expected: `yaml ok`.
- Confirm the exe path matches Task 3's `mainBinaryName`: the step references `target/release/xr-touch-widget.exe`.

Real end-to-end verification (zip built + attached + unzips + runs portable) happens on a tag push in CI and/or the pending Windows on-device session — see the handoff note below. Do NOT mark this as on-device-verified from Mac.

- [ ] **Step 4: STOP for review + commit (user runs)**

Implementer stops — do NOT commit. After review, the USER runs:

```bash
git add .github/workflows/release.yml                               # 워크플로 변경만 스테이지
git commit -m "ci: attach portable Windows zip to releases (D15)"   # -m: 커밋 메시지 인라인
```

---

## Deferred on-device verification (NOT a Mac task)

The plan's Mac-verifiable surface ends at Task 4 Step 3. The following belong to the pending **"Windows 실장비 검증"** session (STATUS 다음 할 일):

1. Push a version tag → confirm CI attaches `xr-touch-widget-portable_<ver>_x64.zip` to the Release.
2. On the broadcast touchscreen PC: unzip → run `xr-touch-widget.exe` → confirm the palette comes up (WebView2 present).
3. Change a setting → confirm `config.toml` appears **next to the exe** (not in `%APPDATA%`).
4. Copy the whole folder to another path/PC → confirm settings travel with it.
5. Sanity-check the installer path is unaffected: an installed build still writes to `%APPDATA%\kr.co.sbs.ncenter.xrt\`.

## Self-Review (done at write time)

- **Spec coverage:** base-dir resolver (Task 1) ↔ spec §3.1/§3.2; main.rs wiring (Task 2) ↔ §3/§7; name unification incl. `mainBinaryName`+`productName` (Task 3) ↔ §2 부수 결정/§7; portable zip in CI (Task 4) ↔ §4/§7; deferred on-device (handoff note) ↔ §6. YAGNI exclusions (file logging, WebView2 fixed-runtime, read-only media) intentionally have no task, per spec §5. No gaps.
- **Placeholders:** none — every code/step is concrete.
- **Type consistency:** `PORTABLE_MARKER`/`is_portable`/`base_dir` signatures are identical in Task 1's Produces block, its code, and Task 2's call site (`paths::base_dir(&exe_dir, &installed_dir)`).
