# HTTP(URL) Action (D16) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A button press fires an ORDERED LIST of actions — the existing OSC message and/or a full-URL HTTP GET (Pixotope Gateway camera cuts) — with a 1.5s red flash on the pressed button when any action fails.

**Architecture:** `xrt-core::config` gains an internally-tagged `Action` enum (`osc` | `http`) and `ButtonDef` becomes `{label, actions}`, with pre-D16 flat configs auto-migrated at deserialization via a serde shadow struct. A new `xrt-core::http` module wraps a blocking `ureq` GET (testable with a plain `TcpListener`). The engine's `Trigger` command becomes `Press { index }`: it resolves `buttons[index]` from ITS running config and fires actions in order — OSC inline (UDP), HTTP on a spawned thread so the engine/heartbeat loop never stalls on a dead gateway. Failures emit `xrt://press-error` which the palette renders as a red flash. The settings window edits the per-button action list (new actions default to `http` — "URL 우선").

**Tech Stack:** Rust (edition 2024) + Tauri 2 + Svelte 5 (runes). Config = TOML via `xrt-core::config`. HTTP = `ureq` 2 (blocking, `default-features = false`).

**Spec:** `docs/superpowers/specs/2026-07-15-http-action-design.md`

## Global Constraints

- **Commits are manual (project rule):** the implementer does NOT commit. Each task ends with STOP; the user reviews the working-tree diff and commits themselves. Commit *messages* are given as text (the user types them in their editor) — never as `git commit -m` commands.
- **Backward compat is non-negotiable:** every pre-D16 `config.toml` (flat `address`/`value`/`value_type` per button, incl. label-only buttons and pre-D14 `graphic_id`/`type` keys) must load and behave exactly as before — enforced by unit tests. Save always writes the NEW shape only.
- **Engine thread never blocks on HTTP:** `Action::Http` is always dispatched on a spawned thread. The engine loop (50ms poll + heartbeat) continues immediately.
- **Fixed public names** (cross-task contract): Tauri command `press(index: usize)`; event `xrt://press-error` with payload `{ button_index: usize, detail: String }`; config action tags `"osc"` / `"http"` (lowercase).
- **HTTP verdict:** `Ok` from ureq (2xx, redirects auto-followed) = success; HTTP error status / transport error / timeout / bad URL = failure with a reason string. Timeout = 3s constant (`HTTP_TIMEOUT` in engine; `http::get` takes it as a parameter for testability).
- **`ureq = { version = "2", default-features = false }`** — plain-http only (field gear is private-LAN `http://`); enabling TLS later is a feature flag away.
- **UI enforces actions ≥ 1 per button** at [적용] — this is what keeps the "empty actions = legacy shape → migrate" rule safe (spec §4.2).
- **Dev machine is macOS.** Mac verifies: `cargo test --workspace`, `cargo check -p xrt-app`, `npm run build`, and a local-HTTP smoke (`python3 -m http.server`). Real Pixotope camera-cut verification happens on the Windows broadcast PC in a later session — do NOT claim it from Mac.
- **Match existing style:** Svelte 5 runes (`$state`/`$derived`/`$effect`), existing CSS voice, comment density and Korean UI copy as in the current files.

---

## File Structure

- `crates/core/src/config.rs` — **[modify]** — `Action` enum, `ButtonDef {label, actions}`, `ButtonDefCompat` shadow (serde(from) migration), tests.
- `crates/core/src/http.rs` — **[create]** — `pub fn get(url, timeout) -> Result<(), String>` + TcpListener tests.
- `crates/core/src/lib.rs` — **[modify]** — add `pub mod http;`.
- `crates/core/Cargo.toml` — **[modify]** — add `ureq` dependency.
- `app/src/engine.rs` — **[modify]** — `EngineCmd::Press`, action dispatch, `PRESS_ERROR_EVENT` + `PressError`.
- `app/src/main.rs` — **[modify]** — `press` command replaces `trigger`; drop unused `ValueType` import.
- `ui/src/widget/ipc.js` — **[modify]** — `press(index)` + `onPressError(cb)` replace `trigger(...)`; mockConfig buttons → new shape.
- `ui/src/widget/Palette.svelte` — **[modify]** — press by index; `xrt://press-error` red flash.
- `ui/src/widget/Settings.svelte` — **[modify]** — per-button action-list editor + validation rewrite.
- `app/tauri.conf.json` — **[modify]** — version 0.2.1 → 0.3.0 (final task).

*(Pre-existence confirmed on 2026-07-15 against HEAD: `crates/core/src/http.rs` is genuinely new — `git cat-file -e HEAD:crates/core/src/http.rs` fails. All other paths exist → modify.)*

---

## Task 1: `Action` enum + `ButtonDef.actions` + legacy migration (core config)

