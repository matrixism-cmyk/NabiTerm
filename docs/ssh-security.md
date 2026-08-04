# SSH 스택 · 포워딩 · 보안 · 자격증명 볼트

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련 [architecture.md](./architecture.md)

---

## 1. SSH 스택 선택 (✅ 검증)

**russh 0.61.2** (순수 Rust, 비동기/tokio, Apache-2.0)을 코어로 채택. thrussh(휴면)·libssh2 기반 ssh2/async-ssh2(C 의존, 동기/구버전)보다 우월 — Windows에서 OpenSSL/libssh2 C 툴체인 불필요, 오케스트레이터의 tokio와 자연 정합, OpenSSH 인증서·PPK 임포트 기본 지원.

- ⚠️ **러스트 키는 `russh::keys`(내장)** — 0.49부터 구 `russh-keys` 크레이트가 `russh::keys`로 병합(RustCrypto `ssh-key` 재노출). 별도 russh-keys 의존 추가하지 말 것.
- ⚠️ **크립토 백엔드 정확히 하나 필수**(`ring` 또는 `aws-lc-rs`; 둘 다 끄면 컴파일 실패). 기본 ring, FIPS/PQ는 aws-lc-rs feature gate. ([architecture.md §6](./architecture.md))
- ⚠️ **russh는 minor마다 breaking change** → 전부 `nabi-ssh` 한 크레이트 뒤로 격리, 정확한 버전 핀.
- 인터랙티브 PTY는 **raw `client::Handler`를 직접 구동**(async-ssh2-tokio는 PTY API 없음 — one-shot exec 전용, MultiExec에만 활용).

## 2. 인증 (M1 P0)

russh 네이티브: password, public-key(ssh-ed25519/rsa-sha2-256·512/ecdsa-nistp256·384·521, OpenSSH 인증서, PPK), keyboard-interactive, ssh-agent(Windows OpenSSH 네임드 파이프 / Pageant). `auth.rs`/`keys.rs`에서 처리. ⚠️ **PPK 암호화 키 폭넓은 호환은 미검증** → 실제 PuTTY 내보내기(암호화 포함)로 검증 후 PuTTY 이주 사용자에게 광고.

## 3. 호스트키 검증 & known_hosts (M1 P0, MITM 치명)

⚠️ **검증 정정(중요):** 호스트키 검증은 `client::Handler::check_server_key`(기본 구현은 모두 거부)에서 수행. **그러나 known_hosts 파싱을 직접 짤 필요 없음** — `russh::keys`가 헬퍼를 **내장**한다:
- `check_known_hosts` / `check_known_hosts_path` — 조회(알고리즘 일치+키 변경 시 MITM 감지 에러)
- `learn_known_hosts` / `learn_known_hosts_path` — 신규 항목 추가(TOFU의 "기억" 절반)

→ 앱이 직접 제공할 것은 **검증 정책 + 인터랙티브 TOFU 프롬프트**(미지 키 수락/학습 결정)뿐. russh엔 full *manager*(항목 삭제/회전/조회 API)는 없으므로 그 관리 UI(`nabi-ui-panels::hostkey_dialog.rs`)와 회전/삭제 로직만 자작.

**UX:** 첫 연결 시 서버 키의 **SHA256 지문 + ASCII randomart** 표시 후 확인받아 기록. **키가 바뀌면** 눈에 띄는 경고 + 자동연결 차단 + 명시적 사용자 조치 요구. 호스트키 검사 비활성은 숨김·경고·세션별 opt-out으로만.

## 4. 인터랙티브 PTY 통합 (M1 P0)

각 원격 pane은 `client::Handler` 구현 → 인증 후 `channel_open_session()` → `request_pty(...)`(현재 cols/rows + term type). 채널 data 콜백이 **ConPTY와 동일한 `ByteChannel`**로 바이트를 `nabi-vt`에 주입 → 로컬/원격 VT 모델 동일. 리사이즈 시 `window_change(channel, cols, rows, ...)`. (`pty_channel.rs`)

## 5. SFTP (M2 P1)

russh-sftp 2.3.0: `channel_open_session` → `request_subsystem(true, "sftp")` → `SftpSession::new(channel.into_stream())`. fs-유사 비동기 ops + 재귀 업/다운로드(`nabi-sftp::transfer.rs`). UI는 `nabi-ui-panels::sftp_browser.rs`(드래그&드롭, 진행률), SSH 로그인 시 우측 패널 자동 오픈 옵션.

## 6. 포트 포워딩 & 점프 (M2–M3)

