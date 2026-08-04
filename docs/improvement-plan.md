# nabi 개선 계획 — 최신 프로그램 대비 UI/UX·안정성·신기술 (2026-06)

벤치마크: **Windows Terminal / WezTerm / Ghostty / Kitty**(터미널 렌더링·기능),
**Warp**(블록·AI UX), **Termius / XPipe / MobaXterm**(원격 관리),
**WinSCP / FileZilla**(파일 전송).

## 0. 현재 위치(이미 확보)

- 탭/분할/줌(tmux식)·분리 OS 창, 세션 사이드바(MobaXterm식), 퀵커넥트 바(FileZilla식)
- SFTP/FTP 탐색기식 브라우저(컬럼·정렬·보기모드·선택·키보드 탐색·DnD 양방향·드롭 강조)
- OSC 7/9/52/133/777, 키워드 하이라이트(색 지정), 붙여넣기 위생/확인, URL·경로·localhost 링크
- 보안: Argon2id+AES-GCM 볼트, TOFU known_hosts, 포터블 저장(DPAPI 미사용)
- 안정성: 오케스트레이터 패닉 격리(catch_unwind), pane별 tracing, 연결 타임아웃+keepalive
- 제약(유지): 단일 프로세스 멀티스레드 / 새 단축키 자제 / 파일 250(소프트)·400(하드)줄

## 1. 격차 분석(최신 대비)

| 영역 | 최신 표준 | nabi 현재 | 격차 |
|---|---|---|---|
| 렌더링 | GPU(wgpu), 글리프 캐시, 리거처 | egui Painter(글로우), 셀별 text() | **큼** |
| 터미널 모델 | 견고한 스크롤백, 이미지(sixel/kitty) | vt100 0.15(스크롤백 언더플로 버그 1건 ignore) | **큼** |
| 셸 통합 | OSC 133 프롬프트 점프·블록 UX(Warp) | OSC 133 감지만(시각화 없음) | 중 |
| 하이퍼링크 | OSC 8 명시 링크 | 휴리스틱 링크만 | 중 |
| SSH | 자동 재접속, agent, 점프호스트, FIDO2 | 타임아웃/keepalive까지 | **큼** |
| 전송 | 재개(resume), 큐 관리 | 진행률·속도제한·취소 | 중 |
| 접근성 | 스크린리더(AccessKit) | 미적용 | 중 |
| 진단 | 파일 로그, 크래시 리포트 | 콘솔 tracing만 | 중 |
| AI | 명령 제안/오류 설명(Warp/WT) | 없음 | 선택 |

## 2. 단계별 계획

### P1 — 안정성 기반 다지기 (먼저)
1. **SSH 자동 재접속**: 연결 끊김 감지 → 토스트+탭 내 "재연결" 버튼 → 지수 백오프 자동 재시도(옵션).
   pane_origins의 SessionKind 재사용. 검증: 네트워크 차단 후 복귀 시나리오.
2. **파일 로깅**: tracing-appender로 `logs/nabi.log` 회전 로그(+ 패닉 훅에서 위치 안내 토스트).
   콘솔 없이 실행해도 사후 진단 가능. NABI_LOG 필터 유지.
3. **설정 원자적 저장**: config.toml 임시파일+rename(전원 차단 시 깨짐 방지), 손상 시 백업 로드.
4. **SFTP 전송 재개**: 부분 파일 존재 시 이어받기(오프셋 read/write). FileZilla 핵심 기능.
5. **vt100 교체 검토(스파이크)**: alacritty_terminal 또는 자작 스크롤백으로 언더플로 버그 근본 해결.
   ignore된 match_nav 테스트를 활성화하는 것이 완료 기준. (모델 경계는 TermModel로 이미 격리됨)

### P2 — 터미널 UX 현대화
1. **OSC 133 활용**: 이전/다음 프롬프트 점프(팔레트 노출), 실패 명령 거터 표시 — Warp 블록의 경량판.
2. **OSC 8 하이퍼링크**: nabi-osc에 파싱 추가(중앙 처리 원칙), 셀 속성으로 밑줄+클릭.
3. **셸 통합 스크립트**: PowerShell/bash용 OSC 133 프로필 스니펫 자동 설치 버튼(설정).
4. **테마 확장**: Windows Terminal/iTerm2 스킴 가져오기(JSON), 라이트 모드 점검.
5. **AccessKit 활성화**: eframe accesskit feature — 스크린리더 기본 지원.

### P3 — 신기술/차별화 (선택)
1. **wgpu 렌더러**: egui 업그레이드(0.29→최신)와 함께 eframe wgpu 백엔드 + 글리프 캐시.
   리스크 큼(렌더 전면 교체) → paint() 경계 뒤에서 단계 전환, gnu/MinGW 빌드 검증 필수.