**Files:**
- Modify: `crates/core/src/config.rs` (ButtonDef block ~lines 42-77; tests ~lines 202-467)
- Test: same file, inline `#[cfg(test)] mod tests` (crate's existing style)

**Interfaces:**
- Consumes: existing `ValueType` (unchanged), `default_address()` (unchanged).
- Produces (later tasks rely on these EXACT shapes):
  - `pub enum Action { Osc { address: String, value_type: ValueType, value: String }, Http { url: String } }` — derives `Debug, Clone, Serialize, Deserialize, PartialEq`; serde `tag = "type", rename_all = "lowercase"` → JSON/TOML shape `{ type: "osc", address, value_type, value }` / `{ type: "http", url }`.
  - `pub struct ButtonDef { pub label: String, pub actions: Vec<Action> }` — after ANY deserialization, `actions` is never empty (legacy/empty folds into one Osc action).

- [ ] **Step 1: Write the failing tests**

In `crates/core/src/config.rs`, REPLACE these four existing tests — `save_then_load_roundtrips`, `button_fields_default_when_absent`, `legacy_button_keys_are_ignored_and_new_fields_default`, `button_value_type_roundtrips_each_variant` — and ADD the four new ones, so the tests block reads:

```rust
    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.targets.push(Target {
            name: "XR-1".into(),
            ip: "192.168.0.10".into(),
            active: true,
        });
        c.buttons.push(ButtonDef {
            label: "그래픽 A".into(),
            actions: vec![Action::Osc {
                address: "/xrt/graphic".into(),
                value_type: ValueType::String,
                value: "lower_third_a".into(),
            }],
        });
        c.appearance = AppearanceConfig {
            bg_opacity: 0.4,
            button_opacity: 0.12,
            accent: "#ff8800".into(),
            bg_tint: "#202020".into(),
            highlight_last: false,
            highlight_color: "#00ff00".into(),
            highlight_opacity: 0.5,
        };
        c.window = WindowConfig {
            width: 900,
            height: 120,
        };
        save(&path, &c).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, c);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    // --- D16: buttons carry an ordered action list (osc | http) ---

    #[test]
    fn mixed_actions_roundtrip_with_url_verbatim() {
        // One button, TWO actions (http first — "URL 우선"), through
        // save→load. The Pixotope-style URL must survive byte-for-byte
        // (query string included).
        let url = "http://10.10.204.184:16208/gateway/25.2.4/publish?Type=Call&Target=Store&Method=SetCameraSet&ParamNumber=0";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.buttons.push(ButtonDef {
            label: "CAM 1".into(),
            actions: vec![
                Action::Http { url: url.into() },
                Action::Osc {
                    address: "/xrt/graphic".into(),
                    value_type: ValueType::String,
                    value: "cam1_lower".into(),
                },
            ],
        });
        save(&path, &c).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, c);
        assert_eq!(
            loaded.buttons[0].actions[0],
            Action::Http { url: url.into() }
        );
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn legacy_flat_button_migrates_to_single_osc_action() {
        // A pre-D16 config.toml: flat OSC fields directly on the button.
        let toml_str = r#"
            [[buttons]]
            label = "A"
            address = "/xrt/graphic"
            value = "graphic_a"
            value_type = "string"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].label, "A");
        assert_eq!(
            c.buttons[0].actions,
            vec![Action::Osc {
                address: "/xrt/graphic".into(),
                value_type: ValueType::String,
                value: "graphic_a".into(),
            }]
        );
    }

    #[test]
    fn label_only_button_migrates_to_default_osc_action() {
        // Pre-D16 semantics preserved: a label-only [[buttons]] entry fired
        // the default /xrt/graphic message — after migration it still does.
        let toml_str = r#"
            [[buttons]]
            label = "A"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].label, "A");
        assert_eq!(
            c.buttons[0].actions,
            vec![Action::Osc {
                address: "/xrt/graphic".into(),
                value_type: ValueType::String,
                value: "".into(),
            }]
        );
    }

    #[test]
    fn legacy_button_keys_are_ignored_and_new_fields_default() {
        // A pre-D14 config.toml carried `graphic_id` + `type`. Those are now
        // unknown fields — the compat shape has NO deny_unknown_fields, so
        // serde silently ignores them and the flat-field migration still
        // produces the default OSC action.
        let toml_str = r#"
            [[buttons]]
            label = "A"
            graphic_id = "a"
            type = "trigger"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].label, "A");
        assert_eq!(
            c.buttons[0].actions,
            vec![Action::Osc {
                address: "/xrt/graphic".into(),
                value_type: ValueType::String,
                value: "".into(),
            }]
        );
    }

    #[test]
    fn explicitly_empty_actions_folds_to_default_osc() {
        // `actions = []` (hand-edited TOML — the settings UI never saves
        // this, it enforces ≥1 action) is indistinguishable from "absent"
        // and folds to the default OSC action the same way. Documents the
        // spec §4.2 edge explicitly.
        let toml_str = r#"
            [[buttons]]
            label = "A"
            actions = []
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].actions.len(), 1);
    }

    #[test]
    fn saved_button_toml_has_no_legacy_flat_keys() {
        // save() must write the NEW shape only: no flat address/value/
        // value_type keys left at the button level (they live inside
        // [[buttons.actions]] entries now).
        let mut c = Config::default();
        c.buttons.push(ButtonDef {
            label: "A".into(),
            actions: vec![Action::Osc {
                address: "/a".into(),
                value_type: ValueType::Int,
                value: "1".into(),
            }],
        });
        let text = toml::to_string_pretty(&c).unwrap();
        let value: toml::Value = toml::from_str(&text).unwrap();
        let button = &value["buttons"][0];
        assert!(button.get("label").is_some());
        assert!(button.get("actions").is_some());
        assert!(button.get("address").is_none());
        assert!(button.get("value").is_none());
        assert!(button.get("value_type").is_none());
    }

    #[test]
    fn button_value_type_roundtrips_each_variant() {
        for vt in [
            ValueType::None,
            ValueType::String,
            ValueType::Int,
            ValueType::Float,
            ValueType::Bool,
        ] {
            let mut c = Config::default();
            c.buttons.push(ButtonDef {
                label: "L".into(),
                actions: vec![Action::Osc {
                    address: "/a/b".into(),
                    value_type: vt.clone(),
                    value: "1".into(),
                }],
            });
            let text = toml::to_string_pretty(&c).unwrap();
            let back: Config = toml::from_str(&text).unwrap();
            assert_eq!(
                back.buttons[0].actions[0],
                c.buttons[0].actions[0]
            );
        }
    }
```

(The old `button_fields_default_when_absent` test is REPLACED by `label_only_button_migrates_to_default_osc_action` — same scenario, new shape.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xrt-core`
Expected: COMPILE ERROR — `ButtonDef` has no field `actions`, `Action` not found. (The construction sites in the tests reference the not-yet-existing shape; a compile failure IS the failing state for a schema change.)

- [ ] **Step 3: Implement `Action` + new `ButtonDef` + compat migration**

In `crates/core/src/config.rs`, REPLACE the current `ButtonDef` block (the doc comment + struct, currently below `ValueType`) with:

```rust
/// One action a trigger button fires (D16, 2026-07-15). `Osc` is the D14
/// message spec (address + a single typed value); `Http` is a full URL hit
/// with GET (Pixotope Gateway-style control — the operator pastes the same
/// URL a browser would use). Internally tagged, so a config.toml entry reads
/// `type = "http"` + its fields, and the JS side sees `{ type: "http", url }`
/// — one shape everywhere.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    Osc {
        #[serde(default = "default_address")]
        address: String,
        #[serde(default)]
        value_type: ValueType,
        #[serde(default)]
        value: String,
    },
    Http {
        #[serde(default)]
        url: String,
    },
}

