# 에이전트 제어 평면 (Agent Control Plane) — pane 스폰·모니터링·제어

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련: [아키텍처](./architecture.md) · [UI/윈도잉](./ui-windowing-rendering.md)

## 0. 목표

nabiTerm의 한 pane 안에서 실행 중인 프로세스(Claude Code 같은 CLI 에이전트, 또는 임의의 스크립트)가
**실행 중인 nabiTerm 인스턴스를 프로그램적으로 제어**할 수 있어야 한다. tmux `send-keys`/`capture-pane`,
`wezterm cli spawn`과 같은 위치의 기능.

유스케이스 (전부 pane 내부 셸에서 발화):

| # | 유스케이스 | 예시 |
|---|-----------|------|
| U1 | 새 터미널 pane 열기 | `nabi cli spawn --shell pwsh --cwd C:\proj` → 새 탭/분할 |
| U2 | 파일 브라우저(탐색기) pane 열기 | `nabi cli open-browser --path C:\proj` |
| U3 | SFTP/FTP 브라우저 pane 열기 | `nabi cli open-sftp --session "prod-web"` (저장 세션) |
| U4 | 다른 pane 모니터링 | `nabi cli capture --pane 3 --lines 50` → 화면/스크롤백 텍스트 |
| U5 | 다른 pane 상태 조회 | `nabi cli list` → pane 목록·제목·cwd·실행 중 명령·종료코드 |
| U6 | 다른 pane 제어(입력 주입) | `nabi cli send --pane 3 "cargo test`r"` |
| U7 | 이벤트 구독(폴링 없이 감시) | `nabi cli wait --pane 3 --until exit` → 명령/pane 종료 대기 |
| U8 | pane 닫기/리사이즈 | `nabi cli kill --pane 3` |

핵심 시나리오: 에이전트가 pane A에서 작업하면서 pane B에 빌드를 띄우고(U1+U6),
B의 출력과 종료코드를 관찰(U4·U5·U7)해 다음 행동을 결정한다.

## 1. 현재 상태 진단

**이미 있는 것 (어휘·상태는 거의 완비):**

- `nabi-proto::Command` — `SpawnLocalPane`(shell/cwd/encoding), `WriteInput`, `Resize`, `ClosePane`,
  `ConnectSsh`, `Sftp*` 전체. U1·U6·U8은 기존 variant 재사용으로 충분.
- `nabi-proto::Event` — `PaneSpawned`/`PaneExited`/`CommandStarted`/`CommandBlock`(OSC 133 명령 경계·exit code)/
  `CwdChanged`(OSC 7). U5·U7에 필요한 신호가 이미 흐르고 있다.
- `nabi-orchestrator::SharedPanes`(`Arc<RwLock<HashMap<PaneId, PaneView>>>`) — pane별 `TermModel`이
  `Arc<Mutex>`라 **임의 스레드에서 화면/스크롤백을 직접 읽을 수 있다**. U4는 Command 왕복 없이 구현 가능.

**없는 것 (이 문서가 채울 부분):**

1. **외부 입구 없음** — `main.rs`는 CLI 인자 파싱 없이 eframe 부트스트랩 직행. named pipe/IPC 서버 없음.
   pane 내부 프로세스가 Command 버스에 닿을 방법이 전무.
2. **자기 pane 식별 불가** — 스폰 시 환경변수 주입이 없어(`nabi-pty`에 env 코드 없음) 셸이 "내가 pane 몇 번인지" 모른다.
3. **앱 레벨 상태 분리** — 파일 브라우저·SFTP 탭, 도킹 레이아웃, 분리 창(`floating`)은 오케스트레이터가 아니라
   `nabi-app`(NabiApp) 소유. U2·U3과 "스폰된 pane을 도크에 올리기"는 **앱 레벨 메시지**가 필요하다.
4. **요청-응답 패턴 없음** — Command/Event는 fire-and-forget 버스. `list`/`capture`처럼 결과를 돌려줘야 하는
   질의(query)는 응답 경로가 필요하다.

## 2. 설계 — 제어 표면 2계층

