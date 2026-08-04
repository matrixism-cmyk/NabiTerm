# 풀다운 메뉴 트리 & 기능 매트릭스

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md)

MobaXterm/PuTTY/WezTerm/Tabby/Termius를 교차 조사해 도출한 **전체 메뉴바**와 **우선순위·마일스톤 매핑**. 모든 메뉴 아이템은 상태를 직접 바꾸지 않고 `nabi-proto::Command`를 방출(메뉴=커맨드 팔레트 패리티). 단축키는 Windows 관례 + 터미널 충돌 회피(예: Copy=`Ctrl+Shift+C`로 SIGINT용 `Ctrl+C` 보존).

---

## 1. 메뉴바 트리

### File
| 아이템 | 단축키 | 비고 |
|--------|--------|------|
| New SSH Session… | Ctrl+N | 세션 에디터(SSH/russh) |
| New Local Terminal | Ctrl+Shift+T | 기본 셸 ConPTY 새 탭 |
| New Local Terminal (Choose Shell) | Ctrl+Alt+T | 서브메뉴: PowerShell 7 / Windows PowerShell / cmd.exe / WSL 배포판 / Git Bash(자동 감지) |
| Quick Connect… | Ctrl+Shift+Q | user@host:port 한 줄 연결, ~/.ssh/config 반영 |
| New Session Type | — | 서브메뉴: SSH/SFTP/Telnet/Serial/Mosh/RDP/VNC (미도달 마일스톤은 비활성) |
| New Window | Ctrl+Shift+N | 새 네이티브 OS 창(deferred viewport) |
| Duplicate Session | Ctrl+Shift+D | 동일 호스트/자격증명으로 재연결 |
| Reconnect | Ctrl+Shift+R | 활성 세션 transport 재수립(볼트 자격증명 재사용) |
| Save Session | Ctrl+S | 세션 매니저 트리에 저장 |
| Import Sessions… | — | 서브메뉴: ~/.ssh/config / PuTTY 레지스트리 / MobaXterm(.mxtsessions) / JSON·TOML |
| Export Sessions… | — | 휴대용 JSON/TOML(기본 비밀 제외) |
| Open known_hosts File | — | known_hosts 매니저 |
| Save Terminal Output As… | Ctrl+Shift+S | 스크롤백을 text/HTML/ANSI로 |
| Print… | Ctrl+P | 가시 버퍼/선택 인쇄 |
| Close Tab | Ctrl+W | 라이브 세션 종료 시 확인 |
| Close Window | Ctrl+Shift+W | 창 패널 fold/파괴 |
| Exit | Alt+F4 | 라이브 세션 확인 + 종료 시 비밀 zeroize |

### Edit
Copy `Ctrl+Shift+C` · Paste `Ctrl+Shift+V`(bracketed-paste, 멀티라인 경고) · Paste Selection `Shift+Insert` · Copy as HTML/RTF · Select All `Ctrl+Shift+A` · Clear Scrollback `Ctrl+Shift+K` · Find… `Ctrl+Shift+F` · Find Next `F3` · Find Previous `Shift+F3` · Copy Mode(키보드 선택) `Ctrl+Shift+X` · Quick Select(힌트) `Ctrl+Shift+Space` · Preferences… `Ctrl+,`

### View
Command Palette… `Ctrl+Shift+P` · Toggle Session Manager Sidebar `Ctrl+B` · Toggle SFTP Browser Panel `Ctrl+Shift+B` · Toggle Tunnel Manager Panel · Toggle Log Pane · Toggle Full Screen `F11` · Toggle Menu Bar `Ctrl+Shift+M` · Zoom In `Ctrl+=` · Zoom Out `Ctrl+-` · Reset Zoom `Ctrl+0` · Color Scheme(서브메뉴) · Appearance Mode(Light/Dark/System)

### Sessions
Session Manager… `Ctrl+Shift+E` · Edit Session… `F2` · Connect to Saved(서브메뉴) · Recent Sessions(서브메뉴) · New Session Folder · Identities & Keys… · Sync Sessions (Cloud Vault)… *(P3)*

### Terminal
Send Special Key(서브메뉴) · Send Break (Serial) · Reset Terminal `Ctrl+Shift+Backspace` · Clear & Reset · Resize (Rows×Cols)… · Log Session to File… · Character Encoding(서브메뉴) · Toggle Local Echo · Paste Protection · Inline Images · Shell Integration Marks · Jump to Previous Prompt `Ctrl+Shift+Up` · Jump to Next Prompt `Ctrl+Shift+Down`

