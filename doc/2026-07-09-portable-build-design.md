# XR-Touch_to_OSC — Portable 빌드 설계 스펙 (D15)

- **날짜**: 2026-07-09
- **상태**: 승인됨 (brainstorming 세션, 사용자 승인)
- **다음 단계**: writing-plans로 구현 계획 작성
- **선행 스펙**: `2026-07-02-xr-touch-trigger-design.md` (D1~D14)

## 1. 개요 / 동기

현재 v0.1.0은 Windows용 **설치판**(NSIS `.exe` + WiX `.msi`)만 배포한다. 여기에 **portable 버전**(무설치 단일 exe + 옆 폴더에 config)을 추가한다.

**동기**: 빠른 테스트와 타 장비 사용 편의. 폴더 하나(exe + config + 미래의 로그)를 통째로 복사하면 설정까지 그대로 따라가므로, 방송 예비기 세팅·다른 PC에서의 즉시 사용이 쉬워진다.

**설치판과의 관계**: 두 트랙은 별개다. 설치판은 in-place 업그레이드(또는 추후 updater), portable은 버전별 zip을 따로 받아 교체한다. updater와 portable은 궁합이 맞지 않으므로 섞지 않는다.

## 2. 결정 (D15)

```text
┌─────┬──────────────────────┬────────────────────────────────────────────────┐
│ #   │ 항목                 │ 확정 내용                                        │
├─────┼──────────────────────┼────────────────────────────────────────────────┤
│ D15 │ portable 빌드        │ 설치판과 "바이트 단위로 동일한 단일 바이너리"를  │
│     │ (2026-07-09)         │ 마커 파일로 portable/설치 모드만 분기.           │
│     │                      │ portable 모드에서는 config·(미래)로그를 exe와    │
│     │                      │ 같은 폴더에 flat하게 저장. 배포물은 exe +        │
│     │                      │ `portable.txt` 마커를 묶은 zip.                  │
└─────┴──────────────────────┴────────────────────────────────────────────────┘
```

**핵심 불변식**: portable exe와 설치판 exe는 **동일한 컴파일 산출물**이다. `tauri build`가 release exe를 한 번 만들고, 그 exe를 NSIS/MSI가 그대로 포장하며, portable zip도 같은 exe를 담는다. 따라서 기능·안정성·동작은 by construction으로 동일하다. 컴파일 타임 feature 분기(별도 바이너리)는 이 불변식을 깨므로 채택하지 않는다.

**사용자 노출 이름 통일(부수 결정, 2026-07-09)**: 현재 productName은 `xrt-widget`, Cargo package name은 `xrt-app`이며 `mainBinaryName`이 미설정이라 실제 exe는 `xrt-app.exe`(productName과도 불일치)다. portable에서는 사용자가 exe를 직접 더블클릭하므로 이름이 곧 UX다. 사용자에게 노출되는 이름을 **`xr-touch-widget`으로 통일**한다:

- `app/tauri.conf.json`의 `productName`을 `xr-touch-widget`으로 변경 → 설치판 표시명·설치 아티팩트 이름이 통일.
- `app/tauri.conf.json`에 `"mainBinaryName": "xr-touch-widget"` 명시 → exe가 `xr-touch-widget.exe`로 결정론적 고정.
- (D14로 범용 OSC 컨트롤러가 된 실체에는 'widget'이 'touch→OSC 변환기'보다 부합.)

