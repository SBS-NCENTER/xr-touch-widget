# XR-Touch_to_OSC — HTTP(URL) 액션 설계 스펙 (D16)

- **날짜**: 2026-07-15
- **상태**: 승인됨 (brainstorming 세션, 사용자 승인)
- **다음 단계**: writing-plans로 구현 계획 작성
- **선행 스펙**: `2026-07-02-xr-touch-trigger-design.md` (D1~D14), `2026-07-09-portable-build-design.md` (D15)

## 1. 개요 / 동기

현재 버튼은 OSC 메시지 1개만 보낸다(D14). 여기에 **HTTP GET 호출 액션**을 추가한다. 1차 use case는 **Pixotope Gateway API로 카메라 컷**:

```
http://10.10.204.184:16208/gateway/25.2.4/publish?Type=Call&Target=Store&Method=SetCameraSet&ParamNumber=0
```

Pixotope 공식 문서 기준, Gateway는 `http://<IP>:16208/gateway/<version>/publish`로 **HTTP GET**을 받으며 GET만으로 상태 변경을 허용한다(`SetCameraSet` + `ParamNumber` = 카메라 입력 전환). 현재 이 조작은 Bitfocus Companion + Stream Deck으로 하고 있고, 같은 API를 이 위젯의 터치 버튼에서 직접 쏘게 한다.

- 참고: [Pixotope Gateway API](https://help.pixotope.com/phc/24.3/pixotope-gateway-api), [Configure camera input switching](https://help.pixotope.com/phc/26.1/configure-camera-input-switching)

## 2. 결정 (D16)

```text
┌─────┬──────────────────────┬────────────────────────────────────────────────┐
│ #   │ 항목                 │ 확정 내용                                        │
├─────┼──────────────────────┼────────────────────────────────────────────────┤
│ D16 │ 버튼 = 액션 리스트    │ 버튼 하나가 액션 N개를 순서대로 발사(Companion   │
│     │ (2026-07-15)         │ 모델). 액션 타입 = `osc`(기존 메시지 스펙) 또는  │
│     │                      │ `http`(URL 통째로 GET). 새 액션 기본 타입 =      │
│     │                      │ http("URL 우선"). 실패 시 해당 버튼 1.5초 빨간   │
│     │                      │ 깜박임. HTTP 발사는 Rust engine 계층에서 처리.   │
└─────┴──────────────────────┴────────────────────────────────────────────────┘
```

**세부 확정 사항** (brainstorming에서 사용자 선택):

- **버튼 모델**: 버튼 1개 = 액션 리스트(순서 있는 N개, OSC/HTTP 혼합 가능).
- **URL 우선**: HTTP 액션은 브라우저에 치던 **URL 한 줄을 통째로** 입력(Pixotope 전용 구조화 폼 아님 — 범용). 새 액션/새 버튼의 기본 타입도 http.
- **실패 피드백**: 액션 실패 시 팔레트의 해당 버튼이 1.5초 빨간 표시. 기존에 로그로만 남던 **OSC 전송 실패도 같은 배선**에 태운다.
- **구현 위치**: 후보 A(Rust engine)/B(웹뷰 fetch)/C(tauri-plugin-http) 중 **A 확정**. B는 CORS(Gateway가 CORS 헤더를 준다는 보장 없음)로 탈락, C는 발사 경로가 OSC와 갈라져 실패감지 로직이 이중화되므로 탈락.

## 3. 아키텍처 / 데이터 흐름

```text
버튼 탭 (Palette.svelte)
   │  press(button_index)              ← UI는 "몇 번 버튼"인지만 전달
   ▼
Tauri command `press` (main.rs)
   │  EngineCmd::Press { index }
   ▼
engine 스레드 ── 자기 config의 buttons[index].actions를 순서대로 발사
   ├─ Action::Osc  → 기존 send_trigger (UDP, active target 전원)   [inline]
   └─ Action::Http → 액션당 std::thread spawn → HTTP GET (timeout 3s)
                        │
                        └─ 실패(연결불가/timeout/비-2xx) 시
                           `xrt://press-error` { button_index, detail } emit
                              → Palette가 해당 버튼 1.5초 빨간 깜박임
```

- **index 기반 press**: 지금은 UI가 OSC 주소/값을 직접 들고 `trigger`를 부른다. D16부터 UI는 index만 보내고 **engine이 자기 config에서 액션을 조회**한다(설정 적용 직후 UI/engine 상태 어긋남 창구 제거). index가 범위 밖이면 무시 + `eprintln!`. 기존 `trigger` command는 `press`로 대체(잔존 호출자 없음 확인 후 제거).
- **비블로킹 발사**: engine 스레드는 heartbeat도 돌리는 단일 루프다. HTTP를 그 자리에서 기다리면 Pixotope가 꺼져 있을 때 timeout 3초 동안 상태점·다음 press가 전부 밀린다. 따라서 **HTTP 액션마다 짧은 스레드를 spawn**해 발사하고(AppHandle clone 소지, 실패 시 스스로 emit), engine은 즉시 다음으로 넘어간다. 사용자 조작 빈도(초당 수 회)에서 스레드 비용은 무시 가능.
- **순서 의미**: 액션 리스트는 **순서대로 발사하되 응답은 기다리지 않는다**(dispatch order만 보장). OSC(UDP)와 HTTP가 수 ms 안에 연달아 나가는 그림이며, 응답 대기 직렬화는 라이브 지연을 만들므로 하지 않는다.

## 4. config 스키마 + 하위호환

### 4.1 새 형식

```toml
[[buttons]]
label = "CAM 1"

[[buttons.actions]]
type = "http"
url = "http://10.10.204.184:16208/gateway/25.2.4/publish?Type=Call&Target=Store&Method=SetCameraSet&ParamNumber=0"

[[buttons.actions]]
type = "osc"
address = "/xrt/graphic"
value_type = "string"
value = "cam1_lower"
```

```rust
// crates/core/src/config.rs
#[derive(..., Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    Osc { address: String, value_type: ValueType, value: String },
    Http { url: String },
}

