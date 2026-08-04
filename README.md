# 🦋 nabiTerm (나비텀)

**네이티브 Windows 터미널 멀티플렉서 + MobaXterm식 SSH 클라이언트** — Rust로 처음부터 만든 빠르고 가벼운 단일 실행 파일.

> A fast, native Windows terminal multiplexer and SSH/SFTP client, written from scratch in Rust.

---

## 소개

nabiTerm은 로컬 셸과 원격 서버를 하나의 창에서 다루기 위한 터미널입니다. 탭·분할·분리 창으로 여러 세션을 배치하고, SSH/SFTP/FTP로 원격에 접속하며, 파일 브라우저·내장 에디터·비밀번호 볼트까지 한 프로그램 안에 담았습니다. 외부 런타임이나 설치 의존성 없이 **단일 exe**로 동작하며, 자동 업데이트를 지원합니다.

전통적인 터미널의 안정성(견고한 VT 코어·스크롤백) 위에, 최신 터미널 트렌드(명령 블록, 하이퍼링크, 인라인 이미지, 스타일드 밑줄)와 **AI 에이전트 제어 평면**을 더한 것이 특징입니다.

## 주요 기능

### 터미널 · 세션
- **탭 / 분할 / 분리 창** — egui_dock 기반 도킹, 탭을 창 밖으로 끌어 분리, tmux식 pane 줌
- **로컬 셸** — PowerShell·cmd 등(Windows ConPTY), 종료 시 실행 중이던 명령/디렉터리 복원
- **SSH / SFTP / FTP** — 순수 Rust(russh) 비동기 접속, 호스트키 TOFU 확인, 포트 포워딩(-L·-R·-D·ProxyJump·X11)
- **멀티 실행(브로드캐스트)** — 여러 pane에 동시에 입력
- **검색 · 스크롤백** — 화면 내 찾기(스마트케이스), 우측 스크롤바, 프롬프트 점프
- **퀘이크 모드 · 전체화면 · 다국어(한/영/일) · 테마** — 8종 색 프리셋, 커서/선택 색 커스터마이즈

### 파일 · 편집
- **파일 브라우저** — 탐색기식 자세히/아이콘 보기, 드래그-아웃 복사, 내 컴퓨터/드라이브 용량, 로컬↔원격 이중창
- **내장 에디터** — 구문 강조(syntect), 자동 인코딩 감지, 라인 번호, 대용량 파일 가상 뷰어, Ctrl+휠 줌
- **비밀번호 볼트** — Argon2id + AES-256-GCM으로 SSH 자격증명 암호화 보관

### 최신 트렌드 기능
- **Warp식 명령 블록** — OSC 133 기반, 프롬프트 줄 좌측에 종료코드 색 막대(성공/실패)
- **하이퍼링크** — 휴리스틱 URL/경로 감지 + OSC 8 명시적 하이퍼링크, 길게 누르기 메뉴(복사/열기)
- **인라인 이미지** — Sixel · iTerm(OSC 1337) · Kitty(APC) 프로토콜 *(SSH 경로에서 동작; 로컬은 ConPTY 제약상 iTerm OSC만)*
- **스타일드 밑줄** — undercurl·이중·점선·파선 + 밑줄 색(SGR 58, nvim LSP 진단 등)

### AI 에이전트 제어 평면
- pane 안의 프로세스가 `nabi cli`(named pipe) 또는 MCP로 nabiTerm을 제어 — `list`·`spawn`·`send`·`capture`·`wait`·`notify` 등
- 권한 정책 **끔 / 물어봄 / 켬**(그룹별 1회 승인)
- 도움말 ▸ AI 제어에서 사용 가이드 복사/저장

## 설치

