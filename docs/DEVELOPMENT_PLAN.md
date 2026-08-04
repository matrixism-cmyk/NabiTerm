# nabi — 개발 계획서 (마스터)

> 네이티브 Windows용 Rust 터미널 에뮬레이터 + MobaXterm급 SSH 클라이언트.
> 중앙 오케스트레이터형 멀티플렉서, GPU 렌더링(egui/wgpu), 멀티 OS 윈도우, 자격증명 볼트.
>
> 본 문서는 마스터 인덱스입니다. 상세는 아래 동반 문서를 참조하세요.
> - [architecture.md](./architecture.md) — 워크스페이스/크레이트/모듈 맵, 데이터 흐름, 라인 제한 강제, 빌드 툴링
> - [ui-windowing-rendering.md](./ui-windowing-rendering.md) — 멀티 뷰포트 윈도우 관리, 탭 분리(tear-out), 도킹, 메뉴바, GPU 렌더러
> - [ssh-security.md](./ssh-security.md) — SSH 스택, 인증, known_hosts/TOFU, 포워딩, SFTP, 자격증명 볼트, 보안 정책
> - [menus-features.md](./menus-features.md) — 전체 풀다운 메뉴 트리 + 기능 매트릭스(P0–P3 / M1–M4)
> - [i18n-cjk.md](./i18n-cjk.md) — 다국어(ko/en/ja) + CJK 입력(IME)/렌더링/인코딩
> - [file-transfer.md](./file-transfer.md) — SFTP/FTP 브라우저 + 외부 편집기 편집 + cwd 추적
> - [sessions-storage.md](./sessions-storage.md) — 세션 내보내기/가져오기 + 저장 위치 지정(포터블)

작성일 2026-06-05. 본 계획은 멀티 에이전트 리서치 + 적대적 검증을 거쳐 작성되었으며, 검증으로 정정된 사항은 각 문서에 ✅(확인)/⚠️(정정/주의)로 표기했습니다.

---

## 1. 비전 & 범위

**한 문장:** "여러 로컬/원격 세션을 한 화면(또는 분리된 여러 OS 창)에서 띄우고, 중앙 오케스트레이터가 세션 간 명령을 분배하고 출력을 수집·판정하는, 안전하고 빠른 네이티브 Windows SSH 클라이언트."

**포함(scope in):**
- 로컬 셸(ConPTY: PowerShell 7/Windows PowerShell/cmd/WSL/Git Bash)과 SSH 원격 세션을 동일한 터미널 모델로 처리
- 한 윈도우 내 다중 탭 + 분할 패널(MDI), 그리고 탭/패널을 **별도 OS 윈도우로 분리**(멀티 뷰포트)
- 풀다운 메뉴바(설정 포함, 일반 SSH 클라이언트가 갖는 전 메뉴)
- SFTP 브라우저, 포트 포워딩/터널, MultiExec(브로드캐스트), 세션 관리자, 자격증명 볼트
- 중앙 오케스트레이터: 명령 디스패치(1:1/1:N), 출력 탭/수집, OSC 133/633 명령 경계 검출

**제외/후순위(scope out, P3+):** RDP/VNC, Mosh 네이티브 SSP, 클라우드 동기화 볼트, 인-프로세스 X 서버. (메뉴 자리는 두되 마일스톤에서 분리 — [menus-features.md](./menus-features.md) 참조)

---

## 2. 핵심 설계 원칙