/// One trigger button (D16, 2026-07-15): a label + an ORDERED list of
/// actions fired per press (dispatch order — responses are not awaited).
/// Deserialization goes through `ButtonDefCompat` (serde(from)), so a
/// pre-D16 config.toml — flat `address`/`value`/`value_type` on the button —
/// still loads: the flat fields fold into a single Osc action. `save()`
/// always writes the new shape only. After ANY deserialization `actions` is
/// non-empty (the settings UI enforces ≥1 action, and empty/absent lists
/// fold to the default OSC action — the pre-D16 behavior of a bare button).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "ButtonDefCompat")]
pub struct ButtonDef {
    pub label: String,
    pub actions: Vec<Action>,
}

/// Accepts BOTH button shapes on load (D16): the new `actions` list and the
/// pre-D16 flat OSC fields (with their D14 per-field defaults). Unknown
/// legacy keys (pre-D14 `graphic_id`/`type`) stay ignored as before — no
/// deny_unknown_fields. Never serialized; `ButtonDef` serializes itself.
#[derive(Deserialize)]
struct ButtonDefCompat {
    #[serde(default)]
    label: String,
    #[serde(default)]
    actions: Vec<Action>,
    #[serde(default = "default_address")]
    address: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    value_type: ValueType,
}

impl From<ButtonDefCompat> for ButtonDef {
    fn from(c: ButtonDefCompat) -> Self {
        // An empty/absent actions list marks a pre-D16 (or hand-emptied)
        // button: fold the flat fields into a single Osc action, preserving
        // pre-D16 behavior exactly (a label-only button still fires the
        // default /xrt/graphic message).
        let actions = if c.actions.is_empty() {
            vec![Action::Osc {
                address: c.address,
                value_type: c.value_type,
                value: c.value,
            }]
        } else {
            c.actions
        };
        ButtonDef { label: c.label, actions }
    }
}
```

`default_address()` stays exactly as-is (the enum and the compat shape both use it).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p xrt-core`
Expected: PASS — all config tests green, including the 5 new D16 tests. No other test in the crate touches `ButtonDef`.

Then: `cargo test --workspace`
Expected: PASS — `xrt-app` still compiles unchanged (neither engine.rs nor main.rs reads `ButtonDef` fields; the old `EngineCmd::Trigger` carries the address/value directly). Do NOT touch the app crate in this task — Task 3 owns those files.

- [ ] **Step 5: STOP for review + commit (user runs)**

Implementer stops here — do NOT commit. After the user reviews the working-tree diff, the USER stages and commits:

```bash
git add crates/core/src/config.rs   # Task 1의 유일한 변경 파일
git commit                          # 에디터가 열리면 아래 메시지 입력
```

Commit message (title + body, typed in the editor):

```
feat(core): button = ordered action list (D16)

Action enum(osc|http, internally tagged) 도입, ButtonDef를
{label, actions}로 변경. pre-D16 flat 필드는 serde(from) 섀도
구조체로 로드 시 자동 마이그레이션(label-only 포함), 저장은
새 형식만. Pixotope URL 쿼리스트링 보존 roundtrip 테스트 포함.
```

---

## Task 2: `xrt-core::http` — blocking GET with verdict

**Files:**
- Create: `crates/core/src/http.rs`
- Modify: `crates/core/src/lib.rs:1-4` (add `pub mod http;`)
- Modify: `crates/core/Cargo.toml` (add `ureq`)
- Test: `crates/core/src/http.rs` inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing from other tasks (leaf module).
- Produces (Task 3 relies on this EXACT signature):
  - `pub fn get(url: &str, timeout: std::time::Duration) -> Result<(), String>`

- [ ] **Step 1: Add the dependency and module registration**

In `crates/core/Cargo.toml` `[dependencies]`, add:

```toml
# D16 HTTP actions: plain-http only (field gear is private-LAN http://) —
# no TLS stack. Blocking by design; the engine spawns a thread per call.
ureq = { version = "2", default-features = false }
```

In `crates/core/src/lib.rs`:

```rust
pub mod config;
pub mod heartbeat;
pub mod http;
pub mod net;
pub mod osc;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/core/src/http.rs` with the module doc + tests only (no `get` yet — compile failure is the failing state):

