# XR-Touch_to_OSC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **이 워크스페이스의 git 케이던스 (사용자 지침, 스킬보다 우선):** implementer(subagent)는 **절대 commit하지 않는다.** 각 task의 마지막 "커밋 게이트" step에서 실행을 멈추고, 사용자가 working-tree diff를 리뷰한 뒤 직접 commit한다.

**Goal:** 터치스크린 Windows PC 최상단에 뜨는 glass 디자인 OSC 트리거 팔레트(Tauri) + 데모 웹페이지 + mock 수신기.

**Architecture:** 순수 로직은 `crates/core`(config·OSC·heartbeat, GUI 없음)에 격리하고, `crates/mock-xr` CLI가 core의 첫 소비자로서 UE 없는 개발 루프를 완성한다. `app/`(Tauri v2)은 창 관리·vibrancy·IPC만 담당하는 얇은 셸이고, `ui/`(Vite+Svelte 멀티 엔트리)가 위젯·설정·데모·dev harness 화면을 담는다.

**Tech Stack:** Rust (rosc, serde, toml, thiserror), Tauri v2, window-vibrancy, Svelte 5 + Vite, GitHub Actions (windows runner).

**Spec:** `docs/superpowers/specs/2026-07-02-xr-touch-trigger-design.md` — 모든 요구의 원천. 충돌 시 spec 우선.

## Global Constraints

- OSC 주소: 트리거 `/xrt/graphic` + string arg(graphic_id), ping `/xrt/ping`(인자 없음), pong `/xrt/pong`(인자 없음). 다른 주소 금지.
- 포트 기본값: UE 수신 `8000`, 위젯 pong 수신 `8001`. 반드시 config로 변경 가능해야 함.
- 트리거는 **단발 전송** — 반복 전송(dedupe 방식) 구현 금지 (spec D2, 이중 재생 사고 방지).
- 트리거는 **active 대상에만**, ping은 **모든 등록 대상에** 전송.
- heartbeat 기본값: 1000ms 주기, 3회 연속 pong 누락 → Lost.
- heartbeat은 표시만 한다 — **Lost 장비로의 트리거 전송을 막는 코드 금지** (spec §8).
- config 파손 시 기본값으로 기동 + 경고 노출 (조용히 죽지 않기).
- `ui/src/widget/`과 `ui/src/demo/`는 서로 import 금지. 공유는 `ui/src/shared/`만.
- `ui/package.json`에 `x-dependency-owners` 장부 유지 — 의존성 추가 시 반드시 분류.
- 코드·주석·식별자는 English. 사용자 대면 UI 문구는 한국어 가능.
- 버튼 `type` 필드는 v1에서 `"trigger"`만 — 다른 타입 구현 금지(스키마 확장용 필드일 뿐).
- (2026-07-03, D8·D9) config `[appearance]`·`[window]` 확장은 전 필드 serde default — 기존 config.toml·기존 테스트와 호환 유지.
- (2026-07-03, D8) 창은 항상 OS-resizable=false — 크기 변경 경로는 편집 모드 grip(수동 delta→setSize)과 설정 창 preview/적용(D10)뿐. 방송 중 오조작 방지.
- (2026-07-03, D9) 설정 창은 불투명 고정 — 투명도 설정 대상 아님(vibrancy·transparent 미적용).
- (2026-07-03, D10) 설정 창의 외형·크기 변경은 실시간 preview(비영속) + [적용] 시에만 config 저장, [뒤로가기] 시 폐기. 프로그램 종료 경로는 설정 창 안에만 둔다(팔레트에 종료 없음).

---

## File Structure

전부 `[create]` (repo에는 현재 `docs/`만 존재). 실행 전 `git cat-file -e HEAD:<path>`로 pre-existence 확인 규약 적용.

```text
XR-Touch_to_OSC/
├── Cargo.toml                        [create] workspace root
├── .gitignore                        [create]
├── crates/
│   ├── core/
│   │   ├── Cargo.toml                [create]
│   │   └── src/
│   │       ├── lib.rs                [create] 모듈 선언만
│   │       ├── config.rs             [create] Config 모델 + TOML load/save + fallback
│   │       ├── osc.rs                [create] OSC encode/decode (rosc 래핑)
│   │       ├── net.rs                [create] UdpSocket 래퍼 (send/recv)
│   │       └── heartbeat.rs          [create] 순수 상태 머신 (시간 주입)
│   └── mock-xr/
│       ├── Cargo.toml                [create]
│       ├── src/
│       │   ├── main.rs               [create] CLI 진입점
│       │   └── lib.rs                [create] 수신 루프 (테스트 가능하게 분리)
│       └── tests/
│           └── loopback.rs           [create] core ↔ mock-xr 통합 테스트
├── app/
│   ├── Cargo.toml                    [create] Tauri 셸
│   ├── build.rs                      [create]
│   ├── tauri.conf.json               [create] frameless·transparent·always-on-top
│   ├── capabilities/default.json     [create]
│   ├── icons/                        [create] tauri icon 생성물
│   └── src/
│       ├── main.rs                   [create]
│       └── engine.rs                 [create] core 스레드 (heartbeat 루프 + 커맨드 채널)
├── ui/
│   ├── package.json                  [create] + x-dependency-owners 장부
│   ├── vite.config.js                [create] 멀티 엔트리 4개
│   ├── widget.html                   [create] Tauri가 로드
│   ├── settings.html                 [create] 설정 창
│   ├── demo.html                     [create] 데모 페이지
│   ├── harness.html                  [create] dev harness (가짜 배경 + 팔레트)
│   └── src/
│       ├── shared/tokens.css         [create] 디자인 토큰 (색·블러·radius)
│       ├── shared/GlassPanel.svelte  [create] glass 컨테이너 공용 컴포넌트
│       ├── widget/Palette.svelte     [create] 팔레트 (핸들·상태점·버튼·⚙)
│       ├── widget/widget-main.js     [create] widget 엔트리
│       ├── widget/Settings.svelte    [create] 설정 화면
│       ├── widget/settings-main.js   [create] settings 엔트리
│       ├── widget/ipc.js             [create] Tauri IPC 래퍼 (mock 가능)
│       ├── demo/Demo.svelte          [create] 데모 슬라이드
│       ├── demo/demo-main.js         [create] demo 엔트리
│       └── harness/harness-main.js   [create] harness 엔트리
├── .github/workflows/build.yml      [create] windows(+macos) 빌드
└── docs/windows-checklist.md         [create] 실장비 수동 검증 체크리스트
```

**Interfaces 개요 (task 간 계약):** core가 내보내는 공개 API는 Task 1~3에서 확정되고, Task 4(mock-xr)·Task 7(app engine)이 소비한다. UI는 Task 8부터 `ipc.js` 한 파일을 통해서만 Tauri와 통신한다(데모·harness는 Tauri API를 절대 import하지 않음).

---

### Task 1: Cargo workspace + core config 모델 (TOML roundtrip & fallback)

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/config.rs`

**Interfaces:**
- Produces: `xrt_core::config::{Config, NetworkConfig, Target, ButtonDef, LoadOutcome, load, save}` — 이후 모든 task가 이 타입을 사용. 시그니처는 Step 3 코드가 정본.

- [ ] **Step 1: workspace 골격 생성**

`Cargo.toml` (repo root):

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/mock-xr"]
```

`.gitignore`:

```gitignore
/target
node_modules
ui/dist
```

`crates/core/Cargo.toml`:

```toml
[package]
name = "xrt-core"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
rosc = "0.11"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

`crates/core/src/lib.rs`:

```rust
pub mod config;
```

mock-xr는 Task 4에서 만들지만 workspace members에 이미 있으므로, 지금은 빈 껍데기를 만들어 빌드를 살린다 — `crates/mock-xr/Cargo.toml`:

```toml
[package]
name = "mock-xr"
version = "0.1.0"
edition = "2024"

[dependencies]
xrt-core = { path = "../core" }
```

`crates/mock-xr/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 2: failing test 작성**