### Tabs
Next Tab `Ctrl+Tab` · Previous Tab `Ctrl+Shift+Tab` · Go to Tab 1–9 `Ctrl+1..9` · Move Tab Left `Ctrl+Shift+PageUp` · Move Tab Right `Ctrl+Shift+PageDown` · **Tear Tab into New Window `Ctrl+Shift+O`** · Rename Tab… · Set Tab Color · Split Pane Right (Vertical) `Ctrl+Shift+\` · Split Pane Down (Horizontal) `Ctrl+Shift+-` · Focus Pane (Direction) `Alt+화살표` · Close Pane · Zoom/Maximize Pane `Ctrl+Shift+Z` · **Broadcast Input to Panes**(synchronize-panes)

### Tools
SFTP File Browser `Ctrl+Shift+S` · Port Forwarding / Tunnel Manager… `Ctrl+Shift+L` · **MultiExec (Broadcast Command)…** · SSH Key Manager… · Generate Key Pair… · ssh-agent… · known_hosts Manager… · Credential Vault… · Macros & Snippets… · Run Login Script… · X Server (Companion)… · Send File via Zmodem

### Connection (활성 세션)
Connection Info… · Host Key Fingerprint… · Keepalive Settings… · Jump Host / ProxyJump… · Proxy Settings… · Agent Forwarding(토글) · X11 Forwarding(토글) · Port Forwards (This Session)… · Algorithm Preferences…

### Settings (Preferences `Ctrl+,` — PuTTY식 카테고리)
General/Startup · Appearance & Fonts · Themes & Color Schemes · Terminal Behavior · Keyboard & Shortcuts · Window & Tabs · **SSH › Connection / Kex / Host Keys / Ciphers & MACs / Authentication / Tunnels / Proxy·Jump / X11** · Security & Vault · SSH Config Import · Plugins & Extensions · Logging & Diagnostics · Advanced/Experimental

### Window
Minimize `Ctrl+M` · Maximize/Restore · Merge All Windows · Move Tab to Window · Always on Top · Switch Window · Save Layout/Workspace… · Restore Layout/Workspace…

### Help
Documentation `F1` · Keyboard Shortcuts Reference `Ctrl+Shift+/` · Command Palette Reference · Release Notes · Check for Updates… · View Logs/Diagnostics · Report an Issue…(비밀 레닥션 보장) · Open Source Licenses · About

> 메뉴 구현: 최상위 메뉴당 1모듈(`nabi-ui-menu/menu_*.rs`), 전역 단축키는 `shortcuts.rs`에서 `KeyboardShortcut`+`consume_shortcut`로 자작([ui-windowing-rendering.md §4](./ui-windowing-rendering.md)).

---

## 2. 기능 매트릭스 (우선순위 / 마일스톤)

### M1 — MVP (P0)
- 로컬 ConPTY 셸(PowerShell/cmd/WSL/Git Bash, 자동 감지)
- SSH 코어(russh 0.61.x) password + public-key 인증
- keyboard-interactive + ssh-agent(OpenSSH 네임드 파이프/Pageant)
- 인터랙티브 PTY: request_pty + window_change(리사이즈)
- **strict known_hosts + 호스트키 SHA256 지문 TOFU 확인 다이얼로그**(러스트 `russh::keys` 헬퍼 활용, 정책/프롬프트만 자작) — MITM 치명
- 세션 매니저 트리(저장 세션/폴더/임포트·익스포트)
- 탭 세션 + 이진 분할(egui_dock 0.19, `TabViewer<PaneId>`)
- GPU 터미널 렌더러(egui_wgpu instanced, grayscale+감마정확 AA, damage 게이트)
- 풀다운 메뉴바 + 커맨드 팔레트
- 스크롤백 검색(find/regex/copy mode/quick-select)
- ~/.ssh/config 임포트(russh-config)
- Windows Credential Manager 저장(keyring 4.x, 접근 직렬화, zeroize/secrecy)

### M2 (P1)
- SFTP 브라우저(russh-sftp 2.x, 드래그&드롭, 재귀 전송)
- 포트 포워딩 로컬(-L)/원격(-R) TCP+UNIX + 그래픽 터널 매니저
- 키 생성 + SSH 키 매니저(Ed25519/RSA/ECDSA, PPK 임포트 — 실제 키로 검증)
- **자격증명 볼트**(Argon2id KDF + AES-256-GCM, 자동 잠금)
- **탭/패널 네이티브 OS 창 분리**(deferred viewport WindowManager 자작 — egui_dock 기본 분리는 in-app 플로팅뿐)
- 설정/Preferences 다이얼로그(figment 계층 설정, PuTTY 깊이, 핫리로드 via 오케스트레이터)
- 세션 임포트/익스포트(PuTTY 레지스트리/MobaXterm/JSON·TOML; 기본 비밀 제외)
- OSC 133/633 프롬프트 마크 + 명령 캡처(jump-to-prompt, exit code; best-effort + 센티넬 폴백)

### M3 (P2)
- **MultiExec**(다중 세션 브로드캐스트) — one-shot은 async-ssh2-tokio, 인터랙티브는 PTY 브로드캐스트
- 매크로/스니펫/로그인 스크립트
- **ProxyJump 체이닝**(direct-tcpip 자작) + per-session proxy
- 동적/SOCKS 포워딩(-D, 자체 SOCKS5 서버, localhost 기본)
- X11 포워딩(VcXsrv/X410 프록시; russh는 배관만, MIT-MAGIC-COOKIE 자작)
- Serial/Telnet 세션 타입(동일 VT 모델; Serial BREAK)
- 인라인 이미지(Sixel/iTerm2/Kitty)
- 테마/색구성 매니저 + 라이브 핫리로드

### M4 (P3)
- PQ KEX 선택 UI(mlkem768x25519-sha256 기본; 백엔드 ML-KEM 탑재 의존)
- 에이전트 포워딩 제어(기본 OFF, 타임아웃, 경고)
- 클라우드 동기 암호화 볼트(E2E)
- 플러그인/스크립팅(트레잇 호스트 API → WASM Extism/wasmtime; 샌드박스)
- RDP/VNC 세션 타입(IronRDP/VNC; SSH 범위 밖 — scope creep 주의)
- Zmodem 전송(rz/sz)
- 멀티 윈도우 워크스페이스/레이아웃 저장·복원

---

## 2.5 추가 기능 — 다국어 · 파일 전송 · 세션 (요구사항 반영)

상세: [i18n-cjk.md](./i18n-cjk.md) · [file-transfer.md](./file-transfer.md) · [sessions-storage.md](./sessions-storage.md).

**추가/변경 메뉴 항목**
- **File › New Session Type** 서브메뉴에 **FTP / FTPS** 추가(브라우저 백엔드 공유).
- **File › Export Sessions…** / **Import Sessions…** — 저장 세션 목록 내보내기/가져오기(TOML·JSON; OpenSSH/PuTTY/MobaXterm 상호운용; 비밀 제외).
- **(브라우저 패널) 파일 우클릭 컨텍스트 메뉴 › Edit (External Editor)** — 다운로드→설정 편집기 실행→저장 시 자동 재업로드. + "Open with Default App", "Edit with…".
- **Settings › General/Startup › Language** — 한국어/English/日本語 런타임 전환(+ OS 언어 자동 감지).
- **Settings › Security & Vault › Storage Location** — 세션 저장 위치 지정, **Portable Mode**(exe 옆 저장), StorageMode/드라이브 경고, 재배치(copy-verify-switch).
- **Terminal › Character Encoding**(기존) — UTF-8 기본 + Shift_JIS/EUC-KR(CP949)/GBK/GB18030/Big5 세션별 오버라이드.
- **View › Toggle SFTP/FTP Browser Panel**(기존 확장) — SSH/FTP 로그인 시 자동 오픈, "Follow Terminal Folder" 토글(OSC 7).

**기능 매트릭스 추가**
| 기능 | 우선순위 | 마일스톤 |
|------|:--:|:--:|
| UI 다국어(ko/en/ja, Fluent) + 런타임 전환 | P0 | M1 |
| CJK IME 입력(Event::Ime 직접 처리) + 이중폭 렌더 + 폰트 폴백(fontdb) | P0 | M1 |
| 레거시 CJK 인코딩 디코딩 탭(encoding_rs) + UTF-8 기본 | P0(UTF-8)/P2(레거시 UI) | M1/M3 |
| 세션 저장 위치 지정 + 포터블 모드(directories) | P0 | M1 |
| 세션 내보내기/가져오기(TOML·JSON, OpenSSH/PuTTY/MobaXterm) | P1 | M2 |
| SFTP 브라우저 자동 오픈(같은 연결) + cwd 추적(OSC 7) | P1 | M2 |
| 외부 편집기 편집(다운로드/감시/재업로드, notify+suppaftp/sftp) | P1 | M2 |
| FTP/FTPS 세션 타입(suppaftp, RemoteFs 공유 백엔드) | P2 | M3 |

---

## 3. 보안 노트 요약

상세는 [ssh-security.md](./ssh-security.md). 핵심: 볼트=Argon2id+AES-256-GCM(난스 매회, master password 미저장·태그검증), 자동 잠금+zeroize, Windows Credential Manager/DPAPI(keyring, 접근 직렬화), known_hosts=russh 헬퍼+자작 TOFU 프롬프트, 호스트키 지문+randomart, 키 변경 시 차단, safe-by-default(포워딩 OFF·strict host-key ON), 페이스트 보호, 로그/리포트 비밀 레닥션, tear-out 시 비밀은 오케스트레이터만 소유(PaneId만 이동), PQ KEX는 미지원 시 silent gap 금지.