### 2.1 주 채널: named pipe JSON 제어 서버 (out-of-band)

**`\\.\pipe\nabi-control-<PID>`** 에 newline-delimited JSON 요청/응답 서버를 연다.

- **out-of-band인 이유:** in-band(OSC)로는 응답을 pane *입력*으로 주입하는 수밖에 없는데, 이는 셸 입력 버퍼를
  오염시킨다(붙여넣기 주입 방지 3경로를 우회하는 꼴). 질의 응답·이벤트 스트림은 별도 파이프가 정도(正道).
- **클라이언트 = `nabi.exe` 자신:** `nabi.exe cli <verb> [...]` 서브커맨드가 파이프에 접속해 1요청-1응답(또는
  `wait`/`tail`은 스트림) 후 종료. 별도 바이너리 불필요, 배포 단일 파일 유지.
- **발견(discovery):** 스폰되는 모든 로컬 셸에 환경변수 주입:
  - `NABI_PANE_ID=<u64>` — 자기 pane ID (요청의 `from` 필드·기본 타깃 추론용)
  - `NABI_CONTROL_PIPE=\\.\pipe\nabi-control-<PID>` — 접속 대상
  - SSH 원격 pane은 주입 불가 → 원격에서는 제어 불가가 기본(보안상도 옳다. §4).

### 2.2 보조 채널: 커스텀 OSC (in-band, fire-and-forget 전용)

`OSC 7771 ; <verb> ; <json> ST` — 응답이 필요 없는 동작(U1 spawn, U2/U3 open, 알림)만 허용.
기존 `nabi-osc` 스캐너에 탭 하나 추가로 끝나고, **원격 SSH pane에서도 동작**한다는 것이 유일한 존재 이유.
질의 verb(`list`/`capture`)는 OSC로 받으면 무시하고 경고 로그. 원격에서 오는 spawn류는 기본 OFF(§4).

### 2.3 메시지 흐름

```
pane 내 에이전트
  │  nabi.exe cli spawn --shell pwsh
  ▼
named pipe ──▶ nabi-control 서버 스레드
                 │ ControlRequest 파싱·권한 검사(§4)
                 ├─ 질의(list/capture/status) ──▶ SharedPanes 직접 읽기 ──▶ JSON 응답
                 ├─ 터미널 동작(spawn/send/kill/resize) ──▶ 기존 cmd_tx(Command 버스) 재사용
                 ├─ 앱 동작(open-browser/open-sftp/dock/float) ──▶ ctrl_tx(신규 AppCtl 채널) ──▶ NabiApp::update()가 drain
                 └─ 구독(wait/tail) ──▶ event bus tap 등록, Event 도착 시 스트림으로 push
```

- **터미널 동작은 기존 버스 재사용**이 핵심: `cmd_tx.send(Command::SpawnLocalPane{..})`를 보내면
  기존 경로 그대로 `PaneSpawned`가 UI에 도착해 **자동으로 도크 탭에 올라간다**. 제어 평면이 UI 동기화를
  따로 할 필요가 없다 (메뉴·팔레트·제어서버가 같은 Command를 방출 = 기존 패리티 원칙 그대로).
- **앱 동작만 신규 채널**(`crossbeam_channel<AppCtl>`): 브라우저/SFTP 탭 생성, pane 도킹 위치 지정(탭/분할/새 창),
  분리 창 띄우기. `NabiApp::update()` 선두에서 drain — egui 프레임과 자연 동기화.

## 3. 프로토콜 어휘

### 3.1 ControlRequest (pipe 수신, serde JSON)