`crates/core/src/config.rs` (테스트 먼저 — 같은 파일 하단 `#[cfg(test)]`):

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---- types are added in Step 4 ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_spec_defaults() {
        let c = Config::default();
        assert_eq!(c.network.ue_port, 8000);
        assert_eq!(c.network.listen_port, 8001);
        assert_eq!(c.network.heartbeat_interval_ms, 1000);
        assert_eq!(c.network.heartbeat_timeout_misses, 3);
        assert!(c.targets.is_empty());
        assert!(c.buttons.is_empty());
    }

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
            graphic_id: "lower_third_a".into(),
            button_type: "trigger".into(),
        });
        save(&path, &c).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, c);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn missing_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let (c, outcome) = load(&dir.path().join("nope.toml"));
        assert_eq!(c, Config::default());
        assert!(matches!(outcome, LoadOutcome::MissingUsedDefault));
    }

    #[test]
    fn corrupt_file_falls_back_to_default_with_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is {{ not toml").unwrap();
        let (c, outcome) = load(&path);
        assert_eq!(c, Config::default());
        assert!(matches!(outcome, LoadOutcome::ParseErrorUsedDefault(_)));
    }

    #[test]
    fn button_type_defaults_to_trigger_when_absent() {
        let toml_str = r#"
            [[buttons]]
            label = "A"
            graphic_id = "a"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].button_type, "trigger");
    }
}
```

- [ ] **Step 3: 컴파일 실패 확인**

Run: `cargo test -p xrt-core`
Expected: FAIL — `Config`, `Target`, `ButtonDef`, `LoadOutcome`, `load`, `save` not found.

- [ ] **Step 4: 최소 구현**

같은 파일 상단(테스트 모듈 위)에 추가:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub targets: Vec<Target>,
    pub buttons: Vec<ButtonDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkConfig {
    pub ue_port: u16,
    pub listen_port: u16,
    pub heartbeat_interval_ms: u64,
    pub heartbeat_timeout_misses: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ue_port: 8000,
            listen_port: 8001,
            heartbeat_interval_ms: 1000,
            heartbeat_timeout_misses: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub name: String,
    pub ip: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ButtonDef {
    pub label: String,
    pub graphic_id: String,
    #[serde(rename = "type", default = "default_button_type")]
    pub button_type: String,
}

fn default_button_type() -> String {
    "trigger".into()
}

/// Result of loading config: the app must always get a usable Config
/// (spec §8 — start with defaults instead of dying).
#[derive(Debug)]
pub enum LoadOutcome {
    Loaded,
    MissingUsedDefault,
    ParseErrorUsedDefault(String),
}

pub fn load(path: &Path) -> (Config, LoadOutcome) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Config::default(), LoadOutcome::MissingUsedDefault),
    };
    match toml::from_str::<Config>(&text) {
        Ok(c) => (c, LoadOutcome::Loaded),
        Err(e) => (
            Config::default(),
            LoadOutcome::ParseErrorUsedDefault(e.to_string()),
        ),
    }
}

pub fn save(path: &Path, config: &Config) -> std::io::Result<()> {
    let text = toml::to_string_pretty(config).expect("Config is always serializable");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `cargo test -p xrt-core`
Expected: PASS (5 tests).

- [ ] **Step 6: 커밋 게이트 (사용자 직접)**

implementer는 여기서 멈춘다. 사용자 리뷰 후:

```bash
git add Cargo.toml .gitignore crates/   # 워크스페이스·core·mock-xr 껍데기를 staging
git commit -m "feat(core): cargo workspace + config model with TOML fallback"   # -m: 메시지 인라인 지정
```

### Task 2: core OSC encode/decode + UDP 소켓 래퍼

**Files:**
- Create: `crates/core/src/osc.rs`, `crates/core/src/net.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `config::Target` (Task 1)
- Produces:
  - `osc::{ADDR_GRAPHIC, ADDR_PING, ADDR_PONG}` (str 상수)
  - `osc::encode_trigger(graphic_id: &str) -> Vec<u8>` / `osc::encode_ping() -> Vec<u8>` / `osc::encode_pong() -> Vec<u8>`
  - `osc::decode(buf: &[u8]) -> Incoming` where `enum Incoming { Trigger(String), Ping, Pong, Other }`
  - `net::OscSocket::{bind(listen_port) -> io::Result<Self>, send_trigger(&self, id, targets: &[Target], ue_port) -> Vec<SendReport>, send_ping_all(&self, targets, ue_port) -> Vec<SendReport>, try_recv(&self) -> Option<(Incoming, SocketAddr)>}`
  - `net::SendReport { ip: String, ok: bool }`

**설계 노트 (구현자가 알아야 할 것):** 소켓 하나를 `0.0.0.0:listen_port`에 bind해서 송신·수신 겸용으로 쓴다. 이러면 ping의 source port가 listen_port가 되어, 수신 측(mock-xr/UE)이 "패킷 발신자 IP + listen_port"로 pong을 되쏘는 계약이 성립한다. `send_trigger`는 **active target만** 걸러 보내고, `send_ping_all`은 전체에 보낸다 (Global Constraints 참조).

- [ ] **Step 1: failing test — OSC 인코딩/디코딩**

`crates/core/src/osc.rs`:

```rust
use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

// ---- implementation added in Step 3 ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_roundtrips_with_graphic_id() {
        let bytes = encode_trigger("lower_third_a");
        assert!(matches!(
            decode(&bytes),
            Incoming::Trigger(id) if id == "lower_third_a"
        ));
    }

    #[test]
    fn ping_and_pong_roundtrip() {
        assert!(matches!(decode(&encode_ping()), Incoming::Ping));
        assert!(matches!(decode(&encode_pong()), Incoming::Pong));
    }

    #[test]
    fn unknown_address_is_other() {
        let msg = OscPacket::Message(OscMessage {
            addr: "/something/else".into(),
            args: vec![],
        });
        let bytes = encoder::encode(&msg).unwrap();
        assert!(matches!(decode(&bytes), Incoming::Other));
    }

    #[test]
    fn garbage_bytes_are_other() {
        assert!(matches!(decode(&[0x01, 0x02, 0x03]), Incoming::Other));
    }
}
```

`crates/core/src/lib.rs` 갱신:

```rust
pub mod config;
pub mod net;
pub mod osc;
```

(net.rs는 Step 4에서 만들기 전까지 빈 파일로 생성: `// filled in by net socket step`)

- [ ] **Step 2: 실패 확인**

Run: `cargo test -p xrt-core osc`
Expected: FAIL — `encode_trigger`, `decode`, `Incoming` not found.

- [ ] **Step 3: OSC 구현**

`osc.rs` 상단에 추가:

```rust
pub const ADDR_GRAPHIC: &str = "/xrt/graphic";
pub const ADDR_PING: &str = "/xrt/ping";
pub const ADDR_PONG: &str = "/xrt/pong";

#[derive(Debug, PartialEq)]
pub enum Incoming {
    Trigger(String),
    Ping,
    Pong,
    Other,
}

fn encode_message(addr: &str, args: Vec<OscType>) -> Vec<u8> {
    encoder::encode(&OscPacket::Message(OscMessage {
        addr: addr.into(),
        args,
    }))
    .expect("static OSC messages always encode")
}

pub fn encode_trigger(graphic_id: &str) -> Vec<u8> {
    encode_message(ADDR_GRAPHIC, vec![OscType::String(graphic_id.into())])
}

pub fn encode_ping() -> Vec<u8> {
    encode_message(ADDR_PING, vec![])
}

pub fn encode_pong() -> Vec<u8> {
    encode_message(ADDR_PONG, vec![])
}

pub fn decode(buf: &[u8]) -> Incoming {
    let Ok((_rest, packet)) = decoder::decode_udp(buf) else {
        return Incoming::Other;
    };
    let OscPacket::Message(msg) = packet else {
        return Incoming::Other;
    };
    match msg.addr.as_str() {
        ADDR_GRAPHIC => match msg.args.first() {
            Some(OscType::String(id)) => Incoming::Trigger(id.clone()),
            _ => Incoming::Other,
        },
        ADDR_PING => Incoming::Ping,
        ADDR_PONG => Incoming::Pong,
        _ => Incoming::Other,
    }
}
```

Run: `cargo test -p xrt-core osc` → Expected: PASS (4 tests).

- [ ] **Step 4: failing test — 소켓 loopback**

`crates/core/src/net.rs`:

```rust
use std::io;
use std::net::{SocketAddr, UdpSocket};

use crate::config::Target;
use crate::osc::{self, Incoming};

// ---- implementation added in Step 6 ----

#[cfg(test)]
mod tests {
    use super::*;

    /// Bind a plain receiver on an ephemeral port, pretend it is the UE box.
    fn fake_ue() -> (UdpSocket, u16) {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = sock.local_addr().unwrap().port();
        (sock, port)
    }

    fn target(ip: &str, active: bool) -> Target {
        Target { name: "t".into(), ip: ip.into(), active }
    }

    #[test]
    fn trigger_reaches_only_active_targets() {
        let (ue, ue_port) = fake_ue();
        ue.set_nonblocking(true).unwrap();
        let osc_sock = OscSocket::bind(0).unwrap(); // 0 = ephemeral for tests

        let targets = vec![target("127.0.0.1", true), target("127.0.0.2", false)];
        let reports = osc_sock.send_trigger("g1", &targets, ue_port);

        // only the active target got a send attempt
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].ip, "127.0.0.1");
        assert!(reports[0].ok);

        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut buf = [0u8; 1024];
        let (n, _) = ue.recv_from(&mut buf).unwrap();
        assert!(matches!(osc::decode(&buf[..n]), Incoming::Trigger(id) if id == "g1"));
        assert!(ue.recv_from(&mut buf).is_err(), "no second packet expected");
    }

    #[test]
    fn ping_goes_to_all_targets_and_pong_comes_back() {
        let (ue, ue_port) = fake_ue();
        let osc_sock = OscSocket::bind(0).unwrap();
        let listen_port = osc_sock.local_port();

        let targets = vec![target("127.0.0.1", false)]; // inactive still pinged
        let reports = osc_sock.send_ping_all(&targets, ue_port);
        assert_eq!(reports.len(), 1);

        // fake UE receives ping, replies pong to source ip + listen_port
        let mut buf = [0u8; 1024];
        let (n, from) = ue.recv_from(&mut buf).unwrap();
        assert!(matches!(osc::decode(&buf[..n]), Incoming::Ping));
        let reply_to = SocketAddr::new(from.ip(), listen_port);
        ue.send_to(&osc::encode_pong(), reply_to).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        let (incoming, from) = osc_sock.try_recv().expect("pong should be waiting");
        assert!(matches!(incoming, Incoming::Pong));
        assert_eq!(from.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn try_recv_returns_none_when_quiet() {
        let osc_sock = OscSocket::bind(0).unwrap();
        assert!(osc_sock.try_recv().is_none());
    }
}
```