pub struct ButtonDef {
    pub label: String,
    pub actions: Vec<Action>,
    // + 로드 전용 legacy 필드(§4.2), 저장 시 미기록
}
```

### 4.2 하위호환 (v0.2.x flat 필드 → 자동 마이그레이션)

pre-D16 config.toml의 `[[buttons]]`는 flat한 `address`/`value`/`value_type`을 가진다. **로드 시**: 버튼에 `actions`가 하나도 없으면 legacy 필드(+D14 per-field 기본값: address=`/xrt/graphic`, value=`""`, value_type=`string`)로 **OSC 액션 1개를 합성**한다. label만 있는 버튼도 pre-D16과 동일하게 동작(기본 OSC 메시지 발사)이 보존된다. **저장 시**: 항상 새 형식으로만 쓴다(legacy 필드 drop — `skip_serializing`).

"actions 없음 = legacy"가 안전하려면 **액션 0개 버튼이 존재하지 않아야** 한다 → 설정 UI의 [적용] 검증에서 **버튼당 액션 ≥ 1**을 강제한다(§6). 아무것도 안 하는 버튼은 설정 실수이므로 UX로도 타당.

## 5. core HTTP 모듈 (`crates/core/src/http.rs` 신규)

- 소형 blocking HTTP client **`ureq`** 사용, `default-features = false`(현장 장비는 사설망 http — TLS 스택 제외로 빌드 경량화; https가 필요해지면 feature만 추가).
- `pub fn get(url: &str) -> Result<(), String>` — timeout **3초 상수**, **2xx = 성공**, 연결불가/timeout/비-2xx/URL 파싱 실패 = `Err(사유)`. 응답 body는 읽어서 버린다(내용 해석 안 함).
- core에 두는 이유: Tauri 의존 없이 단위테스트 가능(테스트에서 `std::net::TcpListener` 미니 서버로 200/500/무응답 케이스 검증 — mock-xr과 같은 철학).

## 6. UI

### 6.1 Settings.svelte — 버튼 편집 = label + 액션 리스트

- 액션 행마다 타입 select `[URL 호출 | OSC 메시지]` — **기본값 URL 호출**.
  - `http` 행: URL 입력칸 한 줄(placeholder = Pixotope 예시 URL).
  - `osc` 행: 기존 주소/타입/값 위젯 그대로 재사용.
- 행마다 [삭제], 버튼마다 [+ 액션 추가]. **순서변경(↑↓) UI는 v1 생략**(사용자 확정 — 응답 비대기 발사라 순서의 실질 영향이 수 ms).
- 새 버튼 생성 시 기본 = 빈 URL의 http 액션 1개.
- [적용] 검증(기존 D14 검증 패턴에 추가): 버튼당 액션 ≥ 1 / http 액션 URL은 trim 후 비어있지 않고 **`http://`로만 시작**(https는 별도 메시지로 거부) / osc 액션은 기존 D14 검증 그대로.
  - *(2026-07-15 최종 리뷰 수정: 원래 이 줄은 `https://`도 허용했으나 §5의 TLS-제외 결정과 모순 — TLS 없는 빌드에서 https는 런타임 100% 실패라 검증이 미리 막아야 한다. ureq `tls` feature를 켜는 시점에 다시 허용.)*

