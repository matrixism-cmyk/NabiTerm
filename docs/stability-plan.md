# 안정성 트랙 상세 계획 (초안 — 함께 다듬는 중)

작성 2026-06-13. 범위: **① SSH 끊김 복원, ② 파서 견고성, ③ 제어 평면 실사용**.
구현 전 계획 단계 — 각 절 끝의 **[결정 필요]**를 함께 확정한 뒤 착수한다.

진행 원칙(유지): 라운드당 1기능 + 빌드/clippy/테스트/라인(250·400)/재실행 검증.
새 전역 단축키 금지(팔레트/메뉴 노출). 단일 프로세스. OSC는 nabi-osc 중앙 처리.

---

## ① SSH 끊김 복원

### 현재 상태(근거)
- `nabi-ssh/src/session.rs:18` `connect(.., on_close: Box<dyn FnOnce(Option<String>)>)` —
  세션 종료 시 1회 호출(None=정상, Some=오류).
- `spawn_pane.rs`의 on_close: None→`PaneExited`, Some→`SshDisconnected{pane,message}`.
- `events.rs:121` `SshDisconnected` → `reconnect_ask = Some((pane,msg))`.
- `reconnect.rs:8` 수동 [다시 연결] 창 1개. **재연결 시 `ClosePane` + `connect_saved`로
  새 pane 생성** → 스크롤백·pane ID·탭 위치·분할 레이아웃 소실(reconnect.rs:36-43).

### 격차
1. 자동 재시도(지수 백오프) 없음 — 일시적 끊김도 매번 수동 클릭.
2. 재연결이 pane을 바꿔치기 → 히스토리/위치 상실(에이전트 제어 타깃 ID도 바뀜).
3. 끊김 분류 없음 — 인증 실패/호스트키 변경(영구 실패)도 재시도 대상이 될 위험.

### 제안 설계
**(A) 제자리 재연결(in-place)** — 같은 PaneId 유지가 핵심.
- 신규 `Command::ReconnectSsh{ pane, params }`: 오케스트레이터가 해당 pane의
  `PaneRuntime.transport`만 새 SshChannel로 **교체**(모델·스크롤백·레지스트리 엔트리 유지).
- 모델에는 재연결 구분선 한 줄 주입(예: `── 재연결됨 HH:MM ──`), 스크롤백 보존.
- 분할/탭 위치는 dock이 PaneId로 들고 있으므로 자동 유지.

**(B) 끊김 분류** — 재시도 가능 여부 판정.
- `on_close`의 메시지/오류 종류를 enum화: `Transient`(네트워크·타임아웃·EOF) /
  `Auth`(인증 거부) / `HostKey`(known_hosts 불일치) / `Unknown`.
- Transient만 자동 재시도. Auth/HostKey는 즉시 수동 창(자동 금지 — 보안/무한루프 방지).

**(C) 백오프 정책**
- 지수 백오프: 1s→2s→4s→8s→… 상한 30s, 최대 N회(기본 5, 0=무한은 옵션).
- 각 시도 사이 토스트+탭 배지("재연결 시도 3/5… 4s 후"), [지금 재시도]/[중단] 버튼.
- 성공 시 토스트 해제 + 구분선. 소진 시 기존 수동 창으로 폴백.
- 상태는 앱 측 `reconnect: HashMap<PaneId, ReconnectState{attempt, next_at, params}>`.

**(D) 자격증명 재사용**
- `pane_origins[pane]`의 SessionKind(볼트 credential_ref) 재사용 — 평문 보관 금지.
- 볼트 잠금 시: 자동 재시도 불가 → 수동 창(현행 Quick Connect 프리필 폴백) 유지.

### 검증
- in-process echo 서버(기존 echo_test.rs 패턴)로 **연결 강제 종료→자동 재연결→입력 지속** 왕복.
- 분류 테스트: Auth 실패는 자동 재시도 안 함(시도 횟수 0) 단위 테스트.
- 수동 시나리오: 실 SSH 끊고(랜선/Wi‑Fi) 복귀 시 같은 탭에서 스크롤백 유지 확인.

### [결정 필요]
- **D1. 제자리 재연결 vs 새 pane**: 제자리(transport 교체, 권장)가 히스토리·제어 ID를
  보존하지만 오케스트레이터에 신규 Command·transport 교체 경로가 필요. 단순히 현행
  새-pane 방식 위에 백오프만 얹는 저비용안도 가능(단 히스토리 상실 유지).