- [ ] **Step 5: 실패 확인**

Run: `cargo test -p xrt-core net`
Expected: FAIL — `OscSocket`, `SendReport` not found.

- [ ] **Step 6: 소켓 구현**

`net.rs` 상단에 추가:

```rust
#[derive(Debug)]
pub struct SendReport {
    pub ip: String,
    pub ok: bool,
}

/// One UDP socket, bound to listen_port, used for both sending and receiving.
/// Binding the send side means our pings carry listen_port as source port,
/// so receivers can reply to "source ip + listen_port" (interface contract).
pub struct OscSocket {
    socket: UdpSocket,
}

impl OscSocket {
    pub fn bind(listen_port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", listen_port))?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().expect("bound socket has addr").port()
    }

    fn send_bytes(&self, bytes: &[u8], ip: &str, port: u16) -> SendReport {
        let ok = self.socket.send_to(bytes, (ip, port)).is_ok();
        SendReport { ip: ip.to_string(), ok }
    }

    /// Single shot, active targets only. Never retries (spec D2).
    pub fn send_trigger(&self, graphic_id: &str, targets: &[Target], ue_port: u16) -> Vec<SendReport> {
        let bytes = osc::encode_trigger(graphic_id);
        targets
            .iter()
            .filter(|t| t.active)
            .map(|t| self.send_bytes(&bytes, &t.ip, ue_port))
            .collect()
    }

    /// Pings every registered target, active or not (status visible before switching).
    pub fn send_ping_all(&self, targets: &[Target], ue_port: u16) -> Vec<SendReport> {
        let bytes = osc::encode_ping();
        targets
            .iter()
            .map(|t| self.send_bytes(&bytes, &t.ip, ue_port))
            .collect()
    }

    pub fn try_recv(&self) -> Option<(Incoming, SocketAddr)> {
        let mut buf = [0u8; 1536];
        match self.socket.recv_from(&mut buf) {
            Ok((n, from)) => Some((osc::decode(&buf[..n]), from)),
            Err(_) => None,
        }
    }
}
```

- [ ] **Step 7: 전체 테스트 통과 확인**

Run: `cargo test -p xrt-core`
Expected: PASS (config 5 + osc 4 + net 3 = 12 tests).

- [ ] **Step 8: 커밋 게이트 (사용자 직접)**

```bash
git add crates/core/   # osc.rs·net.rs 추가와 lib.rs 수정
git commit -m "feat(core): OSC message codec + single-socket UDP send/recv"
```

### Task 3: core heartbeat 상태 머신

**Files:**
- Create: `crates/core/src/heartbeat.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod heartbeat;` 추가)

**Interfaces:**
- Produces:
  - `heartbeat::LinkStatus` — `enum { Unknown, Connected, Lost }` (`Copy, Clone, PartialEq, Debug, serde::Serialize` derive)
  - `heartbeat::Heartbeat::{new(timeout_misses: u32) -> Self, on_tick(&mut self, target_ips: &[String]), on_pong(&mut self, ip: &str), status(&self, ip: &str) -> LinkStatus}`

**설계 노트:** 시간을 읽지 않는 순수 상태 머신. "tick 한 번 = ping 한 번 보낼 타이밍"이며, tick 시점에 직전 ping의 응답이 없었으면 miss 1 증가. miss가 `timeout_misses`에 도달하면 Lost. pong 수신 시 즉시 Connected + miss 리셋. 목록에서 사라진 ip의 상태는 tick 때 정리(제거)한다.

- [ ] **Step 1: failing test 작성**

`crates/core/src/heartbeat.rs`:

```rust
use std::collections::HashMap;

// ---- implementation added in Step 3 ----

#[cfg(test)]
mod tests {
    use super::*;

    fn ips(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn starts_unknown() {
        let hb = Heartbeat::new(3);
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Unknown);
    }

    #[test]
    fn three_silent_ticks_mean_lost() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        hb.on_tick(&t); // miss 1
        hb.on_tick(&t); // miss 2
        assert_ne!(hb.status("10.0.0.1"), LinkStatus::Lost, "not lost yet at 2 misses");
        hb.on_tick(&t); // miss 3 -> Lost
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
    }

    #[test]
    fn pong_connects_and_resets_misses() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        hb.on_tick(&t);
        hb.on_tick(&t);
        hb.on_pong("10.0.0.1");
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
        hb.on_tick(&t); // miss count restarts from here
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
    }

    #[test]
    fn lost_target_recovers_on_pong() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        for _ in 0..3 {
            hb.on_tick(&t);
        }
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
        hb.on_pong("10.0.0.1");
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
    }

    #[test]
    fn removed_target_is_forgotten() {
        let mut hb = Heartbeat::new(3);
        hb.on_tick(&ips(&["10.0.0.1"]));
        hb.on_pong("10.0.0.1");
        hb.on_tick(&ips(&[])); // target removed from config
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Unknown);
    }

    #[test]
    fn pong_from_unregistered_ip_is_ignored() {
        let mut hb = Heartbeat::new(3);
        hb.on_pong("99.9.9.9"); // never ticked -> not tracked
        assert_eq!(hb.status("99.9.9.9"), LinkStatus::Unknown);
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test -p xrt-core heartbeat`
Expected: FAIL — `Heartbeat`, `LinkStatus` not found. (lib.rs에 `pub mod heartbeat;` 추가 잊지 말 것)

- [ ] **Step 3: 구현**

`heartbeat.rs` 상단에 추가:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum LinkStatus {
    Unknown,
    Connected,
    Lost,
}

struct Entry {
    misses: u32,
    status: LinkStatus,
}

/// Pure state machine — no clocks inside. The engine loop owns timing and
/// calls on_tick once per heartbeat interval.
pub struct Heartbeat {
    timeout_misses: u32,
    entries: HashMap<String, Entry>,
}

impl Heartbeat {
    pub fn new(timeout_misses: u32) -> Self {
        Self { timeout_misses, entries: HashMap::new() }
    }

    pub fn on_tick(&mut self, target_ips: &[String]) {
        self.entries.retain(|ip, _| target_ips.contains(ip));
        for ip in target_ips {
            let entry = self
                .entries
                .entry(ip.clone())
                .or_insert(Entry { misses: 0, status: LinkStatus::Unknown });
            entry.misses = entry.misses.saturating_add(1);
            if entry.misses >= self.timeout_misses {
                entry.status = LinkStatus::Lost;
            }
        }
    }

    pub fn on_pong(&mut self, ip: &str) {
        if let Some(entry) = self.entries.get_mut(ip) {
            entry.misses = 0;
            entry.status = LinkStatus::Connected;
        }
    }

    pub fn status(&self, ip: &str) -> LinkStatus {
        self.entries.get(ip).map(|e| e.status).unwrap_or(LinkStatus::Unknown)
    }
}
```

`serde` derive를 쓰므로 이미 dependency에 있음 (Task 1).

- [ ] **Step 4: 통과 확인**

Run: `cargo test -p xrt-core`
Expected: PASS (12 + 6 = 18 tests).

- [ ] **Step 5: 커밋 게이트 (사용자 직접)**

```bash
git add crates/core/
git commit -m "feat(core): heartbeat state machine with injected ticks"
```

### Task 4: mock-xr CLI (UE 대역) + loopback 통합 테스트

**Files:**
- Create: `crates/mock-xr/src/lib.rs`, `crates/mock-xr/tests/loopback.rs`
- Modify: `crates/mock-xr/Cargo.toml`, `crates/mock-xr/src/main.rs`

**Interfaces:**
- Consumes: `xrt_core::osc::{decode, encode_pong, Incoming}`, `xrt_core::net::OscSocket`, `xrt_core::config::Target`, `xrt_core::heartbeat::{Heartbeat, LinkStatus}`
- Produces: `mock_xr::{MockXr, Event}` — `MockXr::bind(port: u16, reply_port: u16) -> io::Result<Self>`, `MockXr::poll_once(&self) -> Option<Event>` where `enum Event { Trigger(String), PingAnswered(SocketAddr) }`. CLI: `mock-xr [port] [reply_port]` (기본 8000, 8001).

**설계 노트:** 수신 루프 본체를 lib로 빼서 테스트에서 스레드 없이 `poll_once`로 한 스텝씩 돌린다. ping 수신 시 "발신자 IP + reply_port"로 pong 회신 — UE Blueprint가 지킬 인터페이스 계약과 동일한 동작.

- [ ] **Step 1: failing test — 통합 loopback**

`crates/mock-xr/tests/loopback.rs`:

```rust
use mock_xr::{Event, MockXr};
use xrt_core::config::Target;
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;

fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