### 6.2 Palette.svelte — press + 빨간 깜박임

- 버튼 탭 → `press(index)` 호출(기존 trigger 호출 대체).
- `xrt://press-error` 구독 → payload의 button_index 버튼에 1.5초 빨간 테두리/배경 클래스(재발생 시 타이머 리셋). 기존 "마지막 누름 강조"(D12)와 독립적으로 동작.

### 6.3 ipc.js — 브라우저 harness 미러

- `trigger(address, valueType, value)` → `press(index)`로 교체, `onPressError(cb)` 추가(Tauri 밖 no-op).
- mockConfig.buttons를 새 shape(`{label, actions:[...]}`)로 갱신 — harness/데모 미리보기가 프로덕션과 동일 shape을 유지해야 한다는 기존 원칙 유지.

## 7. 범위 경계 (YAGNI)

**포함**: Action enum + 마이그레이션, core http 모듈, engine Press 실행 모델, 설정 액션 리스트 UI, press-error 빨간 깜박임(OSC 실패 포함), 테스트.

**제외(확장 여지는 열어둠)**:
- 액션 **순서변경 UI**(↑↓) — 필요해지면 다음 버전.
- **POST/body/헤더** — Pixotope 1차 use case는 GET으로 충분. Action::Http에 필드 추가로 확장 가능.
- **응답 JSON 해석** — Pixotope는 200 + CallResult JSON을 주지만 v1은 HTTP 레벨 성공/실패만 판정.
- **HTTP 대상 health-check 상태점** — OSC target 상태점 같은 주기 확인은 범위 밖(실패 피드백은 press 시점 빨간 깜박임으로 커버).
- **액션 간 delay/응답 대기 직렬화** — 라이브 지연 유발, 요구 없음.

## 8. 테스트 / 검증 경계 (개발기 = Mac)

```text
┌────────────────────────────────────┬─────────────────────────────────────────┐
│ 대상                               │ 검증 위치                                 │
├────────────────────────────────────┼─────────────────────────────────────────┤
│ config: Action roundtrip·legacy    │ Mac `cargo test --workspace`             │
│ 마이그레이션(label-only 포함)       │ (기존 D14/D15 하위호환 테스트 패턴)       │
├────────────────────────────────────┼─────────────────────────────────────────┤
│ core http::get 성공/실패 판정       │ Mac 단위테스트 — TcpListener 미니 서버    │
│ (200/500/연결불가/timeout)          │ (mock-xr 패턴)                           │
├────────────────────────────────────┼─────────────────────────────────────────┤
│ UI 액션 편집·검증·빨간 깜박임        │ Mac 수동(브라우저 harness + tauri dev)    │
├────────────────────────────────────┼─────────────────────────────────────────┤
│ 실제 Pixotope 카메라 컷             │ Windows 방송 PC + Pixotope 가동 상태에서  │
│                                    │ 수동 확인(현재 Pixotope off — 추후 세션)  │
└────────────────────────────────────┴─────────────────────────────────────────┘
```

버전: 구현 완료 후 **v0.3.0** 태그(기존 release.yml 파이프라인 그대로).

## 9. 영향받는 파일 (예상)

- `crates/core/src/config.rs` — Action enum, ButtonDef.actions + legacy 마이그레이션 + 테스트.
- `crates/core/src/http.rs` — 신규: ureq GET wrapper + 단위테스트.
- `crates/core/src/lib.rs`, `crates/core/Cargo.toml` — 모듈 등록, ureq 의존성.
- `app/src/engine.rs` — `EngineCmd::Press { index }`, 액션 dispatch(OSC inline / HTTP thread spawn), `xrt://press-error` emit(OSC 실패 포함).
- `app/src/main.rs` — `press(index)` command 추가·`trigger` 제거, invoke_handler 갱신.
- `ui/src/widget/Settings.svelte` — 액션 리스트 편집 UI + 검증.
- `ui/src/widget/Palette.svelte` — press(index) 호출, press-error 빨간 깜박임.
- `ui/src/widget/ipc.js` — press/onPressError wrapper, mockConfig 새 shape.

*(File Structure의 `[create]`/`[modify]` 최종 판정은 plan 단계에서 실제 repo 상태로 확정한다.)*