- **D2. 기본 동작**: 자동 재시도 기본 ON(옵트아웃) vs 기본 OFF(토스트의 [자동 재시도]
  버튼으로 옵트인). 보수적으로는 OFF 시작.
- **D3. 횟수/상한**: 기본 5회·상한 30s가 적절한지, 무한 옵션 노출 여부.

---

## ② 파서 견고성

### 현재 상태(근거)
- 자작 파싱 영역은 **OSC 스캐너뿐**: `nabi-osc/src/scanner.rs`(손으로 짠 상태기계,
  ESC] … BEL|ST, 4096B 폭주 가드) + `oscparse.rs`(본문 파싱 131줄) + `base64.rs`(OSC 52).
- CSI/이스케이프/그리드 전반은 **alacritty_terminal**(검증된 상류)이 처리 — 자작 아님.
- 인코딩: `pane_registry.rs decoder_for`(encoding_rs 스트리밍, EUC-KR/Shift_JIS/GBK/…).
- 기존 테스트: scanner 단위 7종(OSC 7/9/52/133/777/1337/progress). 퍼징·코퍼스 없음.

### 격차
1. OSC 스캐너가 비정상 입력(잘린 시퀀스, 거대 페이로드, 불완전 UTF-8, 중첩 ESC,
   분할 청크 경계)에 대한 **체계적 테스트 부재** — 손으로 짠 코드라 가장 취약.
2. 인코딩 디코더 경계(부분 멀티바이트가 청크 경계에 걸칠 때) 회귀 테스트 부재.
3. 표준 적합성(vttest/esctest) 추적 없음 — alacritty가 처리하나 우리 파이프라인
   (route_output→model→osc) 전체로는 미검증.

### 제안 설계
**(A) OSC 스캐너 속성 기반 테스트** — 이 환경(gnu/MinGW, nightly 불확실)에 맞춤.
- cargo-fuzz(libfuzzer)는 nightly+클랭 의존 → **이 환경에서 불안정**. 대신
  **proptest/arbitrary**(stable) 채택: 임의 바이트열을 먹여도 (a) 패닉 없음,
  (b) 버퍼 상한 준수, (c) 동일 입력을 1바이트씩 분할해 먹여도 결과 동일(청크 불변).
- 시드 코퍼스: 잘린 OSC(`ESC]7;`만), 4096B 초과 페이로드, 비UTF-8 본문,
  ESC가 본문 중간에 박힌 경우, BEL/ST 혼용.
- **불변식 검증 하니스**(라운드별 1파일): `nabi-osc/tests/fuzz_like.rs`.

**(B) 인코딩 경계 코퍼스**
- `nabi-orchestrator` 또는 `nabi-vt` 테스트: CJK 문자열을 바이트 N개씩 쪼개 디코더에
  순차 주입 → 합치면 원문과 동일(부분 시퀀스 보존) 확인. EUC-KR/Shift_JIS/GBK 각 1케이스.

**(C) vttest/esctest 체크리스트(문서 추적)**
- 자동화 대신 **수동 체크리스트 문서**(`docs/vttest-checklist.md`)로 시작:
  커서 이동/SGR/스크롤 영역/대체 화면/줄바꿈/탭 — 항목별 통과 여부 기록.
- 회귀 가드는 핵심 시나리오만 골라 process()→render_rows() 스냅샷 단위 테스트로.

### 검증
- `cargo test -p nabi-osc`에 속성 테스트 추가(수천 케이스, 패닉 0).
- 청크 불변(1바이트 분할 == 통짜) 테스트 통과.
- 인코딩 경계 라운드트립 통과.

### [결정 필요]
- **P1. 퍼징 도구**: proptest(stable, 권장) vs cargo-fuzz(커버리지↑이나 nightly/clang
  필요 — 이 환경에서 빌드 검증 부담). 우선 proptest, 후일 CI에서 cargo-fuzz 별도?
- **P2. vttest 범위**: 수동 체크리스트 문서만 vs 핵심 시퀀스 스냅샷 자동 테스트까지.
- **P3. 우선순위**: OSC 스캐너(자작·최고 위험) 먼저 → 인코딩 → 표준 적합성 순서 동의?

---

## ③ 제어 평면 실사용

