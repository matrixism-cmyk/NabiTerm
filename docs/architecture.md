# 아키텍처 — 워크스페이스 · 모듈 · 데이터 흐름 · 라인 제한

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md)

## 1. 워크스페이스 레이아웃

matklad 권장 플랫 레이아웃: `crates/` 아래 한 폴더 = 한 크레이트, 폴더명 = 크레이트명. 내부 크레이트는 모두 `version = "0.0.0"`, `publish = false`. 단일 `Cargo.lock`, 단일 `target/`.

```
nabi/
├─ Cargo.toml            # [workspace] + [workspace.dependencies] (모든 외부 버전 정확히 핀)
├─ rustfmt.toml          # max_width 고정 (라인 카운트 안정화)
├─ clippy.toml           # too-many-lines-threshold (함수 단위 보조 캡)
├─ docs/                 # 본 계획 문서
├─ xtask/                # 라인 게이트·CI 자동화 (워크스페이스 멤버지만 라이브러리 의존 아님)
└─ crates/
   ├─ nabi-types  nabi-proto  nabi-error  nabi-log      # 기반/인프라
   ├─ nabi-config  nabi-secret  nabi-plugin-api
   ├─ nabi-vt  nabi-osc  nabi-pty                       # 도메인(로컬)
   ├─ nabi-ssh  nabi-ssh-ext  nabi-sftp                 # 도메인(원격)
   ├─ nabi-orchestrator                                  # 제어 평면(핵심)
   ├─ nabi-render                                        # GPU 렌더
   ├─ nabi-ui-tab  nabi-ui-window  nabi-ui-menu  nabi-ui-panels  # UI
   └─ nabi-app (bin)                                     # 진입점/와이어링
```

## 2. 의존성 계층 (위가 아래에 의존)

```
nabi-app
  └─ nabi-ui-window → nabi-ui-tab → nabi-render → nabi-vt
       └─ nabi-ui-panels → nabi-sftp/nabi-ssh-ext
  └─ nabi-ui-menu
  └─ nabi-orchestrator → { nabi-vt, nabi-osc, nabi-pty, nabi-ssh, nabi-config, nabi-secret, nabi-log }
       └─ nabi-ssh → { nabi-pty(ByteChannel trait), nabi-secret }
nabi-types ← (거의 모든 크레이트)      nabi-proto ← (UI·오케스트레이터·도메인)
```
순환 의존 방지를 위해 `nabi-types`(ID/기하/색)와 `nabi-proto`(Command/Event 메시지 타입)는 로직·I/O 없이 어휘만 제공.

## 3. 크레이트 책임표