- ✅ **로컬(-L)/원격(-R) TCP + UNIX 소켓**: russh 네이티브. ⚠️ 클라이언트 -R 콜백 갭(#183 open; #126은 closed) → 핀 버전에서 동작 확인, 필요 시 `forward_remote.rs`에서 보강.
- ⚠️ **ProxyJump / 동적 SOCKS(-D)는 turn-key 미제공** → `nabi-ssh-ext`에서 자작:
  - `proxy_jump.rs`: 점프 호스트에 `direct-tcpip` 채널을 열고 그것을 다음 홉의 transport 스트림으로 래핑(체이닝).
  - `socks5.rs`: 자체 SOCKS5 서버가 연결마다 `direct-tcpip` 채널을 연다. **기본 localhost 바인드**, 원격 바인드는 명시 토글.
- `russh-config`로 `~/.ssh/config` 파싱(HostName/User/Port/ProxyJump). ⚠️ russh는 `%h/%p` 등 ProxyCommand 토큰 확장 일부 미지원 → 임포트 시 한계 표시.

## 7. X11 포워딩 (M3 P2)

⚠️ **검증 정정:** `Channel::request_x11`은 **세션 채널 위에서** x11-req를 보냄(요청 시점엔 X11 채널 없음, RFC 4254 §6.3). 이후 로컬 X11 클라이언트가 붙으면 **서버가** x11 채널을 열고, russh가 이를 `client::Handler::server_channel_open_x11` 핸들러로 전달. → 앱이 그 채널을 받아 로컬 X 서버(VcXsrv/X410)로 프록시. russh는 프로토콜 배관만 제공 → **MIT-MAGIC-COOKIE 처리·포워딩은 앱 책임**(`x11_channel.rs`). 인-프로세스 X 서버는 후순위.

## 8. 자격증명 볼트 & 저장 (M1 저장 / M2 볼트)

- ✅ **OS 저장(keyring 4.0.1):** 호스트 비밀번호·키 패스프레이즈를 **Windows Credential Manager**(DPAPI 백엔드, 사용자/머신 바인딩)에 저장. ⚠️ Credential Manager는 스레드 간 호출 순서를 보장 안 함 → **모든 접근을 직렬화**(`nabi-secret::serialize_guard.rs`, Mutex).
- ⚠️ **Credential Manager ≠ DPAPI(동격 아님):** Credential Manager = 관리형 영구 저장(keyring 사용); DPAPI(`CryptProtectData`) = blob 암호화 프리미티브(직접 저장처에 보관). 설정파일에 blob을 넣고 싶을 때 DPAPI 사용.
- **앱 볼트(`vault.rs`):** master password → **Argon2id**(OWASP, 메모리 하드, ≥64 MiB/조정 iters/parallelism, 파라미터를 볼트와 함께 저장) → 키 → **AES-256-GCM**(또는 ChaCha20-Poly1305), 암호화마다 새 96-bit 난스. master password는 절대 저장 안 함; 검증은 별도 해시가 아니라 **AEAD 인증 태그(decrypt-and-verify)**로.
- **자동 잠금:** 실행 시 master password 요구, 유휴 타임아웃·화면잠금 시 auto-lock. 잠금/종료 시 파생 키·복호 비밀·패스프레이즈·키 바이트를 **zeroize**.
- ⚠️ **zeroize 한계:** 과거에 디스크로 swap/move된 비밀은 못 지움, 마이크로아키텍처 사이드채널 보장 없음 — 메모리 와이프는 하드닝일 뿐, 별도 위협으로 명시. 모든 비밀은 `secrecy::SecretBox`로 감싸 Debug 마스킹.
- (선택) Windows Hello/DPAPI로 볼트 키를 OS 컨텍스트에 바인딩(심층 방어, master password 대체 아님).

## 9. 보안 기본값 & 명령 캡처

- **safe-by-default:** 포트/에이전트/동적 포워딩 OFF, strict host-key checking ON. ProxyJump를 에이전트 포워딩보다 우선(에이전트 포워딩은 중간 호스트에 소켓 노출 → 측면 이동 위험). 에이전트/동적 포워딩은 경고 동반 opt-in, `ssh-add -t`식 키 수명 타임아웃.
- **페이스트 보호:** bracketed-paste 인식, 멀티라인 페이스트 경고(은닉 명령 주입 방지).
- ⚠️ **OSC 133/633 명령 캡처는 best-effort:** 원격 셸이 마커를 방출하도록 설정돼야 하고(스톡 PowerShell은 기본 미방출 — `prompt` 설정/주입 필요), C(출력 시작) 마커가 자주 누락되며 PS2/리다이렉트 fd 엣지케이스 존재. → 마커 부분집합 허용 + **센티넬 폴백**(`nabi-osc::sentinel.rs`, 예 `; echo __DONE_$LASTEXITCODE__`), 캡처 입력을 **untrusted로 파싱**.
- **레닥션:** tracing/크래시 리포트/지원 번들에서 비밀 제거. 세션 로깅은 민감정보 포함 가능 경고.
- **세션 이전 무결성(tear-out):** 라이브 세션·자격증명은 오케스트레이터 소유, **PaneId만 이동**. 비밀을 viewport 클로저/DockState에 복사 금지.

## 10. 현대/포스트양자 크립토 (M4 P3)

KEX 선호도를 PQ 우선으로(OpenSSH 10.0 정합): `mlkem768x25519-sha256` → `sntrup761x25519-sha512` → `curve25519-sha256`. 암호는 ChaCha20-Poly1305/AES-GCM 우선, 약한/레거시 기본 비활성. ⚠️ PQ KEX는 russh 크립토 백엔드(aws-lc-rs)의 ML-KEM 실제 탑재에 의존 — 미지원이면 **silent gap 금지, 추적 의존 항목으로 표면화**.