2. **이미지 프로토콜**: sixel/iTerm2 inline images — P1-5(터미널 모델 교체) 이후에만.
3. **AI 보조(옵트인)**: 실패 명령 → 설명/수정 제안. 로컬 우선/키 설정식, 기본 꺼짐(프라이버시).
4. **에이전트 포워딩·FIDO2**: russh 기능 확인 후 인증 옵션 확대.

### 의존성 업그레이드 트랙(별도 PR, 한 번에 하나)
egui/eframe/egui_extras 0.29→최신, egui_dock 0.14→0.19+(레이아웃 직렬화 호환 확인),
portable-pty 0.8→0.9, russh 0.61→최신. 각 단계: 빌드+전 테스트+수동 스모크(gnu/MinGW).

## 2.5 따라잡기 트랙(분명히 뒤처지는 4개 영역 — 세부 단계)

### T1. 터미널 코어 교체 (vt100 → alacritty_terminal) — ✅ 완료(2026-06-10)
vt100 제거, alacritty_terminal 0.24 단일 코어. 전 테스트 + **match_nav 깊은 스크롤백 활성 통과**
(언더플로 버그 근본 해결). 모듈: grid(코어)/render(추출·색)/search(검색)/prompts(프롬프트 점프).
수동 스모크(vim/htop/less)는 사용자 확인 대기.
1. ✅ 스파이크 완료(2026-06-10): alacritty_terminal **0.24.2 gnu/MinGW 빌드 OK**,
   바이트→그리드 파싱 검증 테스트 통과(`nabi-vt --features backend-alacritty`, alac_spike.rs).
   0.26 출시됨 — 이식 시 0.26 기준 재검증.
2. TermModel ↔ alacritty Term 매핑(스파이크 조사 결과):
   | TermModel | alacritty 대응 |
   |---|---|
   | process(bytes) | vte ansi::Processor::advance(term, byte)(0.24는 바이트 단위) |
   | render_rows | term.grid() 인덱싱(Point{Line,Column}, Cell{c,fg,bg,flags}) |
   | resize | term.resize(TermSize) |
   | scroll_by/offset | grid().display_offset() + scroll_display(Scroll::Delta) |
   | app_cursor/bracketed/마우스 모드 | term.mode() (TermMode 비트플래그) |
   | title/벨 | EventListener 콜백(Event::Title/Bell) — VoidListener 대신 수집 리스너 |
   | 검색(scroll_to_*_match) | RegexSearch(코어 내장) — 자작 검색 제거 가능 |
   | row_wrapped | Cell flags의 WRAPLINE |
3. nabi-vt 내부를 feature flag로 이중화(`backend-alacritty`) — 경계는 이미 TermModel로 격리
4. 모드 플래그(DECCKM·2004·마우스), render_rows, 검색/스크롤 이식 → 테스트 통과
5. 기본 백엔드 전환 → vt100 제거. 이후 sixel/kitty 이미지·reflow의 기반 확보

### T2. GPU 렌더러
1. (저비용 선행) 현 Painter 최적화: 행 단위 문자열 배치·damage 기반 부분 재그리기
2. eframe glow→wgpu 백엔드 전환(gnu 빌드·드라이버 검증 필수)
3. 글리프 아틀라스 캐시 + cosmic-text/swash 셰이핑(리거처·폴백 품질)
4. vtebench 스루풋 측정으로 회귀 가드

### T3. 실전 견고성(검증량 보완 — 사용자 수를 자동화로 대체)
1. SSH 자동 재접속(끊김 감지→재연결 제안→지수 백오프 옵션) ← 최우선
2. 파일 로깅(tracing-appender 회전) + 패닉 훅 로그 → 사후 진단 가능
3. OSC/CSI 파서 fuzzing(cargo-fuzz), 인코딩 코퍼스(CJK/IME) 테스트
4. vttest/esctest 체크리스트 작성·통과 추적
5. CI(빌드+테스트+clippy, gnu 타깃) — **git 저장소 부재로 보류, repo 생성 시 즉시**

### T4. 생태계/배포
1. 포터블 zip + Inno Setup/cargo-wix 설치본, winget 매니페스트
2. 버전 체크→알림(자동 설치는 후순위), 코드 서명(후순위)
3. 사용자 문서(README·가이드·FAQ)
4. 스크립팅/플러그인은 장기 보류 — 기능 노출은 팔레트/메뉴 원칙으로 대응

## 3. 비목표(하지 않음)
- 멀티프로세스 분할(단일 프로세스 결정 유지 — docs/메모리 참조)
- **새 전역 단축키 추가 금지(확정)** — 이미 과다, 신규 기능은 팔레트/메뉴/컨텍스트 메뉴로만
- tmux 호환 서버/detach 데몬(필요 시 별도 결정)

## 4. 진행 원칙
라운드당 1기능 + 빌드/clippy/테스트/라인검사/재실행 검증. 기존 크레이트 중복 구현 금지(특히 nabi-osc).