| 크레이트 | 책임 | 핵심 모듈(예) |
|----------|------|----------------|
| **nabi-types** | PaneId/SessionId/WindowId/TabId/ViewportKey 뉴타입, 좌표/크기, 색, 셀 속성. 로직 없음 | `ids.rs` `geometry.rs` `color.rs` `attrs.rs` |
| **nabi-proto** | UI↔오케스트레이터 버스의 `Command`/`Event` enum, Pane Input/Output 프레임. 채널 아님, 타입만 | `command.rs` `event.rs` `pane_msg.rs` `window_msg.rs` |
| **nabi-error** | 공용 에러 스캐폴딩(소수 공유 변형/변환 글루). 각 크레이트는 자기 thiserror enum 소유 | `kind.rs` `context.rs` |
| **nabi-log** | tracing/subscriber 설정, per-pane/session span, 인-앱 로그 패널용 채널 싱크 | `subscriber.rs` `span.rs` `app_sink.rs` |
| **nabi-config** | figment 계층 설정(defaults→user TOML→per-session→env), 테마, notify 핫리로드(오케스트레이터로 ReloadConfig 전송) | `schema.rs` `providers.rs` `theme.rs` `watch.rs` `merge.rs` `persist.rs` |
| **nabi-secret** | Argon2id KDF, AES-256-GCM 볼트, master-password, keyring(Windows Credential Manager), zeroize/secrecy. 모든 자격증명 접근 직렬화 | `vault.rs` `kdf.rs` `aead.rs` `keyring_store.rs` `secret_box.rs` `serialize_guard.rs` |
| **nabi-vt** | per-pane alacritty_terminal Grid+스크롤백, 셀 속성, damage 추적, wide/CJK, 선택. I/O 없음(바이트 in→상태 out) | `grid.rs` `scrollback.rs` `cell.rs` `damage.rs` `selection.rs` `cursor.rs` `resize.rs` |
| **nabi-osc** | OSC133(A/B/C/D) + OSC633(VS Code) 명령 경계 + 센티넬 폴백. CommandBlock(명령/출력범위/exit code) 이벤트 방출 | `osc133.rs` `osc633.rs` `sentinel.rs` `command_block.rs` `scanner.rs` |
| **nabi-pty** | ConPTY(portable-pty): 셸 스폰, 비동기 read/write 펌프, resize, 종료. `ByteChannel` 트레잇 정의 | `conpty.rs` `spawn.rs` `pump.rs` `resize.rs` `channel_trait.rs` |
| **nabi-ssh** | russh 0.61.x: client::Handler, 인증, channel_open_session+request_pty+window_change, known_hosts/TOFU. `ByteChannel` 구현 | `handler.rs` `auth.rs` `known_hosts.rs` `tofu.rs` `pty_channel.rs` `keys.rs` `config_import.rs` `keepalive.rs` |
| **nabi-ssh-ext** | russh 미제공 기능: ProxyJump 체이닝, -L/-R/동적 SOCKS5, X11 채널 핸들러, 에이전트 | `proxy_jump.rs` `forward_local.rs` `forward_remote.rs` `socks5.rs` `x11_channel.rs` `agent.rs` |
| **nabi-sftp** | russh-sftp 2.x: 세션, fs-유사 ops, 재귀 업/다운로드, 브라우저 데이터 모델 | `session.rs` `ops.rs` `transfer.rs` `listing.rs` |
| **nabi-orchestrator** | 제어 평면/액터: 모든 Pane 상태 단일 소유, Command 수신/Event 방출, 바이트 라우팅, 자격증명 접근 직렬화, crossbeam↔tokio seam | `actor.rs` `pane_registry.rs` `router.rs` `commands.rs` `events.rs` `bridge.rs` `spawn_pane.rs` `resize_dispatch.rs` |
| **nabi-render** | GPU: egui_wgpu CallbackTrait, grid→instances, CellBg/CellText 패스, grayscale+RGBA 아틀라스, 커서/선택, 인라인 이미지 | `callback.rs` `atlas_alloc.rs` `atlas_upload.rs` `glyph_cache.rs` `run_cache.rs` `shaper.rs` `instance_build.rs` `pass_bg.rs` `pass_text.rs` `pass_cursor.rs` `pass_image.rs` `image_sixel.rs` `image_kitty.rs` `image_iterm.rs` |
| **nabi-ui-tab** | egui_dock `TabViewer<Tab=PaneId>`: 오케스트레이터에서 pane 조회 후 렌더 콜백 발행, 탭 제목/닫기/컨텍스트 | `tab_viewer.rs` `tab_title.rs` `tab_context.rs` |
| **nabi-ui-window** | WindowManager: N개 네이티브 뷰포트(`show_viewport_deferred`), 뷰포트당 `DockState<PaneId>`, **tear-out 제스처(자작)**, 닫기 처리, per-viewport repaint wake | `window_manager.rs` `viewport_spawn.rs` `dock_host.rs` `tear_off.rs` `repaint_wake.rs` `close_handler.rs` `layout_persist.rs` |
| **nabi-ui-menu** | 메뉴바(`egui::containers::menu::MenuBar`), 중첩 서브메뉴, **자작 전역 단축키 테이블**, 커맨드 팔레트(메뉴와 패리티) | `menu_bar.rs` `menu_file.rs` `menu_sessions.rs` `menu_terminal.rs` `menu_view.rs` `shortcuts.rs` `palette.rs` |
| **nabi-ui-panels** | 그리드 외 다이얼로그/패널: 세션 매니저 트리, 호스트키(TOFU)/known_hosts, 볼트 잠금해제, 설정(PuTTY식 카테고리), SFTP 브라우저, 터널 매니저, 로그 패널 | `session_tree.rs` `hostkey_dialog.rs` `vault_unlock.rs` `settings/*.rs` `sftp_browser.rs` `tunnel_manager.rs` `log_pane.rs` |
| **nabi-plugin-api** | (M4) 트레잇 객체 플러그인 호스트 API. WASM(Extism/wasmtime) 경계로 무손상 이전 가능하게 지금 계약만 정의 | `plugin.rs` `host_api.rs` `registry.rs` |
| **nabi-app** | bin: eframe 진입점, NativeOptions/wgpu 선택, tokio 부트스트랩, 와이어링. 비즈니스 로직 없음 | `main.rs` `bootstrap.rs` `runtime.rs` `app.rs` |
| **xtask** | 라인 게이트(`tokei --files --output json` 파싱: >400 실패, >250 경고, override allowlist), fmt/clippy 오케스트레이션 | `main.rs` `lines.rs` `overrides.rs` `ci.rs` |

