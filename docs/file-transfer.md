# 파일 전송 — SFTP/FTP 브라우저 · 외부 편집기 · cwd 추적

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련 [ssh-security.md](./ssh-security.md)

요구: MobaXterm처럼 **SSH 접속 시 파일 브라우저(SFTP/FTP)를 함께 사용**, **파일 우클릭 → 편집 → 외부 편집기로 편집**(저장 시 자동 재업로드), 터미널 cwd를 따라가는 브라우저.

---

## 1. MobaXterm 목표 UX (✅ 확인)

SSH 로그인 시 **같은 연결**로 그래픽 SFTP 브라우저를 좌측 사이드바에 자동으로 띄움(2차 인증 없음). 드래그&드롭 업/다운로드, 원격 파일 더블클릭으로 로컬 앱에서 열기, **브라우저 하단 "follow terminal folder" 체크박스**로 셸에서 `cd` 시 브라우저가 따라감. 일부 구형 서버는 SFTP 대신 SCP로 채워짐.

## 2. 백엔드 추상화 (`nabi-fs` 신규)

SFTP와 FTP/FTPS가 **하나의 브라우저 UI**를 공유하도록 백엔드 무관 트레잇을 둔다.
- `nabi-fs/remote_fs.rs`: `RemoteFs` 트레잇 — `list_dir / stat / get(reader) / put(writer) / mkdir / remove / rename` + `Cwd`. + 공유 DTO(`FileEntry`/`FileKind`/`Perms`)만(≤250줄).
- `nabi-fs/sftp_backend.rs`: 기존 **russh-sftp** 세션 위에 구현 — **이미 맺은 russh 채널 재사용**(2차 인증 없음), MobaXterm의 same-connection 모델 그대로.

## 3. FTP/FTPS 백엔드 (`nabi-ftp` 신규)

- ✅ **`suppaftp 8.0.3`**(MIT/Apache) — 유지보수되는 유일한 현대 Rust FTP/FTPS 클라이언트(구 `ftp` 크레이트는 ~8년 방치). tokio 비동기 + FTPS(explicit/implicit).
- **feature `tokio-async-rustls-ring`**(또는 `-aws-lc-rs`)으로 비동기 + FTPS를 **OpenSSL 빌드 없이**(Windows 친화). Windows 인증서 저장소 신뢰가 필요하면 `native-tls` opt-in feature 제공. explicit FTPS는 `into_secure()`, implicit는 deprecated feature.
- 모듈: `nabi-ftp/ftp_backend.rs`(`RemoteFs` 구현, passive/active, TLS provider 선택, FTP 응답→공유 에러), `nabi-ftp/session.rs`(연결/인증/NOOP keepalive/재연결, 오케스트레이터가 SSH pane처럼 소유).
- ⚠️ **항상 BINARY 전송 모드**(편집 파일의 바이트/줄끝 손상 방지). ⚠️ **평문 FTP는 비밀번호 평문 전송** → UI에서 경고하고 FTPS/SFTP로 유도, 자격증명은 볼트 저장.

## 4. 외부 편집기 편집 (`nabi-editor` 신규)

WinSCP/MobaXterm 방식: 원격 파일을 임시로 받아 외부 편집기로 열고, 저장을 감시해 재업로드.