1. **파일 라인 제한 — 소프트 250 / 하드 400 (절대 준수).** 모든 소스 파일은 단일 책임으로 쪼개고, 250줄을 설계 목표·400줄을 절대 상한으로 둔다. CI에서 `cargo xtask lines`로 강제(하드 위반 시 빌드 실패). → [architecture.md §라인 제한 강제](./architecture.md)
2. **모듈화 우선.** ~22개의 작은 라이브러리 크레이트로 구성된 Cargo 워크스페이스. `lib.rs`는 얇은 facade(`pub mod`/`pub use`)만. 외부 의존성(특히 russh, egui-wgpu)은 각각 하나의 크레이트 뒤에 격리해 잦은 breaking change를 차단.
3. **오케스트레이터 단일 소유권.** 모든 Pane 상태(바이트 채널 + VT 모델 + 스크롤백)는 오케스트레이터가 `PaneId`로 소유. **UI는 절대 터미널 상태를 소유하지 않고 `PaneId`만 보유** → 탭을 다른 창으로 옮기는 것이 `PaneId` 이동에 불과해 desync/누수 없이 안전.
4. **공유 가변 상태 대신 채널.** UI↔오케스트레이터는 crossbeam-channel(동기 UI/PTY) ↔ tokio(비동기 net/SSH)를 **단 하나의 명시적 seam**(`nabi-orchestrator::bridge`)에서 연결. Arc/Mutex는 read-mostly 데이터(config/theme 스냅샷)에만.
5. **로컬·원격 동일 모델.** ConPTY와 SSH 채널 모두 공통 `ByteChannel` 트레잇을 구현 → VT 모델은 로컬/원격이 동일.
6. **안전 기본값(secure by default).** 포트/에이전트/동적 포워딩 OFF, strict host-key checking ON, 페이스트 보호 ON, 비밀은 zeroize/secrecy로 메모리 와이프, 로그에서 비밀 제거.

---

## 3. 검증된 기술 스택

버전은 2026-06 기준. 모든 외부 크레이트는 루트 `[workspace.dependencies]`에서 정확히 핀 고정.

| 영역 | 선택 | 버전 | 라이선스 | 비고 |
|------|------|------|----------|------|
| GUI 프레임워크 | egui / eframe / egui-wgpu | 0.34.x | MIT/Apache | 멀티 뷰포트(0.24+) ✅, wgpu 버전의 단일 진실원 |
| 도킹/탭 | egui_dock | 0.19.1 | MIT | 인-윈도우 탭/분할/도킹. ⚠️ tear-off는 in-app 플로팅 창만 — 네이티브 OS 창 분리는 자작 |
| OS PTY | portable-pty | 0.9.x | MIT | ConPTY. take_writer/try_clone_reader/resize. Win10 1809+ |
| 터미널 모델 | alacritty_terminal | 최신 | Apache-2.0 | Term/Grid + **damage tracking**(재그리기 게이트). vt100은 더 가벼운 대안 |
| SSH | **russh** | **0.61.2 (정확히 핀)** | Apache-2.0 | 순수 Rust 비동기. ⚠️ minor마다 breaking — `nabi-ssh` 뒤에 격리. 크립토 백엔드 1개 필수 |
| SSH keys | russh::keys (내장) | (russh 동봉) | Apache-2.0 | ⚠️ 구 `russh-keys` 크레이트 아님 — 0.49부터 `russh::keys`로 병합. known_hosts 헬퍼 내장 |
| SFTP | russh-sftp | 2.3.0 | Apache-2.0 | SFTP v3 클라이언트 |
| ssh_config | russh-config | 0.58.x | Apache-2.0 | ~/.ssh/config 파싱 (HostName/User/Port/ProxyJump) |
| 크립토 백엔드 | ring (기본) / aws-lc-rs (FIPS·PQ) | — | — | russh는 둘 중 **정확히 하나** 필수. CI는 ring(깨끗한 Windows 빌드), aws-lc-rs는 feature gate |
| 자격증명 저장 | keyring | 4.0.1 | MIT/Apache | Windows Credential Manager(DPAPI 백엔드). 접근 직렬화 필요 |
| 볼트 암호 | argon2 + aes-gcm | 최신 | MIT/Apache | Argon2id KDF + AES-256-GCM |
| 비밀 와이프 | zeroize / secrecy | 1.8 / 0.10 | MIT/Apache | drop 시 메모리 와이프, Debug 마스킹 |
| 텍스트 셰이핑 | rustybuzz + swash | 최신 | MIT/Apache | 글리프 셰이핑·래스터화 |
| 글리프 아틀라스 | etagere | 최신 | MIT/Apache | 아틀라스 패킹 (glyphon 내부와 동일 접근) |
| 설정 | figment + notify + toml/serde | 최신 | MIT/Apache | 계층 설정 + 핫리로드 |
| 동시성 | tokio + crossbeam-channel | 최신 | MIT/Apache | 비동기 net / 동기 UI seam |
| 에러/로그 | thiserror + anyhow + tracing | 최신 | MIT/Apache | lib=thiserror, app=anyhow, 워크스페이스 tracing |
| 라인 게이트 | tokei (+ 자작 xtask) | 최신 | MIT/Apache | `--files --output json` → xtask가 250/400 임계 강제 |