**의도적으로 유지하는 것**: `identifier`(`kr.co.sbs.ncenter.xrt`)는 config 경로(`app_config_dir()` = `%APPDATA%\<identifier>\`)를 결정하므로 **그대로 둔다** — 바꾸면 기존 설정이 고아가 되고 표시 가치가 없다. 따라서 설치판 config 동작은 이름 변경과 무관하게 불변. 내부 crate 이름(`xrt-app`, `xrt-core`)도 사용자에게 노출되지 않으므로 유지한다(`mainBinaryName` 설정으로 exe 이름과 분리됨). GitHub repo 이름 통일(`xr-touch-widget`)은 관련 housekeeping이나 **이 spec의 구현 범위 밖**으로 별도 처리한다.

## 3. 아키텍처 — "데이터 base 폴더" 도입

현재 `app/src/main.rs`(120번째 줄 근처)는 config 경로를 `app.path().app_config_dir()` 하나로만 해석한다. 이를 **base 폴더 해석 → 그 아래 파일 배치**의 2단계로 바꾼다.

```text
                 ┌─ exe와 같은 폴더에 `portable.txt` 마커가 있는가? ─┐
                 │                                                  │
             예 ▼ (portable 모드)                            아니오 ▼ (설치 모드)
      base = exe가 위치한 폴더                          base = app_config_dir()
      (std::env::current_exe() 의 부모)                 (%APPDATA%\kr.co.sbs.ncenter.xrt\)
                 │                                                  │  ← 기존 동작, 변경 없음
                 ▼                                                  ▼
      config = base / "config.toml"                     config = base / "config.toml"
      (미래) log = base / "xrt.log"                     (미래) log = base / "xrt.log"
```

### 3.1 base 폴더 해석 규칙

1. `std::env::current_exe()` → 부모 디렉터리 `exe_dir`을 구한다.
2. `exe_dir / "portable.txt"` 존재 여부를 확인한다.
   - 존재 → **portable 모드**: `base = exe_dir`.
   - 미존재 → **설치 모드**: `base = app_config_dir()` (기존 경로. 해석 실패 시 기존 temp-dir fallback 유지).
3. `config_path = base / "config.toml"`.

### 3.2 왜 마커 파일인가

단일 바이너리가 자신이 어느 모드인지 알 **런타임 신호**가 필요하다(설치판의 APPDATA 동작을 절대 건드리지 않기 위함). 마커 파일이 그 신호다.

- **설치판**: NSIS/MSI가 exe만 설치하고 마커는 넣지 않는다 → 마커 부재 → 기존 APPDATA 동작 100% 보존. **안전**.
- **portable**: 배포 zip에 빈 `portable.txt`를 동봉 → 압축 해제 후 실행하면 마커 존재 → config가 exe 옆에 flat하게 생성.

마커 이름은 `portable.txt`로 확정한다(폴더를 열었을 때 사람이 보고 "portable 모드"임을 바로 인지 가능). 마커 파일의 **내용은 무시**한다(존재 여부만 판단).

## 4. 배포물(portable zip) 생성

1. `tauri build`가 `target/release/xr-touch-widget.exe`를 생성(설치판 빌드와 동일 단계. `mainBinaryName` 설정으로 이 이름이 보장됨 — §2 부수 결정).
2. 그 exe + 빈 `portable.txt`를 한 폴더에 모아 zip으로 압축 → `xr-touch-widget-portable_<version>_x64.zip`.
3. `.github/workflows/release.yml`의 Windows 잡에 위 zip을 만들어 해당 태그의 GitHub Release에 업로드하는 스텝을 추가한다(설치판 아티팩트와 나란히).

사용자 사용법: zip 다운로드 → 압축 해제 → 폴더 내 `xr-touch-widget.exe` 실행. 설정 변경 시 `config.toml`이 같은 폴더에 생성/갱신됨. 폴더째 복사하면 다른 PC로 설정까지 이동.

## 5. 범위 경계 (YAGNI)

**포함**:
- base 폴더 해석 로직 + 단위 테스트.
- `release.yml`에 portable zip 스텝 추가.

**제외(이번 세션 안 함, 단 확장 여지는 열어둠)**:
- **파일 로깅(BACKLOG-2)**: 구현하지 않는다. 단, base 폴더 추상화를 도입해두어 추후 `base / "xrt.log"` 한 줄로 얹을 수 있게 설계만 열어둔다.
- **WebView2 fixed-runtime 번들**(~100MB+): 하지 않는다. 타깃 방송 PC에는 WebView2가 이미 있다(Win11·최신 Win10 기본 탑재). portable exe는 시스템 WebView2에 의존한다.
- **read-only 매체 예외**(읽기전용 USB에서 직접 실행 시 exe 옆 쓰기 실패): 이번엔 다루지 않는다. 주 use case는 폴더를 쓰기 가능한 로컬 디스크에 복사 후 실행. 필요 시 추후 "쓰기 불가 → APPDATA fallback"을 얹는다.

## 6. 테스트 / 검증 경계 (현재 개발기 = Mac)

```text
┌──────────────────────────────────┬───────────────────────────────────────────┐
│ 대상                             │ 검증 위치                                   │
├──────────────────────────────────┼───────────────────────────────────────────┤
│ base 폴더 해석 로직 (크로스플랫폼)│ Mac에서 `cargo test`로 단위 검증:           │
│                                  │ 마커 有 → exe_dir 기반 / 無 → 설치 경로     │
│                                  │ 로 분기하는지. (경로 조립 함수로 추출해     │
│                                  │ 순수 함수 단위 테스트)                      │
├──────────────────────────────────┼───────────────────────────────────────────┤
│ 실제 Windows portable exe + zip  │ CI(windows-latest) 또는 Windows 머신.       │
│ + 실행 스모크                    │ 예정된 "Windows 실장비 검증" 세션에 겸함:   │
│                                  │ zip 해제 → 실행 → config가 옆에 생기는지 →  │
│                                  │ 폴더 복사 후 설정 유지되는지.               │
└──────────────────────────────────┴───────────────────────────────────────────┘
```

**주의**: portable 모드의 최종 사인오프(실제 Windows 실행)는 Mac에서 불가능하다. 이번 세션 산출물은 (1) base-dir 로직 + 단위 테스트, (2) release.yml zip 스텝이며, on-device 스모크는 Windows 검증 세션으로 넘긴다.

## 7. 영향받는 파일 (예상)

- `app/src/main.rs` — config 경로 해석부(120번째 줄 근처)를 base-dir 해석으로 교체. 설치 모드 경로·기존 temp fallback은 그대로 보존.
- (신규 또는 기존 모듈) base-dir 해석을 순수 함수로 추출 → 단위 테스트 대상화.
- `app/tauri.conf.json` — `productName`을 `xr-touch-widget`으로 변경 + `"mainBinaryName": "xr-touch-widget"` 추가(사용자 노출 이름 통일, §2 부수 결정).
- `.github/workflows/release.yml` — Windows 잡에 portable zip 생성·업로드 스텝 추가.

*(File Structure의 `[create]`/`[modify]` 최종 판정은 plan 단계에서 실제 repo 상태로 확정한다.)*
