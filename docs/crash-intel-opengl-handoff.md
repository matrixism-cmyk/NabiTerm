# nabiTerm 실행 크래시 진단 인수인계 (Intel HD 530 / OpenGL)

> 작성: 2026-06-27 · 대상 환경: 사용자 PC (Windows 11 Pro, Intel HD Graphics 530)
> 수신: nabiTerm 소스를 작성한 Claude Code(개발 에이전트)
> 상태: **근본 원인 확정.** 코드 버그 아님 → 렌더 백엔드 정책 결정 필요.

---

## TL;DR

- 증상: nabiTerm가 **간헐적으로 실행 직후/실행 중 크래시** ("실행 안 됨"으로 체감). 항상은 아님 — 정상 실행되는 세션도 있음(진단 중 PID 11584가 16시간 이상 생존 확인).
- 원인: eframe **기본 glow(OpenGL) 렌더러** → 이 PC의 **Intel HD 530 OpenGL 드라이버(`ig9icd64.dll`)** 내부에서 `ACCESS_VIOLATION(0xC0000005)`.
- HD 530은 **단종(Skylake/6세대)**, 설치된 `30.0.101.1339`(2022-01-21)가 **사실상 마지막 드라이버** → **드라이버 업데이트로 해결 불가.**
- nabiTerm 소스 결함 아님. **렌더 경로를 OpenGL ICD에서 떼어내는 것**이 해결의 핵심.

---

## 수집한 증거

### 1) 크래시 덤프 2건 (WER LocalDumps)
경로: `%LOCALAPPDATA%\CrashDumps\`

| 덤프 | 시각 | 예외 코드 | 폴트 위치 |
|---|---|---|---|
| `nabiTerm.exe.17004.dmp` | 2026-06-27 10:08 | `0xC0000005` ACCESS_VIOLATION | **`ig9icd64.dll`** 내부 (base+0xAF00E0) |
| `nabiTerm.exe.6824.dmp` | 2026-06-20 12:21 | `0xC0000005` ACCESS_VIOLATION | addr `0x0` (널 역참조, GPU/렌더 스레드) |

> `ig9icd64.dll` = Intel Graphics **OpenGL ICD**(Installable Client Driver).
> 폴트 주소가 이 모듈의 코드 영역 내부 → **드라이버 자체가 OpenGL 호출 처리 중 죽음.**

미니덤프 파싱은 PowerShell로 직접 수행(cdb 미설치). 예외 스트림(type 6)의 ExceptionCode와
ExceptionAddress를 추출 → ModuleList(type 4)에서 해당 주소를 포함하는 모듈을 역매핑.

### 2) GPU / 드라이버
```
Name          : Intel(R) HD Graphics 530
DriverVersion : 30.0.101.1339
DriverDate    : 2022-01-21
Status        : OK   (HW 고장 아님)
```
OpenGL 런타임 로그(정상 실행 세션에서 캡처):
```
opengl version : 3.3.0 - Build 30.0.101.1339
opengl renderer: Intel(R) HD Graphics 530
opengl vendor  : Intel
Shader version : Gl140 ("3.30")
eframe         : Using the glow renderer
```

### 3) 정상 실행 로그 (참고 — 초기화 자체는 통과)
glow Display 생성 → GL 컨텍스트(WGL) → keyring(Windows 자격증명) → control 서버
(`\\.\pipe\nabi-control-<pid>`) → 오케스트레이터 → alacritty_terminal 까지 정상 진입.
즉 **앱 로직은 멀쩡**하고, 죽는 지점은 OpenGL 드라이버 콜 중간.

---

## 근본 원인

eframe가 **glow(OpenGL) 백엔드**로 구동되며(`nabi-app` Cargo feature `"glow"`,
`NativeOptions`의 기본 `Renderer::Glow`), 실제 GL 명령이 **Intel HD 530의 노후 OpenGL ICD**로
들어가 그 안에서 메모리 접근 위반으로 크래시한다. ICD는 단종 칩셋의 마지막 빌드라 패치가 없다.

- 외부(드라이버) 모듈에서 발생하는 SEH 예외라 **Rust panic hook / `catch_unwind`로 잡히지 않음** →
  `nabi_log::install_crash_handler`(main.rs:250)로도 우아한 복구 불가, 프로세스 즉사.
- 간헐성: GL 상태/타이밍/리사이즈 등에 따라 ICD가 죽는 코드 경로를 밟을 때만 발생.

### 관련 소스 위치
- `crates/nabi-app/src/main.rs:260` — `NativeOptions { ... }` (renderer 미지정 → glow 기본).
- `crates/nabi-app/Cargo.toml` — `eframe = { ... features = ["accesskit","default_fonts","glow","persistence"] }`
- 워크스페이스 `Cargo.toml` 주석: `# M1은 glow(OpenGL) 백엔드로 단순화. wgpu 커스텀 렌더러는 후속.`
- glow 전용 페인트 콜백(`egui_glow` / `PaintCallback` / `glow::`) **사용처 없음** → 백엔드 교체 시
  커스텀 렌더 코드 깨질 위험 없음(즉시모드 egui만 사용).