> ⚠️ **wgpu 버전 동기화(빌드 실패 방지):** egui-wgpu·wgpu·(사용 시)glyphon은 **반드시 동일 major wgpu**로 컴파일되어야 함. egui-wgpu가 정하는 wgpu 버전을 단일 진실원으로 삼고 워크스페이스 전역 핀. 자세히는 [ui-windowing-rendering.md](./ui-windowing-rendering.md).

### 3.1 확장 기능 스택 (다국어 · 파일 전송 · 세션)

| 영역 | 선택 | 버전 | 라이선스 | 비고 |
|------|------|------|----------|------|
| UI 다국어 | fluent-templates (또는 i18n-embed) | 0.14 / 0.16 | MIT/Apache | Fluent 기반(CJK plural). ArcLoader 런타임 ko/en/ja 전환 |
| CJK 인코딩 | encoding_rs | 0.8.35 | (Apache/MIT)+BSD-3 | Shift_JIS/EUC-KR(CP949)/GBK/GB18030/Big5 스트림 디코딩 |
| CJK 폴백 | fontdb (+ unicode-width) | 최신 | MIT/Apache | ⚠️ swash/rustybuzz 단독 폴백 없음 → 직접 구현 또는 cosmic-text FontSystem |
| FTP/FTPS | suppaftp | 8.0.3 | MIT/Apache | tokio + rustls-ring(OpenSSL 불필요). 유일한 현대 Rust FTP 클라 |
| 파일 감시 | notify + notify-debouncer-full | 8.2 / 0.5 | CC0 / MIT-Apache | 외부 편집기 저장 감시(부모 디렉터리) |
| 편집기 실행 | which + open + tempfile + blake3 | — | MIT/Apache | ⚠️ 설정 편집기는 std::process::Command(opener 부적합) |
| 기본 경로 | directories | 6.0.0 | MIT/Apache | ProjectDirs(Known Folder API) |
| 세션 INI | rust-ini | 0.21.3 | MIT | PuTTY/MobaXterm 임포트 |

→ 상세 [i18n-cjk.md](./i18n-cjk.md) · [file-transfer.md](./file-transfer.md) · [sessions-storage.md](./sessions-storage.md). 신설 크레이트 nabi-i18n / nabi-fs / nabi-ftp / nabi-editor / nabi-session ([architecture.md §3b](./architecture.md)).

---

## 4. 아키텍처 개요

```
┌─ nabi-app (bin) ── eframe 진입점, tokio 런타임, tracing, 와이어링만
├─ UI 계층 ── nabi-ui-window(멀티 뷰포트 WindowManager) · nabi-ui-tab · nabi-ui-menu · nabi-ui-panels
├─ 렌더 ──── nabi-render (egui_wgpu CallbackTrait, instanced 글리프 아틀라스)
├─ 제어 ──── nabi-orchestrator (Pane 단일 소유, 라우터/액터, crossbeam↔tokio seam)
├─ 도메인 ── nabi-vt(터미널 모델) · nabi-osc(명령 경계) · nabi-pty(ConPTY) · nabi-ssh(+ssh-ext/sftp)
├─ 인프라 ── nabi-config · nabi-secret · nabi-log · nabi-error · nabi-plugin-api
└─ 기반 ──── nabi-types · nabi-proto   (의존성 없는 공용 어휘/메시지)
+ xtask (라인 게이트·CI 자동화)
```

핵심 불변식: **바이트 → (nabi-osc 명령경계 탭) → nabi-vt(damage 기록) → 해당 PaneId를 호스팅한 뷰포트만 `request_repaint_of`.** 유휴 패널은 damage 없음 → 재그리기 없음 → GPU 0. 전체 크레이트 표·모듈 분해·데이터 흐름은 [architecture.md](./architecture.md).

---

## 5. 마일스톤 로드맵 (요약)