```rust
//! Minimal HTTP GET for button actions (D16). One call = one request:
//! Ok from ureq (2xx, redirects auto-followed) = success; an HTTP error
//! status, transport error, timeout or bad URL = Err(reason). The response
//! body is ignored — the caller only needs fired-or-failed.
//! Lives in core (no Tauri) so it unit-tests against a plain TcpListener,
//! the same philosophy as mock-xr for the OSC path.

use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const T: Duration = Duration::from_millis(500);

    /// One-shot local HTTP server answering the first request with `response`.
    fn serve_once(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        // Query string included: the Pixotope publish URL shape must pass
        // through ureq untouched.
        format!("http://{addr}/gateway/25.2.4/publish?Type=Call&ParamNumber=0")
    }

    #[test]
    fn ok_on_200() {
        let url = serve_once("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n[]");
        assert_eq!(get(&url, T), Ok(()));
    }

    #[test]
    fn err_on_500_with_status_in_reason() {
        let url =
            serve_once("HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\n\r\n");
        let err = get(&url, T).unwrap_err();
        assert!(err.contains("500"), "want status in reason, got: {err}");
    }

    #[test]
    fn err_on_connection_refused() {
        // Bind to grab a free port, then drop the listener so nothing answers.
        let addr = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        assert!(get(&format!("http://{addr}/"), T).is_err());
    }

    #[test]
    fn err_on_timeout() {
        // Accepts and reads, then never responds — get() must give up at
        // `timeout` (200ms here so the test stays fast), not hang.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(2)); // > timeout
            }
        });
        assert!(get(&format!("http://{addr}/"), Duration::from_millis(200)).is_err());
    }

    #[test]
    fn err_on_bad_url() {
        assert!(get("not a url", T).is_err());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p xrt-core http`
Expected: COMPILE ERROR — `get` not found in this scope.

- [ ] **Step 4: Implement `get`**

Insert between the `use std::time::Duration;` line and the tests module:

```rust
/// Fire a GET at `url`. Blocking — callers that must not stall (the engine
/// loop) run it on a spawned thread. `timeout` covers the whole call
/// (connect + response); the engine passes its HTTP_TIMEOUT constant, tests
/// pass short values.
pub fn get(url: &str, timeout: Duration) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    match agent.get(url).call() {
        Ok(_) => Ok(()),
        // 4xx/5xx arrive as Error::Status — surface the code (the operator
        // sees it in the log; the palette only needs "failed").
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        // Transport-level: refused, DNS, timeout, malformed URL, ...
        Err(e) => Err(e.to_string()),
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p xrt-core http`
Expected: PASS — 5 tests (`ok_on_200`, `err_on_500_with_status_in_reason`, `err_on_connection_refused`, `err_on_timeout`, `err_on_bad_url`).

- [ ] **Step 6: STOP for review + commit (user runs)**

```bash
git add crates/core/src/http.rs crates/core/src/lib.rs crates/core/Cargo.toml Cargo.lock   # 새 모듈 + 등록 + 의존성(lock 포함)
git commit
```

Commit message:

```
feat(core): http GET helper for button actions (D16)

ureq 2(default-features=false, http 전용) 기반 blocking GET.
Ok(2xx)=성공, 상태에러/전송에러/timeout/URL불량=Err(사유).
timeout은 파라미터(테스트 200ms, engine 3s). TcpListener
미니서버로 200/500/refused/timeout/bad-url 단위테스트.
```

---

## Task 3: Engine `Press` + `xrt://press-error` + `press` command

**Files:**
- Modify: `app/src/engine.rs` (imports, `EngineCmd`, constants, the `Trigger` match arm)
- Modify: `app/src/main.rs:9` (use line), `:44-60` (trigger → press), `:131-138` (invoke_handler)

**Interfaces:**
- Consumes: `xrt_core::config::Action` (Task 1), `xrt_core::http::get` (Task 2).
- Produces (Task 4 relies on these EXACT names/shapes):
  - Tauri command `press(index: usize)` (invoke payload key: `index`).
  - Event `xrt://press-error`, payload `{ "button_index": usize, "detail": String }`.

- [ ] **Step 1: Rewrite `engine.rs` command surface**

Replace the imports block at the top of `app/src/engine.rs`:

```rust
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use xrt_core::config::{Action, Config, Target};
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::http;
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;
```

Replace the `EngineCmd` enum:

```rust
pub enum EngineCmd {
    /// One button press (D16). The engine resolves `buttons[index]` from ITS
    /// copy of the config — the running truth — so a press can never fire a
    /// stale action list from the UI's copy. Actions fire in list order:
    /// Osc inline (UDP is effectively instant), Http on a spawned thread so
    /// a dead gateway's timeout can never stall this loop (heartbeat
    /// included). Responses are NOT awaited between actions.
    Press { index: usize },
    UpdateConfig(Config),
}
```

Below `pub const STATUS_EVENT: &str = "xrt://status";` add:

```rust
/// Per-press failure feed for the palette's red flash (D16). Emitted for
/// EVERY failed action of a press — OSC send errors and HTTP failures alike.
pub const PRESS_ERROR_EVENT: &str = "xrt://press-error";

/// How long an HTTP action waits (connect + response) before reporting
/// failure. Constant by design (YAGNI — spec §5).
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize, Clone)]
pub struct PressError {
    pub button_index: usize,
    pub detail: String,
}
```

Replace the `Ok(EngineCmd::Trigger { .. })` match arm in the loop with:

```rust
                Ok(EngineCmd::Press { index }) => {
                    if let Some(btn) = config.buttons.get(index) {
                        for action in btn.actions.clone() {
                            match action {
                                Action::Osc { address, value_type, value } => {
                                    for report in socket.send_trigger(
                                        &address,
                                        value_type,
                                        &value,
                                        &config.targets,
                                        config.network.ue_port,
                                    ) {
                                        if !report.ok {
                                            let ip = &report.ip;
                                            let e = report.error.as_deref().unwrap_or("unknown error");
                                            eprintln!("OSC send to {ip} failed: {e}");
                                            hb.mark_lost(&report.ip);
                                            // D16: a failed send now also flashes
                                            // the pressed button red, not just logs.
                                            let _ = app.emit(
                                                PRESS_ERROR_EVENT,
                                                &PressError {
                                                    button_index: index,
                                                    detail: format!("OSC → {ip}: {e}"),
                                                },
                                            );
                                        }
                                    }
                                }
                                Action::Http { url } => {
                                    // Fire on a fresh thread: blocking here would
                                    // stall heartbeat + subsequent presses for up
                                    // to HTTP_TIMEOUT per dead gateway. Presses
                                    // are operator-paced, so thread cost is nil.
                                    let app = app.clone();
                                    std::thread::spawn(move || {
                                        if let Err(e) = http::get(&url, HTTP_TIMEOUT) {
                                            eprintln!("HTTP GET {url} failed: {e}");
                                            let _ = app.emit(
                                                PRESS_ERROR_EVENT,
                                                &PressError {
                                                    button_index: index,
                                                    detail: format!("HTTP: {e}"),
                                                },
                                            );
                                        }
                                    });
                                }
                            }
                        }
                    } else {
                        // Stale UI (button list changed mid-press) — drop it.
                        eprintln!(
                            "press: button index {index} out of range ({} buttons)",
                            config.buttons.len()
                        );
                    }
                }
```

- [ ] **Step 2: Swap the Tauri command in `main.rs`**

Line 9 — drop the now-unused `ValueType`:

```rust
use xrt_core::config::{self, Config, LoadOutcome};
```

Replace the whole `trigger` command (lines 44-60) with:

```rust
#[tauri::command]
fn press(state: State<AppState>, index: usize) {
    // D16: the UI sends only the button INDEX; the engine resolves the
    // action list from its own (running) config, so a press can never fire
    // a stale action list from the UI's copy.
    let _ = state.engine_tx.send(engine::EngineCmd::Press { index });
}
```

In `invoke_handler` replace `trigger` with `press`:

```rust
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            press,
            load_warning,
            open_settings,
            quit_app
        ])
```

- [ ] **Step 3: Verify the workspace compiles and tests pass**

Run: `cargo test --workspace`
Expected: PASS — core tests (incl. Task 1/2 additions) green; `xrt-app` compiles clean with no warnings about unused imports (ValueType removed).

Run: `cargo check -p xrt-app`
Expected: clean.

- [ ] **Step 4: STOP for review + commit (user runs)**

```bash
git add app/src/engine.rs app/src/main.rs   # engine 실행모델 + command 교체
git commit
```

Commit message:

```
feat(app): press command fires action list (D16)

trigger(주소/값 직접 전달) → press(index)로 교체 — engine이
자기 config에서 buttons[index].actions를 순서대로 발사(OSC는
inline UDP, HTTP는 액션당 thread spawn, timeout 3s). 실패는
xrt://press-error{button_index,detail}로 emit — OSC 전송실패도
동일 배선(기존엔 로그만). index 범위 밖은 drop+로그.
```

---

## Task 4: `ipc.js` + `Palette.svelte` — press by index + red error flash

**Files:**
- Modify: `ui/src/widget/ipc.js` (mockConfig buttons ~line 15-19; `trigger` fn ~line 42-46; new `onPressError`)
- Modify: `ui/src/widget/Palette.svelte` (imports ~line 3-15; `press` fn ~line 188-194; `$effect` subscriptions ~line 159-186; button markup ~line 417-426; CSS)