> 모듈 라인 예산은 거의 전부 ≤230줄 목표(여유롭게 400 미만). lib.rs는 `pub mod`/`pub use` facade만. 트레잇 impl은 `impl_*.rs`로 분리. 메뉴/다이얼로그/렌더패스/포워딩모드/프로토콜디코더는 1파일 1개.

## 3b. 확장 크레이트 (다국어 · 파일 전송 · 세션)

추가 요구사항(한/영/일, MobaXterm식 FTP/파일 브라우저, 외부 편집기 편집, 세션 내보내기/저장 위치)으로 신설되는 크레이트·모듈. 상세는 [i18n-cjk.md](./i18n-cjk.md) · [file-transfer.md](./file-transfer.md) · [sessions-storage.md](./sessions-storage.md).

| 크레이트 | 책임 | 핵심 모듈 | 의존 |
|----------|------|----------|------|
| **nabi-i18n** (신규) | UI 다국어(ko/en/ja). Fluent(`fluent-templates` ArcLoader), `tr!` 매크로, `set_language`, .ftl 임베드 | `loader.rs` `lang.rs` `macros.rs` `locales/{en,ko,ja}.ftl` | nabi-types |
| **nabi-fs** (신규) | 백엔드 무관 `RemoteFs` 트레잇(list/stat/get/put/mkdir/remove/rename+Cwd) + DTO. SFTP/FTP가 한 브라우저 공유 | `remote_fs.rs` `sftp_backend.rs`(russh-sftp 재사용) | nabi-ssh, nabi-sftp, nabi-error |
| **nabi-ftp** (신규) | FTP/FTPS 백엔드(`suppaftp` 8.x, tokio+rustls-ring). `RemoteFs` 구현 + 세션 | `ftp_backend.rs` `session.rs` | nabi-fs, nabi-secret, nabi-proto |
| **nabi-editor** (신규) | 외부 편집기 편집: 임시 다운로드→편집기 실행→감시→재업로드 | `external_editor.rs` `watch_reupload.rs` | nabi-fs, nabi-config, nabi-error |
| **nabi-session** (신규) | 저장 세션 트리·내보내기/가져오기·상호운용 | `model.rs` `store.rs` `export.rs` `import.rs` `schema_version.rs` `interop_openssh.rs` `interop_putty.rs` `interop_mobaxterm.rs` | nabi-config, nabi-secret, nabi-types |