| 마일스톤 | 목표 | 대표 기능(P0/P1) |
|----------|------|------------------|
| **M1 — MVP** | "탭 하나에서 로컬·SSH가 뜨고 입력/리사이즈/렌더가 된다" | ConPTY 로컬 셸, russh SSH(password/pubkey/kbd-interactive/agent), request_pty+window_change, **strict known_hosts + 지문 TOFU 다이얼로그**, 세션 관리자 트리, egui_dock 탭+분할, GPU 렌더러, 메뉴바+커맨드 팔레트, 스크롤백 검색, ~/.ssh/config 임포트, Credential Manager 저장 |
| **M2** | "SSH 클라이언트다워진다" | SFTP 브라우저, 로컬(-L)/원격(-R) 포워딩 + 터널 매니저 UI, 키 생성/관리(PPK 임포트), **자격증명 볼트(Argon2id+AES-GCM)**, **네이티브 탭 분리(멀티 뷰포트)**, 설정 다이얼로그(PuTTY 깊이), 세션 임포트/익스포트, OSC 133/633 명령 캡처 |
| **M3** | "MobaXterm 패리티" | MultiExec(브로드캐스트), 매크로/스니펫/로그인 스크립트, **ProxyJump 체이닝**, 동적/SOCKS(-D), X11 포워딩(VcXsrv), Serial/Telnet, 인라인 이미지(Sixel/iTerm2/Kitty), 테마 핫리로드 |
| **M4** | "차별화·하드닝" | PQ KEX 선택 UI, 에이전트 포워딩 제어, 클라우드 동기 볼트, 플러그인(WASM), RDP/VNC, Zmodem, 워크스페이스 저장/복원 |

**추가 요구사항의 마일스톤 배치:**
- **M1:** UI 다국어(ko/en/ja) 골격 + CJK IME 입력 + 이중폭/폰트 폴백 렌더 + 인코딩 탭(P0 — 한국어 사용자 기본 경험) · 세션 저장 위치 지정/포터블 모드(P0)
- **M2:** SFTP 브라우저(기존) + **외부 편집기 편집**(우클릭 Edit→다운로드/감시/재업로드) + 세션 내보내기/가져오기(OpenSSH/PuTTY/MobaXterm) + cwd 추적(OSC 7)
- **M3:** **FTP/FTPS 세션 타입**(suppaftp, 브라우저 백엔드 공유) + 레거시 CJK 인코딩 세션 오버라이드 UI

전체 P0–P3 매트릭스는 [menus-features.md](./menus-features.md).

---

## 6. 리스크 레지스터 (검증 기반)