- `nabi-editor/external_editor.rs`: 원격 파일 → `tempfile::TempDir`로 다운로드 → **사용자 설정 편집기**를 `which`로 해석 후 `std::process::Command`로 실행 → 다운로드 시점 **크기 + blake3 해시** 기록.
- `nabi-editor/watch_reupload.rs`: `notify 8.x` + `notify-debouncer-full`(~300–500ms)로 **임시 파일의 부모 디렉터리**를 감시 → debounce된 저장마다 해시 비교(무변경 skip) → 원격 mtime/size로 충돌 검사 후 `RemoteFs`로 **직렬화된 업로드 큐**에 재업로드 → 편집기 종료/세션 종료 시 임시 정리.
- **편집기 실행 정책(검증 정정):**
  - ⚠️ 설정 편집기 실행에는 **`std::process::Command` 사용**(또는 `open::with(path,"editor")`). **`opener`/`open` 기본 경로는 "OS 기본 앱으로 열기"라 부적합** — 더블클릭 "기본 앱으로 열기" 폴백에만 사용.
  - ⚠️ **탭형/포크형 편집기**(VS Code/Notepad++/Sublime)는 즉시 반환하므로 프로세스 종료로 "편집 끝"을 알 수 없음 → 계속 감시 + 명시적 "편집 중지" 액션. **VS Code는 `code --wait` 필요.** WinSCP식 "편집기가 파일을 별도 프로세스로 연다(블로킹)" per-editor 설정 제공.
  - ⚠️ **원자적 저장**(임시파일 작성 후 rename)이 Windows(ReadDirectoryChangesW)에서 원본 inode 감시를 끊고 Remove+Create로 나타남 → **부모 디렉터리 감시 + 재무장 + debounce** 필수.
  - ⚠️ **충돌:** 다운로드 후 원격이 바뀌었으면 맹목 재업로드가 덮어씀 → 다운로드 시 size+mtime(+해시) 저장, 업로드 전 재확인·프롬프트(서버 mtime 분해능/타임존 한계 감안).
- 브라우저 우클릭 컨텍스트 메뉴의 **Edit** → 이 모듈 호출. (메뉴 항목은 [menus-features.md](./menus-features.md))

## 5. 브라우저 패널 UI (`nabi-ui-panels::browser_panel`)

SSH/FTP 로그인 시 자동 오픈되는 egui 사이드 패널: 듀얼 페인(로컬/원격) 파일 목록, 브레드크럼 내비, 업/다운로드/mkdir/rename/delete, **우클릭 컨텍스트 메뉴(Edit → external_editor 등)**, 드래그&드롭(`raw.dropped_files` 인, 커스텀 드래그 아웃), 전송 진행률, "follow terminal folder" 토글. 라인 캡 준수를 위해 목록 렌더 / 액션 / 전송 큐 / 컨텍스트 메뉴를 작은 모듈로 분리.

## 6. 터미널 cwd 추적 (`nabi-orchestrator::cwd_tracker`)

브라우저가 셸의 `cd`를 따라가게 한다.
- ✅ **OSC 7**(`ESC ] 7 ; file://HOST/PATH ST`, 경로 percent-encoding) 및 iTerm2 **OSC 1337 `CurrentDir=`**를 각 pane의 PTY 바이트 스트림에서 파싱 → `CwdChanged` 이벤트. `browser_panel`이 follow 토글 ON일 때 구독.
- ⚠️ **OSC 7은 단방향 통지**(질의 불가) — 셸이 방출하도록 설정돼야 함(기본 sshd 셸은 대개 미방출). 우리가 로그인을 제어하므로, 미방출 서버용으로 세션 시작 시 **셸 통합 스니펫**(PROMPT_COMMAND/precmd에서 OSC 7 출력) 주입을 opt-in으로 제공(사용자 RC/제한 셸과 충돌 가능 → 견고하게, 한 청크에 다중 OSC 처리, 경로 URL-decode + Windows 드라이브용 선행 슬래시 제거).

## 7. 추가 크레이트 핀

| 크레이트 | 버전 | 라이선스 | 용도 |
|----------|------|----------|------|
| suppaftp | 8.0.3 | MIT/Apache | FTP/FTPS 클라이언트(tokio + rustls-ring) |
| notify | 8.2.0 | CC0-1.0 | 임시 파일 감시(ReadDirectoryChangesW) |
| notify-debouncer-full | 0.5.x | MIT/Apache | 저장 이벤트 디바운스/rename 추적 |
| tempfile | 3.x | MIT/Apache | 편집용 임시 디렉터리(drop 시 정리) |
| which | 7.x | MIT | 설정 편집기 바이너리 해석 |
| open | 5.x | MIT | 기본 앱으로 열기 폴백 + Command/Child 핸들 |
| blake3 | 최신 | CC0/Apache | 편집 파일 변경/충돌 감지 해시 |
