# XR-Touch_to_OSC — 설계 스펙

- **날짜**: 2026-07-02
- **상태**: 승인됨 (사용자 승인, brainstorming 세션에서 D1~D7 단계별 확정)
- **다음 단계**: writing-plans로 구현 계획 작성

## 1. 개요

터치스크린이 연결된 Windows PC 최상단(always-on-top)에 떠 있는 glass 디자인 **floating 버튼 팔레트**. 버튼을 터치하면 OSC 메시지가 활성 XR 장비(Unreal Engine)들로 전송되어 그래픽 재생을 트리거한다. 프레젠테이션 데모용 웹페이지가 같은 repo에 동거하며 디자인 언어를 공유한다.

**사용 시나리오 (데모)**: 발표자가 터치스크린의 풀스크린 데모 웹페이지를 터치로 조작하며 프레젠테이션을 진행하다가, XR 그래픽이 필요한 순간 화면 위에 떠 있는 팔레트의 버튼을 터치 → XR 장비들에서 그래픽 재생.

## 2. 결정 이력 (요약)

```text
┌────┬───────────────────────┬─────────────────────────────────────────────┐
│ #  │ 결정 사항              │ 확정 내용                                    │
├────┼───────────────────────┼─────────────────────────────────────────────┤
│ D1 │ 스코프·요구사항        │ 활성 대상 스위칭(여러 개 가능), stateless     │
│    │                       │ 트리거 버튼, ⚙ 설정 창, 수동 실행,           │
│    │                       │ 데모 페이지 = 미니멀-폴리시드                │
│ D2 │ 신뢰성 모델            │ 트리거 단발 전송(반복 없음 — 이중 재생 회피) │
│    │                       │ + heartbeat 1초 주기 연결 상태 표시          │
│ D3 │ 프로토콜               │ OSC (UE 공식 plugin 내장, 업계 표준)         │
│ D4 │ 스택                   │ Rust + Tauri + Svelte                        │
│ D5 │ 창 구조                │ 컴팩트 팔레트 창 하나 (가로 바, 드래그 이동) │
│ D6 │ 프로젝트 구조          │ 한 repo, 단일 Vite 프로젝트 엔트리 2개,      │
│    │                       │ core crate 분리, x-dependency-owners 장부    │
│ D7 │ 빌드                   │ GitHub Actions windows runner,               │
│    │                       │ UI 반복은 LAN dev server                     │
└────┴───────────────────────┴─────────────────────────────────────────────┘
```

**주요 기각 사유 기록**:
- 트리거 반복 전송(dedupe 방식) 기각 — UE 쪽 dedupe 구현이 삐끗하면 "그래픽 이중 재생"이라는 눈에 보이는 방송 사고. 유선 LAN에서 UDP 단발 유실은 매우 드물어, 위험 교환상 단발이 우세 (조용한 유실(희귀) < 이중 재생(치명)).
- UE Remote Control API 기각 — HTTP 응답의 heartbeat 이점이 OSC ping/pong 채택으로 무의미해짐. 남는 건 세팅 부담·packaged build 확인 필요·운영 인력에게 낯섦.
- 풀스크린 투명 오버레이 기각 — 터치는 hover가 없어 hit-test 기반 click-through가 신뢰성 있게 동작하지 않음. 아래 페이지 터치를 삼키는 사고 위험.
- 트리거별 ACK 기각 — 버튼별 응답 추적은 사실상 상태 관리로, stateless 버튼 모델 취지와 충돌. UE 쪽 작업량 대비 이득 부족.

## 3. 요구사항

### 기능
- floating 버튼 팔레트: 버튼 여러 개, 설정에서 추가/삭제/편집 가능.
- 버튼 = **stateless 트리거**: 누르면 OSC 메시지 한 발. IN/OUT이 필요하면 버튼 두 개로 구성.
- 버튼 정의: 라벨 + 그래픽 식별자 문자열(`graphic_id`) + `type` 필드(v1은 `trigger`뿐, 확장용).
- XR 장비 여러 대 등록, **활성 대상 스위칭**: checkbox 방식으로 여러 대 동시 활성 가능. 트리거는 활성 대상에만 전송.
- ⚙ 설정 창: XR 장비 관리(추가/삭제/IP/활성) + 버튼 관리(추가/삭제/라벨/graphic_id/순서).
- heartbeat: 장비별 연결 상태를 팔레트에 상시 표시.
- 데모 웹페이지: 터치 카드/슬라이드 몇 장의 미니멀-폴리시드 프레젠테이션 콘텐츠. 위젯과 디자인 언어(glass) 통일.