| 리스크 | 영향 | 완화 |
|--------|------|------|
| russh가 minor마다 breaking change | 빌드 깨짐/유지보수 | `nabi-ssh` 한 크레이트 뒤에 전부 격리, 정확한 버전 핀, 변경 시 한 곳만 수정 |
| russh에 ProxyJump/SOCKS turn-key 없음 (#183 open) | M3 패리티 지연 | `nabi-ssh-ext`에서 direct-tcpip 기반 자작, 모드별 작은 모듈로 분리 |
| ⚠️ egui_dock tear-off = in-app 플로팅(네이티브 창 아님) | 핵심 UX 미충족 | 멀티 뷰포트(`show_viewport_deferred`) 위에 **자작 WindowManager**, PaneId만 이동 |
| ⚠️ 멀티 뷰포트 deferred 창은 출력 시 명시적 repaint 안 하면 멈춤 | 원격 출력이 안 보임 | 새 출력 시 해당 viewport에 `request_repaint_of` |
| ⚠️ wgpu 버전 불일치 → 하드 컴파일 실패 | 렌더 통합 불가 | egui-wgpu 기준 단일 핀, glyphon 직접 사용 시 trio 동시 호환 확인 |
| ⚠️ OSC 133은 best-effort(원격 셸 설정 의존, C 마커 자주 누락) | 명령 캡처 부정확 | 마커 부분집합 허용 + 센티넬 폴백(`nabi-osc::sentinel`), 캡처를 untrusted로 파싱 |
| ConPTY 동기 파이프·데드락·SGR 누락 | 멈춤/표시 오류 | 입출력 별도 스레드, 종료 시 마지막 프레임 drain, UTF-8 직접 디코드(chcp 금지) |
| 크립토 백엔드 빌드 복잡성(aws-lc-rs: C/CMake/NASM) | Windows CI 실패 | 기본 ring, aws-lc-rs는 feature gate(FIPS/PQ 필요 시) |
| ⚠️ PPK 암호화 키 폭넓은 호환 미검증 | PuTTY 이주 사용자 인증 실패 | 실제 PuTTY 내보내기 키(암호화 포함)로 검증 후 광고 |
| zeroize 한계(과거 swap/사이드채널 불가) | 비밀 잔존 | 디스크 스왑 최소화·자동 잠금, 사이드채널은 별도 위협으로 명시 |
| ⚠️ egui Windows IME 회귀 이력(#3532 등) | 한/일 입력 불가 | 핀 고정 egui 0.34에서 MS 한/일 IME 실측, 출시 전 검증 버퍼 |
| ⚠️ rustybuzz+swash 단독은 CJK 폰트 폴백 없음 | 한글/일본어 미표시 | fontdb 코드포인트별 폴백 직접 구현(또는 cosmic-text FontSystem 채택), 트레잇 경계로 교체 가능 |
| ⚠️ 외부 편집기 atomic-save/포크형 편집기 | 저장 누락/완료 미감지 | 부모 디렉터리 감시+디바운스, VS Code `code --wait`, blocks-vs-forks 설정 |
| ⚠️ 포터블/동기화 볼트에 DPAPI 사용 시 타 PC 복호 불가 | 자격증명 영구 손실 | 포터블 모드는 master password Argon2id+AES-GCM 강제, OS 바인딩 비밀 제외 |
| ⚠️ 세션 내보내기에 비밀 누출 | 기밀 유출 | export는 vault_key 핸들만, 직렬화 바이트에 비밀 없음을 테스트로 단언 |
| ⚠️ OSC 7 미설정 서버는 cwd 추적 불가 | follow-folder 무동작 | 셸 통합 스니펫 주입(opt-in), 미지원 시 graceful |

---

## 7. 다음 단계

1. **M1-0 스캐폴딩:** 워크스페이스 골격 생성 — 루트 `Cargo.toml`(`[workspace]` + `[workspace.dependencies]` 핀), `nabi-types`/`nabi-proto`/`nabi-error`/`nabi-log`, `xtask`(라인 게이트), `nabi-app`(빈 eframe 창), CI(fmt/clippy/`xtask lines`/test).
2. **M1-1 수직 슬라이스:** ConPTY 로컬 셸 1개 → `ByteChannel` → `nabi-vt` → `nabi-render`로 한 탭에 표시 + 입력/리사이즈.
3. **M1-2:** russh SSH 패널을 같은 `ByteChannel`로 연결, known_hosts/TOFU 다이얼로그.
4. 이후 [menus-features.md](./menus-features.md) 마일스톤 순서.

> 스캐폴딩을 시작하려면: 이 계획 승인 후 "M1-0 스캐폴딩 시작"이라고 알려주시면 워크스페이스를 실제 생성합니다. (모든 파일 ≤250/400줄 준수)

---

## 부록 A. 주요 참조 (OSS / 스펙 / 논문성 자료)

- russh — https://github.com/Eugeny/russh (warp-tech) · russh-sftp — https://github.com/AspectUnk/russh-sftp
- egui 멀티 뷰포트 — https://docs.rs/egui/latest/egui/viewport/ · egui_dock — https://github.com/Adanos020/egui_dock
- egui-wgpu CallbackTrait 예제(custom3d_wgpu) — https://github.com/emilk/egui
- glyphon(cosmic-text+etagere+wgpu) — https://github.com/grovesNL/glyphon · cosmic-term(레퍼런스 GPU 터미널) — pop-os/cosmic-term
- alacritty_terminal — https://docs.rs/alacritty_terminal · portable-pty — https://docs.rs/portable-pty
- 네이티브 Windows Rust tmux 레퍼런스 — psmux(https://github.com/psmux/psmux), wtmux(https://github.com/fukuyori/wtmux)
- OSC 133 셸 통합 — https://learn.microsoft.com/windows/terminal/tutorials/shell-integration · https://contour-terminal.org/vt-extensions/osc-133-shell-integration/
- ConPTY — https://learn.microsoft.com/windows/console/creating-a-pseudoconsole-session · https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/
- 텍스트 렌더링(grayscale+linear) — Kitty PR #5969, Ghostty discussion #8660
- keyring/DPAPI — https://docs.rs/keyring · windows-dpapi · zeroize — https://docs.rs/zeroize
- cargo xtask 패턴 — https://github.com/matklad/cargo-xtask · tokei — https://github.com/XAMPPRocky/tokei