### 현재 상태(근거)
- CP-1~CP-8 구현·**합성 E2E 검증 완료**(list/spawn/send/wait/capture/focus/notify/MCP
  핸드셰이크). 정책 v2(read/act/inject 그룹), MCP 12도구, paste-safe send, tail 델타.
- 검증은 **PowerShell 스크립트로 명령을 흉내** — 실제 에이전트가 장시간 운전한 적 없음.

### 격차(실사용에서만 드러나는 것)
1. **실 에이전트 워크플로 미검증**: Claude Code가 MCP로 spawn→작업 관찰→입력 주입을
   장시간 돌릴 때의 지연·오류 메시지 품질·정책 마찰(ask 모드 승인 빈도).
2. **MCP 등록 흐름 문서/검증**: `claude mcp add nabiterm -- nabi.exe mcp`가 실제로
   환경변수(NABI_CONTROL_*)를 상속받는지 — pane 밖에서 등록 시 동작?
3. **누락 가능 verb**: 실사용에서 자주 쓰는데 없는 동작(예: 특정 줄까지 대기,
   여러 pane 동시 캡처, 명령 완료+출력 동시 회신) 식별.
4. **에러 회복**: pane이 작업 도중 죽었을 때 wait/tail/capture의 응답 명확성.

### 제안 설계 — "관찰 우선, 코드는 그다음"
**(A) 실 에이전트 드라이브 세션(수동·기록)**
- nabiTerm 안에서 Claude Code(또는 다른 에이전트)를 띄우고 MCP로 옆 pane 운전:
  빌드 돌리고→wait→capture→오류 시 send로 수정 같은 **실제 루프**를 1~2회 수행.
- 마찰점·이상 응답·지연을 `docs/control-plane-fieldnotes.md`에 기록(코드 변경 전 관찰).

**(B) MCP 등록 경로 확정**
- pane 안에서 `claude mcp add` 시 env 상속 확인 / pane 밖(전역 등록) 시 디스커버리
  방법 설계(실행 중 인스턴스의 파이프·토큰을 어디서 읽나 — 파일 드롭? 환경?).
- README에 등록 1줄 + 트러블슈팅.

**(C) 관찰에서 나온 갭만 구현**
- 필드노트에서 확인된 진짜 필요만 verb/응답에 추가(추측 기능 금지).
- 후보(검증 후 판단): `wait --until "line:<regex>"`, 다중 pane 캡처, capture에
  명령 블록 경계 메타 동봉.

### 검증
- 실 에이전트가 spawn→빌드→wait(exit code)→capture→send 루프를 **외부 개입 없이** 완주.
- ask 모드에서 승인 UX가 작업을 막지 않는지(그룹 1회 승인으로 충분한지) 확인.
- MCP 등록을 새 셸에서 재현.

### [결정 필요]
- **C1. 어떤 에이전트로 드라이브?** Claude Code(MCP 네이티브) 기준이면 등록·운전이
  자연스러움. 다른 대상도 볼지.
- **C2. 필드노트 먼저 vs 바로 후보 verb 구현**: 관찰 1회 후 갭 확정(권장) vs 예상
  갭(line 대기·다중 캡처)을 선제 구현.
- **C3. 전역 MCP 등록 지원 여부**: pane 안 등록만 지원(단순) vs pane 밖에서도
  실행 중 인스턴스 발견(파일 기반 디스커버리 — 복잡·보안 검토 필요).

---

## 제안 순서(초안)

1. **②-A OSC 스캐너 속성 테스트** — 가장 싸고(코드 적음) 자작 코드 위험 직접 감소.
2. **①-A/B/C 제자리 재연결 + 분류 + 백오프** — 체감 큰 안정성, 아키텍처 손댐.
3. **③-A/B 실사용 관찰 + MCP 등록 확정** — 코드 적고 방향 정보 큼(이후 갭 구현).
4. ②-B 인코딩 코퍼스 / ②-C vttest 체크리스트 — 마무리·회귀 가드.

각 단계 독립 배포 가능. ①은 D1(제자리 vs 새 pane)이 비용을 가르는 분기.

## 미해결 결정 요약(함께 확정)
- D1 제자리 재연결 채택? · D2 자동 기본 ON/OFF · D3 백오프 횟수/상한
- P1 proptest vs cargo-fuzz · P2 vttest 범위 · P3 파서 우선순위
- C1 드라이브 에이전트 · C2 관찰 먼저 vs 선제 구현 · C3 전역 등록 지원
- 전체 **착수 순서**(위 1→4) 동의 여부