### 비기능
- glass 디자인 (반투명 + 배경 블러), 터치 최적화.
- Windows 10/11 모두 지원 (실 장비 버전 확인 대기 중 — 미해결 항목 §12 참조).
- 장기 상주 안정성 (방송 환경).
- core 로직은 GUI 없이 재사용 가능해야 함.
- 수동 실행 (부팅 자동 시작 불필요).

## 4. 아키텍처

```text
터치스크린 PC (Windows)                          XR 장비 1..N (UE + OSC plugin)
┌─────────────────────────────┐                ┌──────────────────────────────┐
│ 위젯 (Tauri)                 │   /xrt/graphic │ OSC Server (Blueprint)       │
│ ┌─────────────────────────┐ │   "graphic_id" │  주소 바인딩 1개              │
│ │ ui: Svelte (widget.html) │ │ ──────────────►│  → String arg로 Switch       │
│ ├─────────────────────────┤ │  UDP :8000     │  → 그래픽 플레이              │
│ │ app: 창 관리·vibrancy    │ │                │                              │
│ ├─────────────────────────┤ │   /xrt/ping    │                              │
│ │ core: config·OSC·       │ │ ──────────────►│  ping 수신 시                 │
│ │       heartbeat          │ │ ◄────────────── │  /xrt/pong 회신              │
│ └─────────────────────────┘ │  UDP :8001     │                              │
└─────────────────────────────┘                └──────────────────────────────┘
```

### OSC 메시지 규격

```text
┌──────────────┬────────────────┬──────────────────────┬─────────────────────────┐
│ 메시지       │ 방향           │ 주소 + 인자          │ 규칙                    │
├──────────────┼────────────────┼──────────────────────┼─────────────────────────┤
│ 트리거       │ 위젯 → UE      │ /xrt/graphic         │ 단발 전송, 활성 대상에만 │
│              │ (:8000)        │ arg0: string         │ 반복 전송 금지          │
│              │                │ (graphic_id)         │                         │
│ ping         │ 위젯 → UE      │ /xrt/ping (인자 없음)│ 1초 주기,               │
│              │ (:8000)        │                      │ 모든 등록 대상에 전송    │
│              │                │                      │ (비활성 포함 — 스위칭    │
│              │                │                      │  전에 상태 확인 목적)    │
│ pong         │ UE → 위젯      │ /xrt/pong (인자 없음)│ ping 수신 시 즉시 회신   │
│              │ (:8001)        │                      │                         │
└──────────────┴────────────────┴──────────────────────┴─────────────────────────┘
```

- 연결 판정: pong **3회 연속 누락**(기본값, config 조정 가능) 시 해당 장비 "끊김" 표시.
- 트리거를 주소가 아닌 string arg로 식별하는 이유: UE Blueprint에서 주소 바인딩 1개 + `Switch on String`으로 처리가 가장 단순.
- 포트 기본값: UE 수신 8000, 위젯 pong 수신 8001. 모두 config에서 변경 가능.

### UE 쪽 요구 작업 (repo 밖, 인터페이스 계약)
1. OSC plugin 활성화 (엔진 내장, 별도 설치 없음).
2. OSC Server 생성 (수신 포트 8000).
3. `/xrt/graphic` 바인딩 → string arg로 그래픽 재생 분기.
4. `/xrt/ping` 바인딩 → 발신자에게 `/xrt/pong` 회신 (OSC Client 노드).

## 5. Repo 구조

```text
XR-Touch_to_OSC/
├── crates/
│   ├── core/            # 순수 로직 crate (GUI 없음 = 재사용 단위)
│   │                    #   - 버튼·대상 모델, config 저장/로드(TOML)
│   │                    #   - OSC 인코딩·전송 (rosc)
│   │                    #   - heartbeat 엔진 (상태 머신)
│   └── mock-xr/         # CLI mock 수신기 — UE 없이 개발·테스트용
│                        #   트리거 로그 출력 + /xrt/ping에 /xrt/pong 응답
│                        #   core의 "첫 번째 소비자"로서 재사용 경계 검증
├── app/                 # Tauri 앱: 창 생성, always-on-top, window-vibrancy,
│                        #   core 호출, UI와 IPC (command/event)
├── ui/
│   ├── src/
│   │   ├── shared/      # 공유 컴포넌트 + 디자인 토큰 (색·블러·radius)
│   │   ├── widget/      # 팔레트·설정 창 UI
│   │   └── demo/        # 프레젠테이션 데모 페이지
│   ├── widget.html      # Vite 엔트리 1 — Tauri가 로드
│   ├── demo.html        # Vite 엔트리 2 — 브라우저(Edge kiosk)로 열기
│   └── package.json     # "x-dependency-owners" 필드로 의존성 소유권 장부
└── docs/superpowers/{specs,plans}   # canonical (vault에 mirror)
```