#[test]
fn full_loop_trigger_ping_pong_and_heartbeat() {
    // widget side: ephemeral listen port
    let widget = OscSocket::bind(0).unwrap();
    let listen_port = widget.local_port();

    // UE side: mock on ephemeral port, replying pongs to our listen port
    let mock = MockXr::bind(0, listen_port).unwrap();
    let ue_port = mock.local_port();

    let targets = vec![Target {
        name: "MOCK".into(),
        ip: "127.0.0.1".into(),
        active: true,
    }];

    // --- trigger path ---
    widget.send_trigger("stinger_open", &targets, ue_port);
    wait_ms(50);
    assert!(matches!(
        mock.poll_once(),
        Some(Event::Trigger(id)) if id == "stinger_open"
    ));

    // --- heartbeat path ---
    let mut hb = Heartbeat::new(3);
    let ips: Vec<String> = targets.iter().map(|t| t.ip.clone()).collect();

    widget.send_ping_all(&targets, ue_port);
    hb.on_tick(&ips);
    wait_ms(50);
    assert!(matches!(mock.poll_once(), Some(Event::PingAnswered(_))));
    wait_ms(50);

    if let Some((Incoming::Pong, from)) = widget.try_recv() {
        hb.on_pong(&from.ip().to_string());
    }
    assert_eq!(hb.status("127.0.0.1"), LinkStatus::Connected);
}
```

`crates/mock-xr/Cargo.toml` 갱신:

```toml
[package]
name = "mock-xr"
version = "0.1.0"
edition = "2024"

[lib]
name = "mock_xr"

[dependencies]
xrt-core = { path = "../core" }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test -p mock-xr`
Expected: FAIL — `mock_xr` lib / `MockXr` not found.

- [ ] **Step 3: lib 구현**

`crates/mock-xr/src/lib.rs`:

```rust
use std::io;
use std::net::{SocketAddr, UdpSocket};

use xrt_core::osc::{self, Incoming};

#[derive(Debug)]
pub enum Event {
    Trigger(String),
    PingAnswered(SocketAddr),
}

/// Stand-in for the UE box: logs triggers, answers pings with pongs.
/// Mirrors the exact interface contract the UE Blueprint must implement.
pub struct MockXr {
    socket: UdpSocket,
    reply_port: u16,
}

impl MockXr {
    pub fn bind(port: u16, reply_port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, reply_port })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().expect("bound").port()
    }

    /// One receive step. Returns None when no packet is waiting.
    pub fn poll_once(&self) -> Option<Event> {
        let mut buf = [0u8; 1536];
        let (n, from) = self.socket.recv_from(&mut buf).ok()?;
        match osc::decode(&buf[..n]) {
            Incoming::Trigger(id) => Some(Event::Trigger(id)),
            Incoming::Ping => {
                let reply_to = SocketAddr::new(from.ip(), self.reply_port);
                let _ = self.socket.send_to(&osc::encode_pong(), reply_to);
                Some(Event::PingAnswered(reply_to))
            }
            _ => None,
        }
    }
}
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test -p mock-xr`
Expected: PASS (1 test).

- [ ] **Step 5: CLI main 구현**

`crates/mock-xr/src/main.rs` 교체:

```rust
use mock_xr::{Event, MockXr};

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8000);
    let reply_port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8001);

    let mock = MockXr::bind(port, reply_port).expect("failed to bind UDP port");
    println!("mock-xr listening on :{port}, answering pongs to <sender-ip>:{reply_port}");

    loop {
        match mock.poll_once() {
            Some(Event::Trigger(id)) => println!("TRIGGER  graphic_id={id}"),
            Some(Event::PingAnswered(to)) => println!("PING     -> pong to {to}"),
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
}
```

- [ ] **Step 6: 수동 스모크 확인**

Run: `cargo run -p mock-xr` (터미널 1에서 실행 후 Ctrl-C로 종료 가능한지 확인)
Expected: `mock-xr listening on :8000, answering pongs to <sender-ip>:8001` 출력.

- [ ] **Step 7: 커밋 게이트 (사용자 직접)**

```bash
git add crates/mock-xr/
git commit -m "feat(mock-xr): CLI stand-in for UE with loopback integration test"
```

### Task 5: ui 스캐폴드 — Vite 멀티 엔트리 + 디자인 토큰 + 의존성 장부

**Files:**
- Create: `ui/package.json`, `ui/vite.config.js`, `ui/widget.html`, `ui/settings.html`, `ui/demo.html`, `ui/harness.html`, `ui/src/shared/tokens.css`, `ui/src/shared/GlassPanel.svelte`, `ui/src/widget/widget-main.js`, `ui/src/widget/settings-main.js`, `ui/src/demo/demo-main.js`, `ui/src/harness/harness-main.js`, `ui/src/widget/Palette.svelte`(placeholder), `ui/src/demo/Demo.svelte`(placeholder), `ui/src/widget/Settings.svelte`(placeholder)

**Interfaces:**
- Produces: 엔트리 4개(`widget.html` `settings.html` `demo.html` `harness.html`), 디자인 토큰 CSS 변수(`--glass-bg`, `--glass-border`, `--radius`, `--accent`, `--text`, `--status-*`), `GlassPanel.svelte`(slot 컨테이너). Task 8~10이 이 토큰·컴포넌트를 사용.
- 규칙: `widget/`↔`demo/` 상호 import 금지, Tauri API import는 `widget/` 안에서만.

- [ ] **Step 1: package.json + 의존성 설치**

`ui/package.json`:

```json
{
  "name": "xrt-ui",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "x-dependency-owners": {
    "shared": ["svelte", "vite", "@sveltejs/vite-plugin-svelte"],
    "widget-only": ["@tauri-apps/api"],
    "demo-only": []
  }
}
```

Run (ui/ 디렉토리에서):

```bash
npm install -D vite svelte @sveltejs/vite-plugin-svelte   # -D: devDependencies로 설치
npm install @tauri-apps/api                                # 런타임 의존성 (widget-only)
```

- [ ] **Step 2: vite 멀티 엔트리 설정**

`ui/vite.config.js`:

```js
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// package.json is "type": "module", so __dirname must be derived
const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  build: {
    rollupOptions: {
      input: {
        widget: resolve(__dirname, 'widget.html'),
        settings: resolve(__dirname, 'settings.html'),
        demo: resolve(__dirname, 'demo.html'),
        harness: resolve(__dirname, 'harness.html'),
      },
    },
  },
  // Tauri dev: fixed port so tauri.conf.json devUrl can point here
  server: { port: 5173, strictPort: true, host: true },
});
```

(`host: true` — LAN dev server 트릭: 터치스크린 PC/다른 기기의 브라우저에서 접속 가능하게.)

- [ ] **Step 3: 디자인 토큰 + GlassPanel**

`ui/src/shared/tokens.css`:

```css
/* Design tokens — single source of truth for widget AND demo (spec §5/§7) */
:root {
  --glass-bg: rgba(20, 24, 32, 0.55);
  --glass-border: rgba(255, 255, 255, 0.14);
  --glass-highlight: rgba(255, 255, 255, 0.06);
  --radius: 14px;
  --accent: #4da3ff;
  --text: rgba(255, 255, 255, 0.92);
  --text-dim: rgba(255, 255, 255, 0.55);
  --status-active: #39d98a;
  --status-inactive: rgba(255, 255, 255, 0.28);
  --status-lost: #ff5c5c;
  --touch-min: 48px; /* minimum touch target size */
  font-family: 'Segoe UI', 'Pretendard', system-ui, sans-serif;
}
```

`ui/src/shared/GlassPanel.svelte`:

```svelte
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
</style>
```

- [ ] **Step 4: 엔트리 4개 + placeholder 컴포넌트**

`ui/widget.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>XRT Widget</title>
  </head>
  <body style="margin:0; background:transparent; overflow:hidden;">
    <div id="app"></div>
    <script type="module" src="/src/widget/widget-main.js"></script>
  </body>
</html>
```

`ui/settings.html` — 동일하되 `<title>XRT Settings</title>`, script `src="/src/widget/settings-main.js"`, body style은 `margin:0; background:transparent;` (overflow 허용).

`ui/demo.html` — `<title>XRT Demo</title>`, script `src="/src/demo/demo-main.js"`, body style `margin:0;`.

`ui/harness.html` — `<title>XRT Dev Harness</title>`, script `src="/src/harness/harness-main.js"`, body style `margin:0;`.

`ui/src/widget/widget-main.js`:

```js
import { mount } from 'svelte';
import '../shared/tokens.css';
import Palette from './Palette.svelte';

mount(Palette, { target: document.getElementById('app') });
```

`ui/src/widget/settings-main.js` — 동일 패턴, `Settings.svelte`를 mount.
`ui/src/demo/demo-main.js` — 동일 패턴, `Demo.svelte`를 mount.

`ui/src/harness/harness-main.js`:

```js
import { mount } from 'svelte';
import '../shared/tokens.css';
import Palette from '../widget/Palette.svelte';

// Busy fake backdrop so glass CSS can be tuned in a plain browser tab
// (browser tabs have no window transparency).
document.body.style.background =
  'linear-gradient(135deg,#0f2027,#203a43,#2c5364), ' +
  'repeating-linear-gradient(45deg, rgba(255,255,255,.08) 0 12px, transparent 12px 24px)';
document.body.style.minHeight = '100vh';

const host = document.getElementById('app');
host.style.padding = '80px';
mount(Palette, { target: host });
```

placeholder 컴포넌트 3개 (Task 8/9/10에서 교체됨) — `ui/src/widget/Palette.svelte`:

```svelte
<script>
  import GlassPanel from '../shared/GlassPanel.svelte';
</script>

<GlassPanel>
  <div style="padding: 12px 20px;">palette placeholder</div>
</GlassPanel>
```

`ui/src/widget/Settings.svelte`·`ui/src/demo/Demo.svelte` — 같은 구조, 문구만 `settings placeholder` / `demo placeholder`.

- [ ] **Step 5: 빌드·dev 확인**

Run (ui/): `npm run build`
Expected: `dist/`에 widget/settings/demo/harness html 4개 생성, 에러 없음.

Run (ui/): `npm run dev` 후 브라우저에서 `http://localhost:5173/harness.html`
Expected: busy 배경 위에 glass placeholder 패널이 블러와 함께 표시됨.