**기존 크레이트에 추가되는 모듈:**
- `nabi-vt`: `decode_tap.rs`(encoding_rs 스트림 디코딩 탭, ByteChannel↔VT파서 사이), `encoding.rs`(라벨↔Encoding). 기본 UTF-8, 세션별 오버라이드.
- `nabi-render`: `font_fallback.rs`(⚠️ fontdb 기반 코드포인트별 CJK 폴백 — cosmic-text FontSystem 미채택 시 직접 구현; [i18n-cjk.md §3](./i18n-cjk.md)), WIDE_CHAR 2칸 처리.
- `nabi-ui`(egui 계층): `ime.rs` `ime_preedit.rs` `ime_rect.rs`(CJK IME 입력 — Event::Ime 직접 처리, 커서 rect 보고).
- `nabi-config`: `paths.rs`(StorageLayout 단일 진실원, `directories::ProjectDirs`) `base_dir.rs`(CLI>env>portable>default) `portable.rs` `drive_kind.rs`. `persist.rs`는 원자적 쓰기 확장.
- `nabi-secret`: `vault_location.rs`(StorageLayout 볼트 경로; ⚠️ 포터블/동기화 시 DPAPI 제외·master password 강제).
- `nabi-orchestrator`: `cwd_tracker.rs`(OSC 7/1337 파싱→CwdChanged, 셸 통합 스니펫 폴백).
- `nabi-ui-panels`: `browser_panel.rs`(SFTP/FTP 듀얼페인, 우클릭 Edit, 드래그&드롭, follow-cwd), `settings/storage.rs`(저장 위치/포터블/재배치), `settings/language.rs`.

> 크레이트 총수 ~22 → ~27. 새 외부 핀: fluent-templates 0.14 · encoding_rs 0.8.35 · suppaftp 8.0.3 · notify 8.2 + notify-debouncer-full 0.5 · directories 6.0.0 · rust-ini 0.21.3 · tempfile/which/open/blake3/fontdb/unicode-width. 모두 루트 `[workspace.dependencies]`에 핀.

## 4. 데이터 흐름

**인바운드(바이트 → 화면):** ConPTY(`nabi-pty`) 또는 SSH 채널(`nabi-ssh`) — 둘 다 `ByteChannel` — 이 바이트 스트림 생성 → `nabi-orchestrator::router`가 `nabi-osc` 탭(OSC133/633/센티넬, CommandBlock 이벤트)을 통과시킨 뒤 해당 pane의 `nabi-vt`로 주입(damage 기록) → router가 PaneOutput 이벤트 + **그 PaneId를 띄운 특정 viewport에만** `ctx.request_repaint_of(viewport_id)`. 유휴 pane은 damage 없음 → repaint 없음 → GPU 0.

**아웃바운드(입력 → 세션):** `TabViewer::ui`의 키/페이스트 → `WriteInput`/`Resize` Command → 오케스트레이터가 PaneId 조회 → 해당 `ByteChannel`에 write(ConPTY write 또는 SSH data; resize→request_pty 크기/window_change). **메뉴·커맨드 팔레트·패널 다이얼로그 모두 동일한 `nabi-proto::Command`를 방출** → 메뉴와 팔레트가 항상 패리티.

**렌더 경로:** dirty 프레임마다 `TabViewer::ui`가 grid rect 할당 → egui_wgpu paint 콜백 발행 → `nabi-render`가 오케스트레이터 핸들/스냅샷으로 VT 셀 읽기 → CellBg/CellText instance 버퍼 구성(가시 그리드 전체; instancing으로 빈 셀 거의 무료) → 공유 grayscale/RGBA 아틀라스 샘플 → egui 렌더 패스 내 clip rect로 그리기. shaped-run/glyph 캐시가 CPU 셰이핑 비용 흡수.

**뷰포트/스레드:** 각 네이티브 창 = deferred viewport(`show_viewport_deferred`), 독립 repaint(바쁜 창이 다른 창을 강제 repaint하지 않음 → immediate viewport의 N×CPU 회피). SSH/네트워크 = tokio 런타임(연결당 task); PTY 펌프와 egui UI = 각자 스레드; 오케스트레이터 액터 task가 레지스트리 소유·랑데부. 창 이동은 PaneId만 옮기므로 안전; viewport 닫힘 시 패널을 오케스트레이터로 fold(또는 파괴), 라이브 채널 drop 안 함.