```rust
/// 제어 클라이언트 → 서버 요청. line-delimited JSON.
pub enum ControlRequest {
    /// U5: pane 목록·상태 스냅샷.
    ListPanes,
    /// U4: 화면(+스크롤백 lines줄) 텍스트 캡처.
    Capture { pane: u64, lines: u32 },
    /// U1: 새 터미널. dock 위치 지정(탭/오른쪽 분할/아래 분할/새 OS 창).
    SpawnTerminal { shell: String, cwd: Option<String>, dock: DockTarget },
    /// U2: 로컬 파일 브라우저 탭.
    OpenBrowser { path: Option<String>, dock: DockTarget },
    /// U3: SFTP/FTP 브라우저. 저장 세션 이름 참조(자격증명은 볼트에서 — 평문 금지 §4).
    OpenSftp { session: String, dock: DockTarget },
    /// U6: 입력 주입(바이트 그대로; \r 포함 여부는 호출자 책임).
    SendInput { pane: u64, data: String },
    /// U8.
    ClosePane { pane: u64 },
    Resize { pane: u64, cols: u16, rows: u16 },
    /// U7: 조건 충족까지 블록(스트림 응답). until: "exit" | "command-done" | "idle" | "output".
    Wait { pane: u64, until: WaitCond, timeout_ms: u64 },
    /// U7 변형: pane 출력을 실시간 스트림(헤드리스 tail -f).
    Tail { pane: u64 },
}

pub enum DockTarget { Tab, SplitRight, SplitDown, NewWindow }
```

### 3.2 ControlResponse

```rust
pub enum ControlResponse {
    Panes(Vec<PaneInfo>),       // id·제목·종류(local/ssh/browser/sftp)·cwd·실행중 명령·마지막 exit code·크기
    Captured { pane: u64, text: String, cursor: (u16, u16) },
    Spawned { pane: u64 },      // 새 pane ID 즉시 회신 → 후속 send/capture 타깃으로 사용
    Ok,
    Err { code: ErrCode, message: String },   // Denied / NoSuchPane / BadRequest / Timeout
    Event { pane: u64, kind: String, data: String },  // Wait/Tail 스트림 항목
}
```

`PaneInfo`의 cwd·실행중 명령·exit code는 기존 OSC 7/133 추적 결과(`CwdChanged`/`CommandBlock`)를
오케스트레이터가 pane별로 최신값 캐시해 제공(현재 UI 상태바가 쓰는 것과 동일 출처).

### 3.3 신규 Event (구독용 보강)

기존 `Event`로 충분하나, `Wait{until: Idle}`을 위해 "출력이 N ms 잠잠"은 서버 측 타이머로 구현(프로토콜 추가 없음).

## 4. 보안 모델 (입력 주입은 위험 — 기본 보수적)

| 위협 | 대책 |
|------|------|
| 타 사용자/프로세스의 파이프 접속 | named pipe ACL을 **현재 사용자 SID 전용**으로 생성 + 접속 시 토큰 검증(스폰 때 `NABI_CONTROL_TOKEN` 주입, 첫 요청에 에코) |
| 원격(SSH) 출력이 OSC 7771로 spawn/제어 시도 | OSC 채널은 **로컬 pane만 + 설정 opt-in**(`[control] allow_osc = false` 기본). 원격 pane 발 OSC 7771은 무조건 무시·로그 |
| 에이전트의 무제한 입력 주입 | 설정 3단계: `off`(서버 안 띄움) / `ask`(pane당 최초 제어 시 토스트 승인) / `on`. 기본 `ask` |
| 자격증명 노출 | `OpenSftp`는 저장 세션 이름만 받는다. 비밀번호/키를 제어 채널로 받지 않음(볼트 잠금 상태면 UI 잠금해제 다이얼로그 경유) |
| 감사 | 모든 제어 요청을 tracing에 `control` span으로 기록(요청자 pane, verb, 타깃) — 인-앱 로그 패널에서 열람 |

추가 안전: `SendInput`은 bracketed-paste 모드인 타깃에는 200~/201~ 래핑 없이 raw로 넣지 않는다(기존
붙여넣기 주입 방지 경로와 동일 처리를 통과).

## 5. 크레이트/모듈 매핑 (라인 게이트 250/400 고려)

