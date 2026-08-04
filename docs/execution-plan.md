# 실행 계획 — 설치본(Phase 0) + 에이전트 제어 평면 업그레이드(CP-5~CP-8)

> **상태(2026-06-11): 전 단계 구현 완료.** Phase 0(설치본 3건 + setup.exe)과
> CP-5~CP-8 전부 구현·테스트 통과 — 상세는 agent-control-upgrade.md §4 참조.

← [업그레이드 설계](./agent-control-upgrade.md) · [v1 설계](./agent-control.md)

사용자 지시: **설치본 3건을 맨 먼저**, 이후 CP-5→CP-8 순서로 진행.

## Phase 0 — 설치본 (최우선)

| # | 항목 | 상태 | 내용 |
|---|------|------|------|
| 0-1 | installer 산출물 | **구현됨** | `cargo run -p xtask -- dist`: release 빌드 → `dist/stage/nabiTerm.exe` 스테이징 → `dist/nabiTerm-portable.zip`(portable.toml 동봉) → Inno Setup(ISCC) 있으면 `dist/nabiTerm-setup.exe` 컴파일(`installer/nabiTerm.iss`) |
| 0-2 | 개발본/설치본 구별 | **구현됨** | 설치본은 **nabiTerm.exe**, 개발본은 nabi.exe — 프로세스명이 달라 개발 중 정리가 설치본을 죽이지 않음. 개발 측 정리는 경로 필터(`$_.Path -like "*Desktop\nabi\target*"`)를 병행 |
| 0-3 | 콘솔(로그) 창 숨김 | **구현됨** | release는 `windows_subsystem="windows"`(GUI) — 파워셸 로그 창 없음. `nabi cli`는 AttachConsole(부모 콘솔)로 출력 유지. 디버그 빌드는 콘솔 유지(NABI_LOG 관찰) |
| 0-4 | 잔여 | 대기 | Inno Setup 6 설치(또는 포터블 zip만 배포), VERSIONINFO 사후 주입(xtask icon 확장 — ProductName "nabiTerm"), 버전 번호 체계(0.1.0~) 확정, 코드서명(후순위) |

빌드 환경 주의: 링크에 `~\mingw64\bin`이 PATH에 필요(libshlwapi.a). winres의 .rsrc는
GNU ld가 버리므로 아이콘은 **xtask icon**(UpdateResourceW 사후 주입)이 정본.

## Phase 1 — CP-5 정합성 (G1~G6)

| 순서 | 작업 | 파일 | 검증 |
|------|------|------|------|
| 1 | G5 파이프 DACL: 현재 사용자 SID 전용 보안 기술자 | nabi-control `pipe_acl.rs`(신규), server.rs | 다른 토큰 접속 거부 테스트(가능 범위), DACL 문자열 단위 테스트 |
| 2 | G1 스폰 seq 회신: `SpawnLocalPane{reply_seq}` + `PaneSpawned{seq}` 에코, 서버는 EventHub로 자기 seq 대기(폴링 제거) | nabi-proto command.rs/event.rs, orchestrator actor.rs, control dispatch.rs | 동시 스폰 2건 ID 정확성 roundtrip 테스트 |
| 3 | G3 wait 페이로드: exit code·CommandBlock(JSON)을 `data`로 회신 | control subscribe.rs | `wait --until exit`가 code 회신 테스트 |
| 4 | G4 정책 재분류: wait/tail을 read 그룹으로(ask 모드 무승인) | control policy.rs, server.rs | read 무승인 테스트 |
| 5 | G2 tail 델타: absolute line index 기반 신규 줄만 전송(damage 이벤트로 깨움) | nabi-vt grid(절대 줄 카운터 노출), control `tail_delta.rs`(신규) | 중복 0·유실 0 테스트(빠른 출력 시나리오) |
| 6 | G6 send paste 안전: bracketed paste 상태 조회 후 200~/201~ 래핑, `--raw` 우회 | control dispatch.rs, nabi-vt 모드 조회 | paste 모드 pane에 send → 래핑 확인 테스트 |

## Phase 2 — CP-6 에이전트 가시성 (G7~G9)

1. `pane_meta.rs`(orchestrator 신규): cwd(OSC 7)·실행 중 명령·마지막 exit·종류 캐시 + 상태머신(Idle/Working/Blocked/Exited, `state_since_ms`).
2. PaneInfo 확장(kind/cwd/state/last_exit/running_cmd — serde 기본값 하위호환).
3. CLI `--json`(모든 verb, ControlResponse 직렬화) — client.rs 출력 분기.
4. `--match "title:x,cwd:y,kind:z,state:idle"` 주소지정 — `matcher.rs`(신규), 0건/2건+ 모호성 거부.
5. 탭 UI 상태 배지(Working 스피너/Blocked 경고색 — 기존 activity 점 확장).
- 검증: `list --json` 필드 노출, match 모호성 거부 테스트, working→idle→exited 전이 테스트.

## Phase 3 — CP-7 동작 확장·정책 v2 (G10~G13)

1. `SpawnTerminal{dock: tab|split-right|split-down|new-window}` + `spawn --ssh <저장세션>`(볼트 경유).
2. 정책 v2: verb 3그룹(read/act/inject), 그룹별 off/ask/on, inject 별도 승인 집합, 설정 UI 승인 현황+revoke.
3. `focus`/`set-title`/`notify` verbs(AppCtl 확장; notify는 기존 OSC 9 토스트 경로 재사용, 발신 pane 표기).
- 검증: split-right 스폰 레이아웃, read 무승인/inject 별도 승인, 토스트 verb 표기.

## Phase 4 — CP-8 통합 (G14·G15 + capture 품질)

1. `nabi mcp`(stdio MCP 서버 = 제어 파이프 프록시; 도구는 verb 1:1) — `mcp.rs`(신규). Claude Code 등록: `claude mcp add nabiterm -- nabi.exe mcp`.
2. capture 범위(`--start/--end`)·`--escapes`(SGR 보존)·`alt_screen` 동봉.
3. OSC 7771 잔여(원격 pane in-band) — opt-in 유지, 최후순위.
- 검증: MCP로 spawn→send→capture 왕복, capture -e SGR 보존 테스트.

## 공통 원칙

- 각 Phase 종료마다: build + clippy + test + lines(위반 0) + 실행 검증 후 다음 Phase.
- 라인 게이트(250/400) — §5 모듈 매핑(업그레이드 문서) 준수.
- 단축키 신설 금지(팔레트/메뉴 노출), 비밀번호 평문 금지, OSC는 nabi-osc 중앙 처리.