**동시성 seam:** crossbeam-channel(동기 UI/PTY) ↔ tokio mpsc/oneshot(비동기 net/SSH)을 **`nabi-orchestrator::bridge` 한 곳**에서 변환. config/theme 같은 read-mostly 데이터는 tokio `watch` 채널로 최신값만 전 viewport에 브로드캐스트.

## 5. 라인 제한 강제 (방어 3중)

1. **`cargo xtask lines` (권위 게이트):** `tokei --files --output json`을 `crates/`에 실행 → 파싱. 소스 파일 **>400줄 = exit 0 아님(하드 실패)**, **>250줄 = 경고 목록**. tokei엔 임계 기능 없으므로 deny/warn 로직은 `xtask/src/lines.rs`에 자작. (a) rustfmt `max_width`와 **일치하는 카운트 기준**(재포맷이 몰래 초과시키지 않게), (b) 정당하게 큰 파일(생성 코드·큰 match 표·속성 스키마)용 **override allowlist + 사유 문자열**(`xtask/src/overrides.rs`).
2. **pre-commit 훅(로컬 빠른 실패):** rusty-hook/devx-pre-commit으로 `cargo fmt --check` → `cargo xtask lines` → `cargo clippy -- -D warnings` 순.
3. **CI 게이트:** 동일 트리오 + `clippy.toml`의 `too-many-lines-threshold`(함수 단위 보조 캡 → TabViewer::ui·라우터·메뉴 빌더가 자연히 작아짐). `rustfmt.toml`의 `max_width` 핀으로 카운트 안정.

**설계 차원(게이트가 거의 안 울리게):** 크레이트 분할(서브시스템마다 자기 크레이트) + 모듈 분할(파일당 단일 책임, lib.rs는 facade) 병행. **250 = 설계 목표, 400 = 절대 넘지 않는 천장.**

## 6. 빌드 툴링 & 버전 핀

- 루트 `[workspace.dependencies]`에 **모든 외부 크레이트 정확히 핀.** egui-wgpu가 정하는 **wgpu가 단일 진실원**(glyphon/sugarloaf 재사용 시 정확히 일치 안 하면 링크 실패).
- **2026-06 핀:** egui/eframe/egui-wgpu 0.34.x · egui_dock 0.19.1 · russh **0.61.2(정확)** · russh-sftp 2.3.0 · russh-config 0.58.x · keyring 4.0.1 · portable-pty 0.9.x.
- **크립토 백엔드(초기 결정, CI 영향):** russh는 `aws-lc-rs` 또는 `ring` 중 **정확히 하나** 필수(둘 다 끄면 컴파일 실패). aws-lc-rs는 빠르고 FIPS 가능하나 빌드에 C/CMake/NASM 필요(Windows CI 복잡). **권장: CI 기본 ring(깨끗한 Windows 빌드), aws-lc-rs는 cargo feature gate(FIPS/PQ 사용자).** PQ KEX(mlkem768x25519-sha256) 지원 백엔드는 약속 전 확인.
- **에러/로그 규약:** 라이브러리 크레이트마다 thiserror enum 1개(`#[from]/#[source]`로 원인 보존, 에러 파일 작게). anyhow는 `nabi-app`/main·글루에만(lib 경계로 누출 금지). tracing+subscriber는 `nabi-log` 통해 워크스페이스 전역.
- **CI 매트릭스:** 주 타깃 `x86_64-pc-windows-msvc`. 단계: `fmt --check` → `xtask lines` → `clippy -D warnings` → `cargo test`(portable-pty/expectrl PTY 통합 테스트, insta/expect-test로 VT 화면 스냅샷; SSH는 로컬 russh 서버/컨테이너 sshd 대상). 보안 민감 크레이트(secret/ssh)는 cargo-deny/cargo-audit. 다중 크레이트 컴파일 가속 위해 sccache + 공유 target 고려.
- **플러그인 페이징:** `nabi-plugin-api`는 지금 트레잇 계약만, WASM 런타임(Extism/wasmtime)은 연기. ⚠️ Extism은 wasmtime 기반, Zellij는 wasmi 사용 — 둘 중 선택은 M4에서.