| 위치 | 신규 모듈 | 책임 |
|------|----------|------|
| **nabi-control (신규 크레이트)** | `protocol.rs` | ControlRequest/Response serde 타입 (어휘만, I/O 없음) |
| | `server.rs` | named pipe 리스너(스레드)·접속 수락·토큰 검증 |
| | `dispatch.rs` | 요청→{SharedPanes 읽기, cmd_tx, ctrl_tx} 라우팅·권한 검사 |
| | `subscribe.rs` | Wait/Tail용 event tap·타임아웃 |
| | `client.rs` | CLI 쪽 파이프 접속·요청/응답 (nabi.exe cli가 사용) |
| **nabi-proto** | `appctl.rs` | `AppCtl` enum(OpenBrowser/OpenSftp/Dock 지정) — 앱 레벨 어휘 |
| **nabi-pty** | `spawn.rs` 확장 | CommandBuilder에 `NABI_PANE_ID`/`NABI_CONTROL_PIPE`/`NABI_CONTROL_TOKEN` env 주입 |
| **nabi-osc** | `scanner.rs` 확장 | OSC 7771 verb 파싱 → `OscEvent::Control` |
| **nabi-orchestrator** | `pane_meta.rs` | pane별 cwd/명령/exit code 최신값 캐시(ListPanes 응답 출처) |
| **nabi-app** | `main.rs`/`cli.rs` | argv[1]=="cli"면 클라이언트 모드로 분기(eframe 안 띄움), 아니면 서버 와이어링 |
| | `ctlapply.rs` | `update()` 선두에서 ctrl_rx drain → 브라우저/SFTP 탭 생성·도킹 적용 |
| | `settings` 확장 | `[control] mode = off/ask/on`, `allow_osc` 설정 UI |

의존 방향: `nabi-control → {nabi-proto, nabi-types, nabi-log}` 만. 오케스트레이터·앱이 nabi-control의
server를 와이어링하므로 순환 없음. 클라이언트 모드는 `nabi-control::client`만 사용(egui 미초기화 — 콘솔 출력).

> Windows 콘솔 주의: nabi.exe가 GUI 서브시스템이면 `cli` 모드의 stdout이 안 보인다.
> `AttachConsole(ATTACH_PARENT_PROCESS)` 호출 또는 경량 `nabi-cli.exe` 별도 bin 중 택일 — 구현 시 결정.

## 6. 구현 마일스톤

| 단계 | 범위 | 검증 |
|------|------|------|
| **CP-1 골격** ✅ | nabi-control 크레이트(protocol/server/client), env 주입, `nabi cli list`/`capture` (읽기 전용) | 파이프 왕복 테스트 + 라이브 E2E 완료(2026-06-11) |
| **CP-2 스폰·제어** ✅ | `spawn`(새 pane ID 회신)/`send`/`kill`/`resize`, 권한 모드 off/ask/on, 감사 로그 | spawn→send→capture 라이브 E2E 완료 |
| **CP-3 앱 동작** ✅ | AppCtl 채널, `open-browser`/`open-sftp`(저장 세션) | E2E open-browser 완료 |
| **CP-4 구독·OSC** ✅ | `wait`(exit/command-done/idle/output)/`tail` 스트림, OSC 7771(opt-in `control_allow_osc`), 설정 UI·다국어 | wait until=output/exit 라이브 E2E 완료 |

**구현 완료(2026-06-11).** 핵심 시나리오(에이전트가 옆 pane에 빌드 띄우고 wait로 종료 관찰) 동작 확인.
부수 개선: 셸 자연 종료 감지(nabi-pty `spawn_child_waiter`로 child.wait — ConPTY는 마스터 살아있으면
reader EOF 안 옴), 종료 시 레지스트리 중앙 정리(actor.rs exit_rx → state+panes 제거, 좀비 list 방지).

## 7. 비범위 (이번 설계에서 제외)

- 원격(SSH) pane 쪽에서의 제어 발화 — 보안상 기본 차단, 필요 시 별도 설계.
- 다중 nabiTerm 인스턴스 간 제어(파이프 이름이 PID 스코프 — 자기 인스턴스만).
- 플러그인 API(M4)와의 통합 — ControlRequest 어휘를 플러그인 호스트 API가 재사용할 수 있게 nabi-control을
  어휘/전송 분리해 두는 것까지만 고려.