**import 방향 규칙**: `widget/`과 `demo/`는 `shared/`를 import할 수 있으나 **서로는 import 금지**. 데모를 나중에 분리할 때 `demo/` + `shared/` 복사로 끝나도록.

**의존성 장부** (`package.json`은 JSON이라 주석 불가 → 커스텀 필드 사용, npm/pnpm은 무시):

```json
{
  "x-dependency-owners": {
    "shared":      ["svelte", "vite"],
    "widget-only": ["@tauri-apps/api"],
    "demo-only":   []
  }
}
```

새 의존성 추가 시 반드시 세 목록 중 하나에 분류한다.

## 6. Config 스키마 (TOML)

설정 창이 읽고 쓴다. 위치: OS 표준 config 디렉토리 (Windows `%APPDATA%`, Linux `~/.config` — Tauri path API 사용).

```toml
[network]
ue_port = 8000                  # XR 장비가 듣는 포트
listen_port = 8001              # pong 수신 포트
heartbeat_interval_ms = 1000
heartbeat_timeout_misses = 3

[[targets]]
name = "XR-1"
ip = "192.168.0.10"
active = true                   # checkbox — 여러 개 true 가능

[[buttons]]
label = "그래픽 A"
graphic_id = "lower_third_a"
type = "trigger"                # v1엔 "trigger"뿐 — 스키마 확장용 필드
```

## 7. UI/UX

### 팔레트 (메인 창)
- 가로 바 형태, 기본 위치 우상단, always-on-top, frameless.
- 구성: `[⠿ 핸들][●●○ 상태점][버튼들…][⚙]`
  - **⠿ 핸들**: 드래그로 팔레트 이동.
  - **상태점**: 등록 장비당 점 하나 — 활성=채운 점, 비활성=빈 점, 끊김=빨강. "지금 트리거가 어디로 가는지"가 항상 보이는 것이 활성 스위칭 모델의 모드 실수 방지책.
  - **⚙**: 설정 창 열기.
- 창 크기 = 콘텐츠 크기 (아래 데모 페이지의 터치를 가리지 않음 — 팔레트 창 구조 선택의 핵심 근거).
- 접기(collapse) 기능은 v1 보류.

### 설정 창
- ⚙ 터치 시 별도 glass 창으로 열림.
- 섹션 1 — XR 장비: 추가/삭제, 이름, IP, 활성 checkbox.
- 섹션 2 — 버튼: 추가/삭제, 라벨, graphic_id, 순서 변경.
- 저장 시 config 파일에 쓰고 팔레트에 즉시 반영.

### Glass 구현
```text
┌─────────────┬──────────────────────────────────────────────┐
│ 환경        │ 방식                                         │
├─────────────┼──────────────────────────────────────────────┤
│ Windows 11  │ window-vibrancy: acrylic 또는 mica (최종)    │
│ Windows 10  │ window-vibrancy: acrylic (구 API 경로, 최종) │
│ macOS (개발)│ window-vibrancy: vibrancy — 진짜 블러로      │
│             │ 디자인 방향 미리보기 (재질은 acrylic과 다름) │
│ Linux (개발)│ 단순 반투명 fallback (blur 없음)             │
└─────────────┴──────────────────────────────────────────────┘
```
버튼 모양·터치 피드백·애니메이션·타이포는 전부 CSS (Svelte 컴포넌트).

### 데모 페이지
- 터치로 넘기고 조작하는 카드/슬라이드 몇 장. `shared/` 디자인 토큰 사용으로 위젯과 통일감.
- 정적 빌드 산출물을 Edge 전체화면(kiosk 모드)으로 실행.

## 8. 에러 처리

- **config 파손/파싱 실패**: 기본값으로 기동하고 설정 창에 경고 표시. 안 뜨는 것보다 뜨는 게 낫다.
- **OSC 전송 실패 (소켓 에러)**: 해당 장비 상태점 즉시 빨강 + 로그 기록.
- **heartbeat 끊김**: 표시만 한다. **빨간 장비로도 트리거 전송은 막지 않음** — 라이브에서 소프트웨어가 오퍼레이터의 판단을 이기면 안 된다.
- **UDP 유실**: 감수한다 (D2 결정). heartbeat로 사전 감지가 방어선.

## 9. 테스트 전략