- [ ] **Step 6: 커밋 게이트 (사용자 직접)**

```bash
git add ui/
git commit -m "feat(ui): vite multi-entry scaffold + design tokens + dev harness"
```

### Task 6: Tauri 셸 — frameless·transparent·always-on-top + glass 적용

**Files:**
- Create: `app/Cargo.toml`, `app/build.rs`, `app/tauri.conf.json`, `app/capabilities/default.json`, `app/src/main.rs`, `app/icons/*` (생성물), `app-icon.png` (원본)
- Modify: `Cargo.toml` (workspace members에 `"app"` 추가)

**Interfaces:**
- Consumes: ui dev server (`http://localhost:5173`, Task 5)
- Produces: 창 label `"palette"` (메인)·`"settings"` (Task 9에서 생성). `apply_glass(&WebviewWindow)` — OS별 vibrancy 적용 함수.

**사전 준비:** `cargo install tauri-cli --locked` (tauri v2 CLI. 이미 설치돼 있으면 생략).

- [ ] **Step 1: app crate 생성**

workspace root `Cargo.toml`의 members를 `["crates/core", "crates/mock-xr", "app"]`로 수정.

`app/Cargo.toml`:

```toml
[package]
name = "xrt-app"
version = "0.1.0"
edition = "2024"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
xrt-core = { path = "../crates/core" }
window-vibrancy = "0.6"
```

`app/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 2: tauri.conf.json + capabilities**

`app/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "xrt-widget",
  "version": "0.1.0",
  "identifier": "kr.co.sbs.ncenter.xrt",
  "build": {
    "devUrl": "http://localhost:5173",
    "frontendDist": "../ui/dist",
    "beforeDevCommand": "npm --prefix ../ui run dev",
    "beforeBuildCommand": "npm --prefix ../ui run build"
  },
  "app": {
    "macOSPrivateApi": true,
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "palette",
        "url": "widget.html",
        "title": "XRT",
        "width": 720,
        "height": 96,
        "transparent": true,
        "decorations": false,
        "alwaysOnTop": true,
        "resizable": false,
        "shadow": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico", "icons/icon.icns"]
  }
}
```

(`macOSPrivateApi` — macOS에서 transparent 창에 필요. `shadow: false` — 투명 창 모서리 그림자 아티팩트 방지.)

`app/capabilities/default.json`:

```json
{
  "identifier": "default",
  "windows": ["palette", "settings"],
  "permissions": ["core:default", "core:window:allow-start-dragging"]
}
```

- [ ] **Step 3: 아이콘 생성**

단색 512×512 PNG를 만들어 tauri icon 세트 생성 (bundle 빌드에 필수):

```bash
magick -size 512x512 canvas:'#4da3ff' app-icon.png   # ImageMagick. 없으면 아무 512x512 PNG나 app-icon.png로 저장
cargo tauri icon app-icon.png -o app/icons            # icons/ 세트 생성 (-o: 출력 디렉토리)
```

Expected: `app/icons/`에 32x32.png·128x128.png·icon.ico·icon.icns 등 생성.

- [ ] **Step 4: main.rs — 창 + glass**

`app/src/main.rs`:

```rust
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let win = app.get_webview_window("palette").expect("palette window exists");
            apply_glass(&win);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// OS-level behind-window blur (spec §7). CSS cannot blur what is behind