[**Releases**](https://github.com/matrixism-cmyk/NabiTermPub/releases)에서 받으세요.

- `nabiTerm-setup.exe` — 설치본(관리자 권한 불필요, per-user 설치)
- `nabiTerm-standalone.zip` — 포터블(압축 해제 후 실행, 설정을 exe 옆에 저장)

설치 후 **자동 업데이트**가 새 버전을 알리고 한 번에 적용합니다(도움말 ▸ 정보에서 수동 확인 가능).

### GPU 없는 VM · 헤드리스 환경

nabiTerm은 GPU(wgpu: DX12→Vulkan→GL)로 그립니다. **GPU가 전혀 없는 VM·헤드리스 서버**에서는 시작 시 렌더 가능 여부를 자동 점검합니다:

- **인터넷 연결 시**: "GPU가 감지되지 않습니다. 소프트웨어 렌더링 구성요소(약 22MB)를 받을까요?" 확인창이 뜨고, 동의하면 자동으로 받아(무결성 SHA256 검증) `nabiTerm.exe` 옆에 설치한 뒤 소프트웨어로 실행합니다. 한 번 받으면 다음부터는 바로 실행됩니다.
- **오프라인(폐쇄망) 시**: 별도 자산 **`nabiTerm-mesa-software-gl.zip`**(Mesa llvmpipe, ~22MB)을 받아 두 DLL을 `nabiTerm.exe` 옆에 직접 풀어 주세요.
- 환경변수 `NABI_RENDERER=software`로 소프트웨어 렌더를 강제, `NABI_RENDERER=hardware`로 자동 점검을 건너뛸 수 있습니다.

## 기술 스택

- **언어:** Rust (워크스페이스, 다수의 `nabi-*` 크레이트로 모듈화)
- **터미널 코어:** alacritty_terminal · **GUI:** egui / eframe (wgpu: DX12→Vulkan→GL, 소프트웨어 GL 폴백)
- **SSH/SFTP:** russh / russh-sftp · **로컬 PTY:** portable-pty(ConPTY)
- **플랫폼:** Windows 10 / 11 (x64)

## 라이선스

nabiTerm 자체는 **Apache License 2.0**으로 배포됩니다(루트 [`LICENSE`](LICENSE) 참조).

사용한 오픈소스 구성요소와 각 라이선스는 앱 내 **도움말 ▸ 오픈소스** 탭에 표기되어 있으며, 아래 [오픈소스 사용 고지](#오픈소스-사용-고지)에도 정리했습니다.

## 오픈소스 사용 고지

nabiTerm은 아래를 비롯한 오픈소스로 만들어졌습니다(제작자분들께 감사드립니다). 전이 의존성 660여 개는 대부분 MIT/Apache-2.0 허용형 라이선스입니다.

| 구성요소 | 용도 | 라이선스 |
|---|---|---|
| alacritty_terminal | 터미널/VT 코어 | Apache-2.0 |
| egui · eframe · epaint · egui_extras | GUI 프레임워크 | MIT OR Apache-2.0 |
| wgpu | GPU 렌더링 백엔드 | MIT OR Apache-2.0 |
| Mesa 3D (llvmpipe, 별도 자산) | 소프트웨어 OpenGL 폴백 | MIT |
| egui_dock | 도킹 탭 | MIT |
| epaint_default_fonts | 번들 UI 폰트 | OFL-1.1, UFL-1.0 |
| image | PNG/JPEG/GIF 디코드 | MIT OR Apache-2.0 |
| portable-pty | 로컬 PTY(ConPTY) | MIT |
| russh · russh-sftp | SSH / SFTP | Apache-2.0 |
| suppaftp | FTP | MIT OR Apache-2.0 |
| tokio | 비동기 런타임 | MIT |
| encoding_rs · chardetng | 텍스트 인코딩/감지 | Apache-2.0/MIT, BSD-3 |
| syntect · fancy-regex | 구문 강조 | MIT |
| ttf-parser | 폰트 열거 | MIT OR Apache-2.0 |
| memmap2 | 대용량 파일 뷰어 | MIT OR Apache-2.0 |
| arboard · rfd | 클립보드 · 파일 대화상자 | MIT (OR Apache-2.0) |
| argon2 · aes-gcm · zeroize | 볼트 암호화 | MIT OR Apache-2.0 |
| serde · serde_json · toml | 설정/직렬화 | MIT OR Apache-2.0 |
| chrono · directories | 시간 · 경로 | MIT OR Apache-2.0 |
| option-ext (directories 경유) | 전이 의존성 | MPL-2.0 |

---

<sub>🤖 nabiTerm 개발과 이 문서 작성에는 [Claude Code](https://claude.com/claude-code)의 도움을 받았습니다.</sub>