**Interfaces:**
- Consumes: command `press(index)` + event `xrt://press-error` (Task 3).
- Produces (Task 5's mock-driven manual checks rely on these):
  - `ipc.press(index)` — invokes `press` with `{ index }`.
  - `ipc.onPressError(cb)` — cb receives `{ button_index, detail }`; returns unlisten fn.
  - mockConfig buttons in the NEW shape (`{label, actions: [...]}`).

- [ ] **Step 1: Update `ipc.js`**

Replace the mockConfig `buttons` array with (keeps the harness true to production shape — D16):

```js
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
```

Replace the `trigger` export with:

```js
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
```

- [ ] **Step 2: Update `Palette.svelte`**

Imports — replace `trigger` with `press` and add `onPressError`:

```js
  import {
    getConfig,
    saveConfig,
    press,
    onPressError,
    openSettings,
    loadWarning,
    onStatus,
    onConfigChanged,
    onAppearancePreview,
    onWindowMoved,
    setSize,
    innerSize,
  } from './ipc.js';
```

State — next to `flashIndex` add:

```js
  // D16: which button INDEX is currently flashing red after a failed action
  // (xrt://press-error). Runtime-only; a newer failure retargets the flash.
  let errorIndex = $state(null);
  let errorTimer = null;
```

Local press handler — replace the existing `press(btn, i)` function (the imported `press` now owns that name):

```js
  async function pressButton(i) {
    flashIndex = i;
    lastPressedIndex = i;
    setTimeout(() => (flashIndex = null), 250);
    // D16: send only the index — the engine resolves buttons[i].actions
    // from the running config and fires them in order.
    await press(i);
  }
```

Inside the `$effect`'s async block, after the `onAppearancePreview` subscription line, add:

```js
      // D16: any failed action of a press (OSC or HTTP) flashes that button
      // red for 1.5s. A newer failure retargets and restarts the flash.
      unsubs.push(
        await onPressError((payload) => {
          errorIndex = payload.button_index;
          if (errorTimer) clearTimeout(errorTimer);
          errorTimer = setTimeout(() => (errorIndex = null), 1500);
        }),
      );
```

Button markup — add the error class and the renamed handler:

```svelte
          <button
            class="trig"
            class:flash={flashIndex === i}
            class:press-error={errorIndex === i}
            class:last-pressed={highlightLast && lastPressedIndex === i}
            onclick={() => pressButton(i)}
          >
            <span class="label">{btn.label}</span>
          </button>
```

CSS — after the `.trig:active, .trig.flash` rule add:

```css
  /* D16: press-failure flash — a failed action (OSC send error or HTTP
     failure) paints the button red for 1.5s, driven by xrt://press-error.
     Deliberately louder than .flash so a dead camera cut can't be missed
     mid-broadcast; wins over .flash/.active because it comes later in the
     cascade at equal specificity. */
  .trig.press-error {
    border-color: var(--status-lost);
    background: rgba(255, 70, 70, 0.35);
  }
```

(Note: place it AFTER the `.trig:active, .trig.flash` rule so the red wins while both classes are present.)

- [ ] **Step 3: Verify the UI builds**

Run: `npm run build`
Expected: vite builds all 4 entries (widget, settings, demo, harness) with no errors. (Svelte's compiler will also flag unused-import mistakes here.)

- [ ] **Step 4: Manual harness spot-check (browser, no Tauri)**

Run: `npm run dev` → open the printed URL's harness page → the palette shows CAM 1 / 그래픽 A / CLEAR from the new-shape mock; clicking logs `[mock] press 0` in the console. (No red flash in the browser — there's no event source outside Tauri; that's expected.)

- [ ] **Step 5: STOP for review + commit (user runs)**

```bash
git add ui/src/widget/ipc.js ui/src/widget/Palette.svelte   # press 배선 + 빨간 깜박임
git commit
```

Commit message:

```
feat(ui): press by index + press-error red flash (D16)

팔레트 버튼이 press(index)만 보내고(액션 해석은 engine),
xrt://press-error 구독으로 실패 버튼을 1.5초 빨갛게 표시.
mockConfig 버튼을 새 shape({label, actions[]})로 갱신 —
harness가 프로덕션과 동일 스키마 유지.
```

---

## Task 5: `Settings.svelte` — per-button action-list editor + validation

**Files:**
- Modify: `ui/src/widget/Settings.svelte` (script: `addButton`/`valueInvalid`/`validateButtons` ~lines 127-204; markup: the `버튼 (OSC 메시지)` section ~lines 288-351; CSS additions)

**Interfaces:**
- Consumes: config button shape `{label, actions: [{type:'http', url} | {type:'osc', address, value, value_type}]}` (Task 1, via get_config/save_config serde).
- Produces: draft edits that always satisfy "actions ≥ 1 per button" at [적용] (the migration-safety invariant, spec §4.2).

- [ ] **Step 1: Script changes**

Replace `addButton` and add the action helpers (after `removeButton`/`moveButton`, before `I32_MIN`):

```js
  function addButton() {
    // D16: a new button starts with ONE empty URL action — "URL 우선".
    draft.buttons.push({ label: '', actions: [{ type: 'http', url: '' }] });
  }
  function removeButton(i) {
    draft.buttons.splice(i, 1);
  }
  function moveButton(i, delta) {
    const j = i + delta;
    if (j < 0 || j >= draft.buttons.length) return;
    [draft.buttons[i], draft.buttons[j]] = [draft.buttons[j], draft.buttons[i]];
  }
  function addAction(b) {
    // New actions default to http too (D16 "URL 우선").
    b.actions.push({ type: 'http', url: '' });
  }
  function removeAction(b, j) {
    b.actions.splice(j, 1);
  }
  /** Swap an action's type IN PLACE, resetting to that type's defaults —
   *  the two shapes share no fields, so nothing meaningful carries across. */
  function setActionType(b, j, type) {
    if (b.actions[j].type === type) return;
    b.actions[j] =
      type === 'http'
        ? { type: 'http', url: '' }
        : { type: 'osc', address: '/xrt/graphic', value: '', value_type: 'string' };
  }
```

(`removeButton`/`moveButton` are unchanged — shown for placement only.)

Change `valueInvalid` to take an ACTION (same body — the fields moved from the button onto the osc action):

```js
  /** Inline feedback: true when an osc action's value doesn't parse for its
   *  current value_type, so the value widget can flag itself red BEFORE
   *  [적용]. Re-runs reactively whenever a.value or a.value_type changes. */
  function valueInvalid(a) {
    return !valueParses(a.value_type, a.value);
  }

  /** Inline feedback for http actions: red until the URL is a plausible
   *  http(s) URL — same rule the apply-time validator enforces. */
  function urlInvalid(a) {
    const url = (a.url ?? '').trim();
    return url === '' || !/^https?:\/\//.test(url);
  }
```

Replace `validateButtons` entirely:

```js
  /** Validate the button list before it can be saved (D14 rules per osc
   *  action + D16 structure rules). Returns a Korean error message for the
   *  FIRST invalid entry, or null if all are OK. `actions ≥ 1` is the
   *  migration-safety invariant (an empty list in a saved config would read
   *  as the legacy shape on the Rust side — spec §4.2). */
  function validateButtons(buttons) {
    for (let i = 0; i < buttons.length; i++) {
      const b = buttons[i];
      const n = i + 1;
      if (!b.actions || b.actions.length === 0)
        return `버튼 ${n}: 액션이 최소 1개 필요합니다`;
      for (let j = 0; j < b.actions.length; j++) {
        const a = b.actions[j];
        const m = j + 1;
        if (a.type === 'http') {
          const url = (a.url ?? '').trim();
          if (url === '') return `버튼 ${n} 액션 ${m}: URL을 입력하세요`;
          if (!/^https?:\/\//.test(url))
            return `버튼 ${n} 액션 ${m}: URL은 http:// 또는 https:// 로 시작해야 합니다`;
          continue;
        }
        // osc action — the D14 rules, per action.
        const address = a.address ?? '';
        if (address.trim() === '') return `버튼 ${n} 액션 ${m}: 주소(address)를 입력하세요`;
        if (!address.startsWith('/')) return `버튼 ${n} 액션 ${m}: 주소는 '/'로 시작해야 합니다`;
        if (/\s/.test(address)) return `버튼 ${n} 액션 ${m}: 주소에 공백이 있으면 안 됩니다`;
        const trimmedAddr = address.trim();
        if (trimmedAddr === '/xrt/ping' || trimmedAddr === '/xrt/pong')
          return `버튼 ${n} 액션 ${m}: /xrt/ping 과 /xrt/pong 은 heartbeat 전용 주소라 사용할 수 없습니다`;
        if (!valueParses(a.value_type, a.value)) {
          if (a.value_type === 'int') return `버튼 ${n} 액션 ${m}: 정수(int) 값이 올바르지 않습니다`;
          if (a.value_type === 'float') return `버튼 ${n} 액션 ${m}: 실수(float) 값이 올바르지 않습니다`;
          if (a.value_type === 'bool') return `버튼 ${n} 액션 ${m}: 불린(bool) 값은 true / false 여야 합니다`;
        }
      }
    }
    return null;
  }