---

## 해결안 (택1 또는 조합)

### A. Mesa3D `opengl32.dll` 드롭인 — 무재빌드·즉시·가역  ★빠른 완화
설치 폴더(`%LOCALAPPDATA%\Programs\nabiTerm\`)에 Mesa3D의 `opengl32.dll`을 동봉하면
glow가 Intel ICD 대신 **Mesa**를 로드 → 문제의 `ig9icd64.dll` 경로를 완전히 우회.
- `GALLIUM_DRIVER=llvmpipe` : 순수 소프트웨어 렌더. **가장 안정**(드라이버 크래시 원천 차단).
  터미널 에뮬레이터라 소프트웨어 렌더로도 체감 성능 충분.
- `GALLIUM_DRIVER=d3d12` : GL→DirectX12 변환. GPU 가속 유지하면서 OpenGL ICD는 회피.
- 장점: 소스 변경 0, 롤백은 DLL 제거뿐, 지금 설치본에서 즉시 검증 가능.
- 작업: 인스톨러(`installer/nabiTerm.iss`)·portable zip 패키징에 DLL + 환경변수 기본값 포함.
- 주의: Mesa 빌드(mesa-dist-win 등) 동봉 라이선스 표기(MIT) 추가.

### B. eframe **wgpu(DX12)** 렌더러로 전환 — 정공법·재빌드 필요
- `Cargo.toml`: eframe feature `"glow"` → `"wgpu"`.
- `main.rs`: `NativeOptions { renderer: eframe::Renderer::Wgpu, ..}`.
- OpenGL 경로 자체를 제거(Windows에서 wgpu 기본 DX12 백엔드).
- 트레이드오프: 바이너리/의존성 증가, **이 노후 드라이버의 DX12도 100% 안전 보장은 아님**,
  계획서상 GPU 렌더러 변경은 "별도 합의 후"(B4/P4) 항목.
- 게이트 통과 필요: gnu/MinGW 빌드 + clippy 0 + xtask lines 0 + test → dist → 릴리스(+0.0.1).

### C. (병행 권장) 런타임 폴백 옵션 노출
- 환경변수/설정으로 렌더러 강제 선택(glow / wgpu / mesa-software)을 사용자에게 노출.
- glow 유지 시: `hardware_acceleration` 폴백, 멀티샘플 0 유지 등 — 단, **드라이버 내부 크래시라
  옵션만으로는 근절 어려움**(완화 한계 명시).
- `install_crash_handler`에 "직전 GL 크래시 감지 시 다음 실행은 자동으로 소프트웨어 렌더" 같은
  자가 회복(안전 모드) 추가 검토.

### D. 불가 — 드라이버 업데이트
HD 530 단종, `30.0.101.1339`(2022)가 최종. 신규 드라이버 없음. **선택지 아님.**

---

## 권장 경로

1. **즉시:** A(Mesa `opengl32.dll`, 우선 `llvmpipe`)로 사용자 PC 안정화 — 무재빌드·가역.
2. **본 수정:** B(wgpu/DX12) 또는 C의 안전모드 폴백을 다음 배포(+0.0.1)에 반영하되,
   이 머신에서 wgpu 안정성 실측 후 기본값 결정. 불확실하면 A를 기본 폴백으로 유지.
3. 어느 쪽이든 **사용자 선택 가능한 렌더러 토글**을 노출해 노후 GPU 사용자 전반을 커버.

---

## 검증 방법

- 재현이 간헐적이라 1회 정상 실행으로 "고쳐짐" 단정 금지. 다음을 기준화:
  - WER 덤프 모니터: `%LOCALAPPDATA%\CrashDumps\nabiTerm.exe.*.dmp` 신규 생성 여부.
  - 다회 콜드 스타트 + 리사이즈/탭 다수 생성/스크롤백 부하 시나리오 반복.
  - 적용 후 며칠간 덤프 미발생 = 해결 신호.
- 덤프 분석 재현(디버거 없이): 미니덤프 헤더(`MDMP`) → StreamDirectory에서
  ExceptionStream(6)/ModuleList(4) 파싱 → 예외주소를 모듈 범위에 역매핑.
  (cdb/WinDbg 설치 시 `!analyze -v`로 동일 결론 확인 가능.)

---

## 부록 — 현장 상태 메모

- Rust 툴체인: `stable-x86_64-pc-windows-gnu` (cargo/rustc 1.96.0) — `~/.cargo/bin`. 재빌드 가능.
- 빌드 산출물: `Desktop\nabi\dist\` (setup/portable/standalone), 설치본 exe 24.7MB(2026-06-27 01:39 빌드).
- 진단 중 실행 인스턴스 PID 11584 생존 → 앱이 "항상" 죽는 게 아님을 입증.