/// the window — only the compositor can.
fn apply_glass(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        // Acrylic works on Win10 and Win11. Mica (Win11-only) is an
        // alternative to evaluate during final on-device tuning.
        if let Err(e) = window_vibrancy::apply_acrylic(win, Some((20, 24, 32, 120))) {
            eprintln!("acrylic failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
        if let Err(e) = apply_vibrancy(win, NSVisualEffectMaterial::HudWindow, None, Some(14.0)) {
            eprintln!("vibrancy failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Dev fallback: plain transparency, no blur (spec §7).
        let _ = win;
    }
}
```

- [ ] **Step 5: 수동 검증 (Linux 또는 Mac)**

Run (app/): `cargo tauri dev`
Expected:
- 프레임 없는 작은 창이 화면에 뜸, 배경 투명(placeholder glass 패널만 보임).
- 다른 창을 클릭해도 팔레트가 항상 위에 있음 (always-on-top).
- Mac이라면: 창 뒤가 실제로 블러됨. Linux라면: 블러 없는 반투명 (정상 — spec §7 fallback).

- [ ] **Step 6: 커밋 게이트 (사용자 직접)**

```bash
git add Cargo.toml app/ app-icon.png
git commit -m "feat(app): tauri shell - frameless transparent always-on-top + vibrancy"
```

### Task 7: app engine — core 스레드·IPC commands·status 이벤트

**Files:**
- Create: `app/src/engine.rs`
- Modify: `app/src/main.rs`, `crates/core/src/heartbeat.rs` (mark_lost 추가)

**Interfaces:**
- Consumes: `xrt_core::{config, net::OscSocket, heartbeat::Heartbeat, ...}` (Task 1~3)
- Produces (JS가 소비, Task 8~9의 계약):
  - commands: `get_config() -> Config`, `save_config(config: Config) -> Result<(), String>`, `trigger(graphicId: String)`, `open_settings()`, `load_warning() -> Option<String>`
  - event `"xrt://status"` payload: `[{ "name": String, "ip": String, "active": bool, "status": "Unknown"|"Connected"|"Lost" }]`
  - Rust 내부: `engine::EngineCmd { Trigger(String), UpdateConfig(Config) }`, `engine::spawn(app_handle, config, socket) -> Sender<EngineCmd>`

- [ ] **Step 1: failing test — Heartbeat::mark_lost (즉시 빨간 점, spec §8)**

`crates/core/src/heartbeat.rs` tests 모듈에 추가:

```rust
    #[test]
    fn mark_lost_forces_lost_even_when_connected() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        hb.on_tick(&t);
        hb.on_pong("10.0.0.1");
        hb.mark_lost("10.0.0.1"); // e.g. socket send error
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
    }
```

Run: `cargo test -p xrt-core mark_lost` → Expected: FAIL (method not found).

- [ ] **Step 2: mark_lost 구현 + 통과 확인**

`Heartbeat` impl에 추가:

```rust
    /// Immediate Lost, e.g. when a send fails at socket level (spec §8).
    pub fn mark_lost(&mut self, ip: &str) {
        if let Some(entry) = self.entries.get_mut(ip) {
            entry.misses = self.timeout_misses;
            entry.status = LinkStatus::Lost;
        }
    }
```

Run: `cargo test -p xrt-core` → Expected: PASS (전체 19 tests).

- [ ] **Step 3: engine 스레드 구현**

`app/src/engine.rs`:

```rust
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use xrt_core::config::Config;
use xrt_core::heartbeat::{Heartbeat, LinkStatus};
use xrt_core::net::OscSocket;
use xrt_core::osc::Incoming;

pub enum EngineCmd {
    Trigger(String),
    UpdateConfig(Config),
}

#[derive(Serialize, Clone)]
pub struct StatusEntry {
    pub name: String,
    pub ip: String,
    pub active: bool,
    pub status: LinkStatus,
}

pub const STATUS_EVENT: &str = "xrt://status";

/// Owns the socket and all timing. UI talks to it via EngineCmd only.
pub fn spawn(app: AppHandle, mut config: Config, socket: OscSocket) -> Sender<EngineCmd> {
    let (tx, rx): (Sender<EngineCmd>, Receiver<EngineCmd>) = mpsc::channel();
    std::thread::spawn(move || {
        let mut hb = Heartbeat::new(config.network.heartbeat_timeout_misses);
        let mut last_tick = Instant::now();
        loop {
            // 1) handle a pending command (50ms poll keeps loop responsive)
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(EngineCmd::Trigger(id)) => {
                    for report in socket.send_trigger(&id, &config.targets, config.network.ue_port) {
                        if !report.ok {
                            hb.mark_lost(&report.ip);
                        }
                    }
                }
                Ok(EngineCmd::UpdateConfig(new_config)) => {
                    hb = Heartbeat::new(new_config.network.heartbeat_timeout_misses);
                    config = new_config;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }

            // 2) drain incoming pongs
            while let Some((incoming, from)) = socket.try_recv() {
                if matches!(incoming, Incoming::Pong) {
                    hb.on_pong(&from.ip().to_string());
                }
            }

            // 3) heartbeat tick when interval elapsed
            let interval = Duration::from_millis(config.network.heartbeat_interval_ms);
            if last_tick.elapsed() >= interval {
                last_tick = Instant::now();
                let ips: Vec<String> = config.targets.iter().map(|t| t.ip.clone()).collect();
                for report in socket.send_ping_all(&config.targets, config.network.ue_port) {
                    if !report.ok {
                        hb.mark_lost(&report.ip);
                    }
                }
                hb.on_tick(&ips);
                let payload: Vec<StatusEntry> = config
                    .targets
                    .iter()
                    .map(|t| StatusEntry {
                        name: t.name.clone(),
                        ip: t.ip.clone(),
                        active: t.active,
                        status: hb.status(&t.ip),
                    })
                    .collect();
                let _ = app.emit(STATUS_EVENT, &payload);
            }
        }
    });
    tx
}
```

**주의:** tick 순서가 miss 판정에 의미 있음 — `send_ping_all` 직후 `on_tick`이므로 "이번 ping에 대한 pong"은 다음 iteration의 drain에서 잡히고, 다음 tick 전에 misses가 리셋된다. Task 3의 상태 머신 의미론과 일치.

- [ ] **Step 4: main.rs에 state·commands 연결**

`app/src/main.rs` 전체 교체:

```rust
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod engine;

use std::path::PathBuf;
use std::sync::{mpsc::Sender, Mutex};

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use xrt_core::config::{self, Config, LoadOutcome};
use xrt_core::net::OscSocket;

struct AppState {
    config_path: PathBuf,
    config: Mutex<Config>,
    engine_tx: Sender<engine::EngineCmd>,
    load_warning: Option<String>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(state: State<AppState>, config: Config) -> Result<(), String> {
    config::save(&state.config_path, &config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config.clone();
    let _ = state.engine_tx.send(engine::EngineCmd::UpdateConfig(config));
    Ok(())
}

#[tauri::command]
fn trigger(state: State<AppState>, graphic_id: String) {
    let _ = state.engine_tx.send(engine::EngineCmd::Trigger(graphic_id));
}

#[tauri::command]
fn load_warning(state: State<AppState>) -> Option<String> {
    state.load_warning.clone()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        return;
    }
    // Settings window is opaque by design (D9, 2026-07-03) — readability first,
    // so no transparent flag and no vibrancy here.
    let builder = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
        .title("XRT Settings")
        .inner_size(460.0, 620.0)
        .decorations(false)
        .always_on_top(true);
    match builder.build() {
        Ok(_) => {}
        Err(e) => eprintln!("failed to open settings window: {e}"),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            trigger,
            load_warning,
            open_settings
        ])
        .setup(|app| {
            let win = app.get_webview_window("palette").expect("palette window exists");
            apply_glass(&win);

            let config_path = app
                .path()
                .app_config_dir()
                .expect("app config dir resolvable")
                .join("config.toml");
            let (config, outcome) = config::load(&config_path);
            let load_warning = match outcome {
                LoadOutcome::Loaded => None,
                LoadOutcome::MissingUsedDefault => None, // first run is not an error
                LoadOutcome::ParseErrorUsedDefault(e) => {
                    Some(format!("config.toml is broken, started with defaults: {e}"))
                }
            };

            let socket = OscSocket::bind(config.network.listen_port)
                .expect("failed to bind OSC listen port");
            let engine_tx = engine::spawn(app.handle().clone(), config.clone(), socket);

            app.manage(AppState {
                config_path,
                config: Mutex::new(config),
                engine_tx,
                load_warning,
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn apply_glass(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = window_vibrancy::apply_acrylic(win, Some((20, 24, 32, 120))) {
            eprintln!("acrylic failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
        if let Err(e) = apply_vibrancy(win, NSVisualEffectMaterial::HudWindow, None, Some(14.0)) {
            eprintln!("vibrancy failed (falling back to plain transparency): {e}");
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = win;
    }
}
```

- [ ] **Step 5: 빌드 + 수동 검증 (mock-xr 왕복)**

Run: `cargo build -p xrt-app` → Expected: 컴파일 성공.

수동 검증 (터미널 2개):
1. 터미널 1: `cargo run -p mock-xr` (기본 8000/8001)
2. config 파일에 mock 대상 추가 — `~/.config/xrt-widget/config.toml` (Linux dev; `cargo tauri dev` 첫 실행 후 경로가 없으면 생성):

```toml
[[targets]]
name = "MOCK"
ip = "127.0.0.1"
active = true
```

3. 터미널 2 (app/): `cargo tauri dev` → 우클릭 → Inspect → Console에서:

```js
window.__TAURI__.event.listen('xrt://status', (e) => console.log(e.payload));
```

(`withGlobalTauri: true`라 `window.__TAURI__`가 콘솔에서 사용 가능 — Task 6 conf 참조.)

Expected: 1초마다 `[{name:"MOCK", ip:"127.0.0.1", active:true, status:"Connected"}]` 로그. mock-xr를 끄면 3초 뒤 `status:"Lost"`로 전환.

- [ ] **Step 6: 커밋 게이트 (사용자 직접)**

```bash
git add crates/core/src/heartbeat.rs app/
git commit -m "feat(app): engine thread with heartbeat loop + IPC commands + status events"
```

### Task 8: 팔레트 UI — ipc 래퍼·상태점·트리거·dev harness

**Files:**
- Create: `ui/src/widget/ipc.js`
- Modify: `ui/src/widget/Palette.svelte` (placeholder 교체), `app/capabilities/default.json` (편집 모드 window 권한 추가), `crates/core/src/config.rs` (`[appearance]`·`[window]` 섹션 — Step 2a), `ui/src/shared/tokens.css` (`--btn-fill` 토큰 — Step 2b), `app/src/main.rs` (기동 시 저장된 창 크기 적용 — Step 2c)

**Interfaces:**
- Consumes: Task 7의 commands·`xrt://status` 이벤트
- Produces: `ipc.js` — `{ getConfig, saveConfig, trigger, openSettings, loadWarning, onStatus, onConfigChanged }`. **widget 쪽 모든 Tauri 접근은 이 파일 경유** (Tauri 밖 브라우저에서는 자동으로 mock 동작 → harness·데모 시연 가능).

- [ ] **Step 1: ipc.js — Tauri 래퍼 + 브라우저 mock**

`ui/src/widget/ipc.js`:

```js
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
```

- [ ] **Step 2: Palette.svelte 본 구현 (placeholder 교체)**

`ui/src/widget/Palette.svelte`:

```svelte
<script>
  import GlassPanel from '../shared/GlassPanel.svelte';
  import { getConfig, trigger, openSettings, loadWarning, onStatus, onConfigChanged } from './ipc.js';

  let buttons = $state([]);
  let statuses = $state([]);
  let warning = $state(null);
  let flashId = $state(null);

  $effect(() => {
    let unsubs = [];
    (async () => {
      const config = await getConfig();
      buttons = config.buttons;
      statuses = config.targets.map((t) => ({ ...t, status: 'Unknown' }));
      warning = await loadWarning();
      unsubs.push(await onStatus((list) => (statuses = list)));
      unsubs.push(await onConfigChanged((config) => {
        buttons = config.buttons;
        statuses = config.targets.map((t) => ({ ...t, status: 'Unknown' }));
      }));
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
</script>

<GlassPanel>
  <div class="row">
    <div class="handle" data-tauri-drag-region>☰</div>
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

<style>
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
    background: rgba(255, 255, 255, 0.07);
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
</style>
```

**추가 요구 (2026-07-03 사용자 결정 — spec D8·D9, §6·§7. 위 Step 2 코드와 충돌 시 이 절이 우선):**

- [ ] **Step 2a: core config 확장 (TDD)** — `crates/core/src/config.rs`에 `[appearance]`(`bg_opacity` 0.55, `button_opacity` 0.07, `accent` "#4da3ff", `bg_tint` "#141820")·`[window]`(`width` 720, `height` 96) 섹션 추가. **전 필드 serde default** — 두 섹션이 없는 기존 TOML 로드 시 기본값이 나오는 테스트 + roundtrip 테스트 확장. 기존 테스트는 깨지면 안 됨 (Global Constraints).

- [ ] **Step 2b: appearance 적용** — Palette가 `getConfig()` 및 `onConfigChanged` 수신 시 `config.appearance`를 CSS 변수로 반영: `--glass-bg`(= `bg_tint` RGB + `bg_opacity` 알파 합성), `--accent`, `--btn-fill`(= 흰색 + `button_opacity` 알파). `ui/src/shared/tokens.css`에 `--btn-fill: rgba(255, 255, 255, 0.07)` 토큰 추가, Palette 스타일의 `.trig`/`.gear` 하드코딩 `rgba(255, 255, 255, 0.07)`을 `var(--btn-fill)`로 교체. 브라우저(mock config)에서도 동일 코드 경로로 동작해야 함 — harness에서 확인.

- [ ] **Step 2c: 편집 모드 리사이즈 (D8)** — ☰ 핸들 long-press(~600ms, pointer 이벤트로 touch/마우스 공통 처리) 시 `editMode` 토글:
  - 편집 모드 중: accent 색 테두리로 모드 상태 상시 표시 + 모서리 resize grip 노출. grip 드래그는 **수동 delta 리사이즈**: grip pointer capture → 이동량 계산(client 좌표 = logical px) → `setSize(LogicalSize)` (rAF throttle, min 240×64 clamp). 네이티브 `startResizeDragging`은 macOS undecorated 창에서 동작하지 않아 기각(2026-07-03 실기기 검증). 창은 항상 `resizable=false` 유지 — 프로그램적 `setSize`는 resizable 플래그와 무관하므로 unlock 자체가 불필요(잠금이 더 강해짐).
  - 종료 시 `innerSize()`를 읽어 `config.window`에 반영 후 `saveConfig` — 재기동 시 `app/src/main.rs`의 setup에서 저장된 크기를 적용(`set_size`; Rust 쪽 호출이라 ACL 불요).
  - `app/capabilities/default.json`에 필요한 `core:window:allow-*` 권한 추가 (set-size·inner-size 계열 — 정확한 ACL 이름은 구현 시 Tauri v2 문서/빌드 에러로 확정하고 필요 최소만 추가).
  - Tauri 밖 브라우저에서는 편집 모드 UI는 뜨되 window 호출은 조용히 no-op(기존 ipc.js mock 패턴과 동일).
  - 창 크기 잠금 원칙(Global Constraints): 편집 모드 밖에서 리사이즈 경로가 없어야 한다.

- [ ] **Step 3: harness에서 mock 동작 확인**

Run (ui/): `npm run dev` → 브라우저에서 `http://localhost:5173/harness.html`
Expected: busy 배경 위 팔레트 — 상태점 3색(1초 후 녹색/빨강 갱신), 버튼 터치 시 flash + 콘솔 `[mock] trigger graphic_a`, ⚙ 클릭 시 콘솔 `[mock] openSettings`. 콘솔에 에러 없음.

- [ ] **Step 4: Tauri 실동작 확인 (mock-xr 왕복)**

Task 7 Step 5와 동일 세팅 (터미널 1: `cargo run -p mock-xr`, config에 127.0.0.1 target).
Run (app/): `cargo tauri dev`
Expected:
- 팔레트에 config의 버튼들이 뜸, MOCK 상태점 녹색.
- 버튼 터치 → mock-xr 터미널에 `TRIGGER  graphic_id=...` 출력.
- mock-xr 종료 → 3초 내 상태점 빨강.
- ☰ 드래그로 창 이동 가능.
- ☰ long-press → accent 테두리 + 모서리 grip 표시, grip 드래그로 크기 조절, 다시 long-press로 잠금 복귀. 앱 재시작 후 조절된 크기 유지.
- 평상 모드에서 창 가장자리 드래그로 크기가 바뀌지 않음 (잠금 확인).

- [ ] **Step 5: 커밋 게이트 (사용자 직접)**

```bash
git add ui/ crates/core/src/config.rs app/capabilities/default.json app/src/main.rs
git commit -m "feat(ui): palette with status dots, trigger flash, edit-mode resize, appearance config"
```

### Task 9: 설정 창 — 장비·버튼 관리 + config-changed 전파

**Files:**
- Modify: `ui/src/widget/Settings.svelte` (placeholder 교체), `app/src/main.rs` (save_config에서 이벤트 emit + `quit_app` — Step 2c), `ui/settings.html` (불투명 배경 — Step 2a), `app/capabilities/default.json` (allow-close — Step 2c), `ui/src/widget/ipc.js` (`onAppearancePreview`·preview emit·`quit` 래퍼 — Step 2d)

**Interfaces:**
- Consumes: `ipc.js`의 `getConfig`/`saveConfig`/`loadWarning`/`setSize` (Task 8), command `open_settings` (Task 7)
- Produces: Rust `save_config`가 `"xrt://config-changed"` 이벤트(payload = 새 Config)를 emit — Palette(Task 8)가 이미 구독 중.
- Produces (D10): 전역 이벤트 `"xrt://appearance-preview"` payload `{appearance, window}` (settings → palette, 비영속 preview) · command `quit_app()` (앱 종료) · 설정 창 상단 드래그 바.

- [ ] **Step 1: save_config에서 이벤트 emit**

`app/src/main.rs`의 `save_config`를 다음으로 교체 (`app: AppHandle` 파라미터 추가, `use tauri::Emitter;` 추가):

```rust
#[tauri::command]
fn save_config(app: AppHandle, state: State<AppState>, config: Config) -> Result<(), String> {
    config::save(&state.config_path, &config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config.clone();
    let _ = state.engine_tx.send(engine::EngineCmd::UpdateConfig(config.clone()));
    let _ = app.emit("xrt://config-changed", &config);
    Ok(())
}
```

Run: `cargo build -p xrt-app` → Expected: 컴파일 성공.

- [ ] **Step 2: Settings.svelte 본 구현 (placeholder 교체)**

`ui/src/widget/Settings.svelte`:

```svelte
<script>
  import GlassPanel from '../shared/GlassPanel.svelte';
  import { getConfig, saveConfig, loadWarning } from './ipc.js';

  let config = $state(null);
  let warning = $state(null);
  let savedFlash = $state(false);

  $effect(() => {
    (async () => {
      config = await getConfig();
      warning = await loadWarning();
    })();
  });

  function addTarget() {
    config.targets.push({ name: '', ip: '', active: false });
  }
  function removeTarget(i) {
    config.targets.splice(i, 1);
  }
  function addButton() {
    config.buttons.push({ label: '', graphic_id: '', type: 'trigger' });
  }
  function removeButton(i) {
    config.buttons.splice(i, 1);
  }
  function moveButton(i, delta) {
    const j = i + delta;
    if (j < 0 || j >= config.buttons.length) return;
    [config.buttons[i], config.buttons[j]] = [config.buttons[j], config.buttons[i]];
  }
  async function save() {
    await saveConfig($state.snapshot(config));
    savedFlash = true;
    setTimeout(() => (savedFlash = false), 1200);
  }
</script>

{#if config}
  <GlassPanel>
    <div class="body" data-tauri-drag-region>
      <h1 data-tauri-drag-region>XRT 설정</h1>
      {#if warning}<div class="warning">{warning}</div>{/if}

      <h2>XR 장비</h2>
      {#each config.targets as t, i}
        <div class="grid-row">
          <input placeholder="이름" bind:value={t.name} />
          <input placeholder="IP" bind:value={t.ip} />
          <label class="chk"><input type="checkbox" bind:checked={t.active} /> 활성</label>
          <button class="del" onclick={() => removeTarget(i)}>✕</button>
        </div>
      {/each}
      <button class="add" onclick={addTarget}>+ 장비 추가</button>

      <h2>버튼</h2>
      {#each config.buttons as b, i}
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

      <button class="save" class:done={savedFlash} onclick={save}>
        {savedFlash ? '저장됨 ✓' : '저장'}
      </button>
    </div>
  </GlassPanel>
{/if}

<style>
  .body { padding: 18px 22px; display: flex; flex-direction: column; gap: 10px; }
  h1 { font-size: 16px; margin: 0 0 4px; cursor: grab; }
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
  .chk { display: flex; align-items: center; gap: 4px; color: var(--text); font-size: 13px; }
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
  .save { background: var(--accent); font-size: 15px; margin-top: 14px; min-height: 48px; }
  .save.done { background: var(--status-active); }
  .warning { color: var(--status-lost); font-size: 12px; }
</style>
```

**추가 요구 (2026-07-03 사용자 결정 — spec D9, §7. 위 Step 2 코드와 충돌 시 이 절이 우선):**

- [ ] **Step 2a: 설정 창 불투명화 (D9)** — 설정 창은 불투명 고정 (Task 7의 `open_settings`는 transparent·vibrancy 미적용으로 이미 반영됨). `ui/settings.html`의 body 배경을 `background: #141820`(불투명)으로 변경. Settings.svelte 루트는 GlassPanel 대신 같은 토큰을 쓰는 솔리드 패널 스타일 사용 (backdrop-filter·반투명 배경 금지 — 가독성 우선). 설정 창 투명도를 조절하는 항목은 만들지 않는다.

- [ ] **Step 2b: 섹션 3 — 외형 (D9)** — Settings.svelte에 외형 섹션 추가:
  - `appearance.bg_opacity`·`appearance.button_opacity`: `<input type="range">` 0~1 (step 0.01, % 라벨 표시).
  - `appearance.accent`·`appearance.bg_tint`: `<input type="color">`.
  - 섹션 3에 팔레트 창 크기 `width`/`height` 숫자 입력 추가 (D10 — D8 편집 모드와 공존).

- [ ] **Step 2c: 설정 창 UX — 드래그 바 + 적용/뒤로가기/종료 (D10)**
  - 상단 타이틀 영역을 명시적 **드래그 바**로: `data-tauri-drag-region`은 드래그 바(와 빈 배경)에만 — 입력 요소·버튼에는 절대 금지.
  - 기존 [저장] 버튼을 세 버튼으로 교체: **[적용]** = `saveConfig(draft)` (영속 + `xrt://config-changed` emit → 팔레트 갱신) · **[뒤로가기]** = 미적용 변경 폐기(마지막 저장값 payload로 preview 이벤트 emit해 팔레트 복원) 후 `getCurrentWindow().close()` · **[프로그램 종료]** = 신규 command `quit_app` 호출.
  - `app/src/main.rs`: `#[tauri::command] fn quit_app(app: AppHandle) { app.exit(0); }` + invoke_handler 등록. `app/capabilities/default.json`에 `core:window:allow-close` 추가(정확한 ACL 이름은 빌드로 확정).

- [ ] **Step 2d: 실시간 preview (D10)**
  - 설정 창은 draft 상태를 로컬로 편집. 외형·크기 입력 변경 즉시 전역 이벤트 `xrt://appearance-preview` (payload `{appearance, window}`) emit.
  - 팔레트 쪽: `ipc.js`에 `onAppearancePreview(cb)` listener 추가, Palette가 수신 시 appearance는 CSS 변수로, window 크기는 `setSize`로 **비영속** 반영 (config 저장 없음).
  - [적용] 없이 [뒤로가기] 시: settings가 마지막 저장값 payload로 같은 이벤트를 emit → 팔레트 복원 → 창 닫기.
  - 장비/버튼 목록 변경은 preview 대상 아님 — [적용] 시에만 반영.

- [ ] **Step 3: 왕복 검증**

Run: 터미널 1 `cargo run -p mock-xr`, 터미널 2 (app/) `cargo tauri dev`
Expected:
- ⚙ → 설정 창(**불투명**)이 열림. 두 번 눌러도 창이 중복 생성되지 않음(포커스만).
- 상단 드래그 바로 설정 창 이동 가능, 입력 필드에서는 드래그 안 됨.
- 외형 슬라이더·색상·크기 조작 → **적용 전에 즉시** 팔레트에 preview 반영. [뒤로가기] → 변경 폐기·팔레트가 저장값으로 복원·창 닫힘.
- [적용] → config 저장, 앱 재시작 후에도 유지. [프로그램 종료] → 앱 완전 종료.
- 장비 활성 checkbox 토글 → 저장 → 팔레트 상태점이 즉시 채움/빈 점으로 바뀜.
- 버튼 추가(라벨 `TEST`, graphic_id `test_1`) → 저장 → 팔레트에 즉시 나타나고, 누르면 mock-xr에 `TRIGGER  graphic_id=test_1`.
- 앱 재시작 후에도 설정 유지 (config.toml 저장 확인).

- [ ] **Step 4: 커밋 게이트 (사용자 직접)**

```bash
git add ui/src/widget/Settings.svelte ui/settings.html app/src/main.rs
git commit -m "feat(ui): settings window for targets, buttons and appearance with live palette refresh"
```

### Task 10: 데모 페이지 — 터치 프레젠테이션 콘텐츠

**Files:**
- Modify: `ui/src/demo/Demo.svelte` (placeholder 교체)

**Interfaces:**
- Consumes: `shared/tokens.css`, `shared/GlassPanel.svelte`만. **Tauri API·`widget/` import 금지** (Global Constraints).

- [ ] **Step 1: Demo.svelte 본 구현**

`ui/src/demo/Demo.svelte`:

```svelte
<script>
  import GlassPanel from '../shared/GlassPanel.svelte';

  // Placeholder presentation content (spec D1: minimal-polished, swappable later)
  const slides = [
    { title: 'SBS N센터', body: '터치 프레젠테이션 데모', hint: '옆으로 넘겨보세요 →' },
    { title: '오늘의 아이템', body: '카드를 터치하면 반응합니다', cards: ['아이템 1', '아이템 2', '아이템 3'] },
    { title: 'XR 그래픽', body: '화면 위 팔레트 버튼을 누르면\nXR 장비에서 그래픽이 재생됩니다', hint: '↑ floating 팔레트' },
  ];
  let tapped = $state(null);
</script>

<main>
  {#each slides as slide, i}
    <section>
      <GlassPanel>
        <div class="slide">
          <h1>{slide.title}</h1>
          <p>{slide.body}</p>
          {#if slide.cards}
            <div class="cards">
              {#each slide.cards as card, j}
                <button
                  class="card"
                  class:tapped={tapped === `${i}-${j}`}
                  onclick={() => (tapped = `${i}-${j}`)}
                >
                  {card}
                </button>
              {/each}
            </div>
          {/if}
          {#if slide.hint}<span class="hint">{slide.hint}</span>{/if}
        </div>
      </GlassPanel>
    </section>
  {/each}
</main>

<style>
  :global(body) {
    background:
      radial-gradient(1200px 600px at 20% 10%, #1b3a5c 0%, transparent 60%),
      radial-gradient(1000px 700px at 80% 90%, #23504a 0%, transparent 60%),
      #0d1117;
  }
  main {
    display: flex;
    height: 100vh;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
  }
  section {
    flex: 0 0 100vw;
    scroll-snap-align: center;
    display: grid;
    place-items: center;
  }
  .slide {
    padding: 56px 72px;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 18px;
    align-items: center;
  }
  h1 { margin: 0; font-size: 44px; color: var(--text); }
  p { margin: 0; font-size: 20px; color: var(--text-dim); white-space: pre-line; }
  .cards { display: flex; gap: 16px; margin-top: 12px; }
  .card {
    min-width: 160px;
    min-height: 100px;
    border: 1px solid var(--glass-border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.06);
    color: var(--text);
    font-size: 18px;
    cursor: pointer;
    transition: transform 0.12s ease, background 0.2s ease;
  }
  .card.tapped { background: var(--accent); transform: scale(1.05); }
  .hint { font-size: 14px; color: var(--text-dim); }
</style>
```

- [ ] **Step 2: 브라우저 확인**

Run (ui/): `npm run dev` → `http://localhost:5173/demo.html`
Expected: 그라데이션 배경 위 glass 슬라이드 3장, 가로 스크롤(터치 스와이프) 시 slide snap, 카드 터치 시 accent 색 반응. 콘솔 에러 없음. 위젯 팔레트와 같은 토큰(색·radius·blur)이라 시각적으로 한 시스템으로 보임.

- [ ] **Step 3: 커밋 게이트 (사용자 직접)**

```bash
git add ui/src/demo/
git commit -m "feat(demo): touch presentation slides sharing design tokens"
```

### Task 11: GitHub Actions 빌드 + Windows 실장비 체크리스트

**Files:**
- Create: `.github/workflows/build.yml`, `docs/windows-checklist.md`

**Interfaces:**
- Consumes: workspace 전체 (테스트), `app/` (tauri build), `ui/package-lock.json` (Task 5 커밋에 포함되어 있어야 `npm ci` 동작 — 없으면 Task 5 커밋 확인)

- [ ] **Step 1: workflow 작성**

`.github/workflows/build.yml`:

```yaml
name: build

on:
  push:
    branches: [main]
  workflow_dispatch: # 수동 실행 버튼

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
      - run: cargo test --workspace
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci --prefix ui
      - run: npm run build --prefix ui

  build-windows:
    needs: test
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci --prefix ui
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
      - run: npm install -g @tauri-apps/cli
      - run: tauri build
        working-directory: app
      - uses: actions/upload-artifact@v4
        with:
          name: xrt-widget-windows
          path: |
            target/release/bundle/nsis/*
            target/release/bundle/msi/*
```

(리눅스 test job이 먼저 통과해야 windows 빌드 실행 — 비싼 runner 낭비 방지. `tauri build`는 npm 배포판 CLI라 설치가 빠름.)

- [ ] **Step 2: 체크리스트 문서 작성**

`docs/windows-checklist.md`:

```markdown
# Windows 실장비 수동 검증 체크리스트

CI artifact(xrt-widget-windows)를 터치스크린 PC에 설치 후 확인.
spec §9 "최종 검증" 단계 — Mac/Linux에서 검증 불가한 항목만 모음.

## 설치
- [ ] Windows 버전 확인 후 기록 (10 / 11 — spec §12 미해결 항목 해소)
- [ ] WebView2 런타임 존재 확인 (없으면 installer가 안내하는지)
- [ ] NSIS installer로 설치 → 실행됨

## Glass / 창
- [ ] acrylic 재질이 실제로 보임 (창 뒤 콘텐츠가 블러되어 비침)
- [ ] Win11이라면: mica로 바꿔볼 가치가 있는지 육안 비교 메모
- [ ] 팔레트가 데모 페이지·다른 앱 위에 항상 떠 있음 (always-on-top)
- [ ] 팔레트 창 밖 영역의 터치가 아래 앱에 정상 전달됨

## 터치
- [ ] 버튼 터치 정확도 (오터치·무반응 없음, flash 피드백 보임)
- [ ] ⠿ 핸들 터치 드래그로 창 이동
- [ ] 설정 창의 input에 터치 키보드로 입력 가능

## 왕복 (mock-xr 또는 실제 UE)
- [ ] 트리거가 수신됨 (mock-xr 로그 또는 UE 그래픽 재생)
- [ ] 상태점: 장비 켜짐=녹색, 끔=3초 내 빨강, 재기동=녹색 복귀
- [ ] 설정 변경(장비/버튼) 저장 → 재시작 후 유지

## 장기 상주 smoke
- [ ] 반나절 이상 켜둔 뒤: 메모리 증가 추세·상태점 정상·트리거 즉응성
```

- [ ] **Step 3: 커밋 게이트 + push (사용자 직접)**

```bash
git add .github/ docs/windows-checklist.md
git commit -m "ci: windows build workflow + on-device verification checklist"
```

push까지 하면 (사전에 GitHub repo 생성 필요 — `gh auth status`로 SBS-NCENTER 활성 확인 후 `gh repo create`):

```bash
git push -u origin main   # -u: 이후 git push만으로 되도록 upstream 지정
```

Expected: Actions 탭에서 test → build-windows 순차 실행, artifact `xrt-widget-windows` 생성. **push 후 INDEX.md의 last-pushed 갱신 잊지 말 것 (워크스페이스 규약).**

---

## 실행 순서 요약

```text
Task 1 ─ workspace + config      ┐
Task 2 ─ OSC codec + socket      │ core (Linux, cargo test로 완결)
Task 3 ─ heartbeat               │
Task 4 ─ mock-xr                 ┘
Task 5 ─ ui 스캐폴드              ┐
Task 6 ─ Tauri 셸 + glass        │ 셸·UI (Linux/Mac, 수동 검증 병행)
Task 7 ─ engine + IPC            │
Task 8 ─ 팔레트                  │
Task 9 ─ 설정 창                 ┘
Task 10 ─ 데모 페이지             — 독립 (Task 5 이후 언제든)
Task 11 ─ CI + 체크리스트         — push 시점
이후: Windows 실장비 검증 (docs/windows-checklist.md) → acrylic/mica·터치 튜닝
```