```

- [ ] **Step 2: Markup — replace the button section**

Replace everything from `<h2>버튼 (OSC 메시지)</h2>` through the `+ 버튼 추가` button with:

```svelte
      <h2>버튼</h2>
      {#each draft.buttons as b, i}
        <div class="button-block">
          <div class="grid-row">
            <input class="col-label" placeholder="라벨" bind:value={b.label} />
            <span class="order">
              <button onclick={() => moveButton(i, -1)}>▲</button>
              <button onclick={() => moveButton(i, 1)}>▼</button>
            </span>
            <button class="del" onclick={() => removeButton(i)}>✕</button>
          </div>
          {#each b.actions as a, j}
            <div class="grid-row action-row">
              <select
                class="col-atype"
                value={a.type}
                onchange={(e) => setActionType(b, j, e.currentTarget.value)}
              >
                <option value="http">URL 호출</option>
                <option value="osc">OSC 메시지</option>
              </select>
              {#if a.type === 'http'}
                <input
                  class="col-url"
                  class:invalid={urlInvalid(a)}
                  type="text"
                  placeholder="http://10.10.204.184:16208/gateway/25.2.4/publish?…"
                  bind:value={a.url}
                />
              {:else}
                <input class="col-address" placeholder="주소 (예: /xrt/graphic)" bind:value={a.address} />
                {#if a.value_type === 'bool'}
                  <select class="col-value" class:invalid={valueInvalid(a)} bind:value={a.value}>
                    <option value="true">true</option>
                    <option value="false">false</option>
                  </select>
                {:else if a.value_type === 'none'}
                  <input class="col-value" type="text" placeholder="(값 없음)" disabled />
                {:else if a.value_type === 'int'}
                  <input
                    class="col-value"
                    class:invalid={valueInvalid(a)}
                    type="number"
                    step="1"
                    inputmode="numeric"
                    placeholder="값"
                    value={a.value}
                    oninput={(e) => (a.value = e.currentTarget.value)}
                  />
                {:else if a.value_type === 'float'}
                  <input
                    class="col-value"
                    class:invalid={valueInvalid(a)}
                    type="number"
                    step="any"
                    inputmode="decimal"
                    placeholder="값"
                    value={a.value}
                    oninput={(e) => (a.value = e.currentTarget.value)}
                  />
                {:else}
                  <input
                    class="col-value"
                    type="text"
                    placeholder="값"
                    value={a.value}
                    oninput={(e) => (a.value = e.currentTarget.value)}
                  />
                {/if}
                <select class="col-type" bind:value={a.value_type}>
                  <option value="none">none</option>
                  <option value="string">string</option>
                  <option value="int">int</option>
                  <option value="float">float</option>
                  <option value="bool">bool</option>
                </select>
              {/if}
              <button class="del" onclick={() => removeAction(b, j)}>✕</button>
            </div>
          {/each}
          <button class="add add-action" onclick={() => addAction(b)}>+ 액션 추가</button>
        </div>
      {/each}
      <button class="add" onclick={addButton}>+ 버튼 추가</button>
```

Notes for the implementer:
- The osc branch is the EXISTING widget row verbatim, with `b.` → `a.` — the value-always-a-String rule (one-way `value=` + `oninput`) and the `bind:value` on the bool select carry over unchanged.
- The ▲▼ reorder stays at the BUTTON level (existing feature). There is NO action-level reorder in v1 (user decision).
- The `버튼 (OSC 메시지)` heading becomes just `버튼` — buttons are no longer OSC-only.

- [ ] **Step 3: CSS additions**

Add after the `.grid-row .col-value` rule:

```css
  /* D16: one bordered block per button (label row + its action rows), so
     multi-action buttons read as one unit in the list. */
  .button-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--glass-border);
    border-radius: 10px;
  }
  /* Action rows sit indented under the button's label row. */
  .action-row { margin-left: 12px; }
  .grid-row .col-atype { flex: none; width: 116px; }
  .grid-row .col-url { flex: 3; min-width: 0; }
  .add-action { margin-left: 12px; min-height: 32px; font-size: 12px; align-self: flex-start; }
```

And REPLACE the existing invalid-feedback rule (`.col-value.invalid { border-color: var(--status-lost); }`) with the extended selector so the URL field gets the same red border:

```css
  .col-value.invalid, .col-url.invalid { border-color: var(--status-lost); }
```

(`select.col-atype` picks up the existing dark `select` styling automatically.)

- [ ] **Step 4: Verify build + manual settings check**

Run: `npm run build`
Expected: all 4 entries build clean.

Manual (browser, mock): `npm run dev` → open the settings page entry → the three mock buttons render as blocks; CAM 1 shows an `URL 호출` row with the Pixotope URL; `+ 액션 추가` appends an http row; switching a row to `OSC 메시지` shows the D14 widgets; deleting the last action then [적용] → `버튼 1: 액션이 최소 1개 필요합니다` flash.

- [ ] **Step 5: STOP for review + commit (user runs)**

```bash
git add ui/src/widget/Settings.svelte   # 액션 리스트 편집 UI + 검증
git commit
```

Commit message:

```
feat(ui): per-button action list editor (D16)

버튼 편집을 label + 액션 리스트(블록)로 개편 — 행마다
[URL 호출|OSC 메시지] 타입 선택(기본 http, "URL 우선"),
URL 한 줄 입력(inline invalid 표시), OSC 행은 D14 위젯
재사용. [적용] 검증에 액션≥1(마이그레이션 안전 불변식)
+ http URL 규칙 추가, 기존 D14 규칙은 액션 단위로 이동.
```

---

## Task 6: Version bump + full verification + Mac smoke

**Files:**
- Modify: `app/tauri.conf.json:5` (`"version": "0.2.1"` → `"0.3.0"`)

**Interfaces:**
- Consumes: everything above.
- Produces: a verified v0.3.0 tree ready for the user to tag/release after the Windows/Pixotope on-device session.

- [ ] **Step 1: Bump the version**

In `app/tauri.conf.json` line 5:

```json
  "version": "0.3.0",
```

- [ ] **Step 2: Full automated verification**

Run: `cargo test --workspace`
Expected: PASS — all core tests (config D16 + http + existing net/heartbeat/osc/mock-xr) green.

Run: `cargo check -p xrt-app`
Expected: clean, no warnings.

Run: `npm run build`
Expected: 4 entries build clean.

- [ ] **Step 3: Mac smoke — end-to-end HTTP action through the real app**

This is the one step that exercises the full path (palette → press → engine → spawned thread → ureq → red flash). Terminal A:

```bash
python3 -m http.server 8123   # 로컬 미니 HTTP 서버 — GET에 200 응답
```

Terminal B: `npm run dev` is NOT enough (no Tauri) — run the real app the usual way for this repo:

```bash
tauri dev   # 전역 @tauri-apps/cli (평소 개발 실행 방식 그대로)
```

In the app: ⚙ → add a button `HTTP OK` with action `URL 호출` = `http://127.0.0.1:8123/` → [적용] → press it. Expected: no red flash; terminal A logs a GET. Then stop the python server and press again. Expected: the button flashes red ~1.5s and the dev console logs `HTTP GET http://127.0.0.1:8123/ failed: ...`. Also press an OSC button with no mock-xr running and an ACTIVE target configured — expected: still no crash; (optional) run `cargo run -p mock-xr` and confirm OSC presses work as before.

Also verify config migration on disk: quit the app, open the saved `config.toml` — buttons must show `[[buttons.actions]]` entries and NO flat address/value/value_type keys at the button level.

- [ ] **Step 4: STOP for review + commit (user runs)**

```bash
git add app/tauri.conf.json   # 버전만
git commit
```

Commit message:

```
chore: bump version to 0.3.0

D16 HTTP(URL) 액션 — 버튼=액션 리스트, Pixotope Gateway
카메라 컷 지원. Windows 실기(Pixotope 가동) 검증 후 태그.
```

**Tag/Release는 여기서 하지 않는다** — Windows 방송 PC에서 Pixotope 실검증(카메라 컷 실제 동작) 후 사용자가 `git tag v0.3.0` + push로 release.yml을 태운다(기존 v0.2.x 절차와 동일).

---

## Post-plan notes

- **Windows 검증 세션으로 넘길 것**: 실제 Pixotope Gateway 카메라 컷(`SetCameraSet` ParamNumber 0/1/...), 방송 터치스크린에서 red-flash 시인성, WebView2 렌더 회귀 없음 확인.
- **STATUS.md / WORKLOG.md** (vault): 구현 세션 종료 시 갱신 — plan 실행 상태 + 다음 할 일(Windows 검증)을 기록.
- **BACKLOG 연동**: HTTP 실패 상세는 현재 `eprintln!` — Windows release에선 증발(BACKLOG-2 파일 로깅과 같은 제약). red flash가 라이브 안전을 커버하므로 v1 수용.