```text
┌──────────────────────┬──────────────────────────────────────┬─────────┐
│ 대상                 │ 방법                                 │ 환경    │
├──────────────────────┼──────────────────────────────────────┼─────────┤
│ core: config         │ 단위 테스트 — 저장/로드 roundtrip,   │ Linux   │
│                      │ 파손 파일 → 기본값 fallback          │ (cargo) │
│ core: OSC 인코딩     │ 단위 테스트 — rosc 인코딩/디코딩     │ Linux   │
│ core: heartbeat      │ 단위 테스트 — 상태 머신 (모의 시간)  │ Linux   │
│ 전 구간 통합         │ mock-xr 상대로 loopback:             │ Linux   │
│                      │ 트리거 수신·pong 왕복·끊김 판정      │         │
│ glass 방향·창 동작   │ Tauri 실행 — vibrancy 미리보기,      │ Mac     │
│                      │ frameless·topmost 기능 확인          │         │
│ Blink 렌더링 근사    │ Vite dev URL을 Chrome으로 열어       │ Mac     │
│                      │ Windows WebView2 렌더링 사전 확인    │         │
│ UI 터치·CSS          │ LAN dev server → 터치스크린 Edge     │ 실장비  │
│ 최종 검증            │ 수동 체크리스트: acrylic/mica 재질,  │ 실장비  │
│                      │ always-on-top 실동작, 터치 정확도    │         │
└──────────────────────┴──────────────────────────────────────┴─────────┘
```

## 10. 개발·빌드 워크플로우

### 검증 티어

```text
Linux/Mac (일상 개발: core·UI·데모)
   → Mac (glass 방향 미리보기 · 창 동작 · Blink 렌더링 근사 · 장기 smoke)
      → Windows/실장비 (acrylic/mica 최종 재질 튜닝 · 터치)
```

**webview 엔진 사실**: Tauri는 OS 내장 webview 고정 — 설치된 Chrome을 쓸 수 없다. Windows = WebView2(Chromium/Blink), macOS = WKWebView(WebKit), Linux = WebKitGTK(WebKit). 즉 렌더링 엔진 관점에서 Mac은 Linux와 같은 가족이고 Windows가 유일한 Blink다.

- **개발 위치**: core, mock-xr, UI(Svelte/CSS), 데모 페이지는 Linux/Mac 어디서든 (Tauri dev 양쪽 동작).
- **Mac의 고유 역할 1 — vibrancy 미리보기**: window-vibrancy가 macOS vibrancy를 지원해, Linux에선 불가능한 "진짜 블러 glass"로 디자인 방향(블러 강도·투명도·대비)을 CI 빌드 없이 튜닝. 재질이 acrylic/mica와 달라 시각 최종 사인오프는 Windows.
- **Mac의 고유 역할 2 — Blink 렌더링 근사**: Vite dev URL을 Mac의 Chrome으로 열면 위젯 UI의 Chromium 렌더링(= Windows WebView2 근사)을 사전 확인. 브라우저 탭에는 창 투명·블러가 없으므로 **dev harness**(팔레트 뒤에 busy한 가짜 배경 이미지를 까는 개발용 엔트리)로 glass CSS 튜닝을 보조한다 — 구현 plan에 태스크로 포함.
- **빠른 터치 반복**: Linux/Mac에서 Vite dev server 기동 → 터치스크린 PC의 Edge로 LAN 접속 → 실제 터치 반응을 실시간 확인하며 수정.
- **Windows 빌드**: GitHub Actions windows runner. 터치스크린 PC에는 산출물 다운로드만 (방송 장비에 개발환경 설치 안 함).
- **Windows 검증 대상 (Mac 티어 도입으로 축소)**: acrylic/mica 최종 재질 튜닝 + 터치 최종 확인. WebView2 렌더링은 Mac+Chrome 근사로 사전 커버되므로 잔여 차이만 점검.
- WebView2 런타임: Win11 기본 탑재, Win10 대부분 탑재 — 없으면 설치 포함 배포 옵션 사용.
- **데모 페이지 브라우저**: Chrome/Edge는 동일 Blink — Mac Chrome 확인이 Windows Edge kiosk와 사실상 동일(폰트 래스터라이징 미세 차이만).

## 11. 확장 여지 (v1 범위 밖, 스키마만 대비)

- 버튼 `type` 확장: 토글형(상태 동기화 필요 — UE 피드백 채널 전제), 버튼별 대상 override.
- 팔레트 접기(collapse).
- 부팅 자동 시작.
- 데모 페이지의 별도 repo 분리 (`demo/` + `shared/` 복사).
- 트리거별 ACK (방송 자동화 시스템 편입 시).

## 12. 미해결 항목

- **터치스크린 PC의 Windows 버전 확인** — glass 경로(acrylic vs mica) 선택에 영향. 설계는 양쪽 커버로 잡았으므로 구현 차단 요소는 아님.
- 데모 페이지의 구체적 콘텐츠(카드 장수·소재)는 구현 단계에서 결정.
