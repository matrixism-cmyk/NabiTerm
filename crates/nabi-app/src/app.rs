//! eframe 앱 상태 + 업데이트 루프.

use nabi_orchestrator::OrchestratorHandle;
use nabi_types::{GridSize, PaneId};
use nabi_vt::Theme;
use std::collections::HashMap;

/// 복원 중 다음 스폰에 실어 보낼 컨텍스트: (레이아웃 ordinal, 로컬 백로그, 분리창 기하[x,y,w,h]).
pub type SpawnCtx = (Option<usize>, Option<Vec<u8>>, Option<[f32; 4]>);
pub type SnippetPrompt = (PaneId, String, Vec<(String, String)>); // 대화형 스니펫 입력(D6)

/// nabi 앱 상태. 탭/분할은 egui_dock DockState가, pane 상태는 오케스트레이터가 소유.
pub struct NabiApp {
    pub orch: OrchestratorHandle,
    pub dock: egui_dock::DockState<PaneId>,
    pub last_grid: HashMap<PaneId, GridSize>,
    pub font_size: f32,
    pub theme: Theme,
    pub lang: nabi_i18n::Lang,
    pub quick_connect: crate::connect::QuickConnect,
    /// AI 터미널 프로필 관리 독립창 열림(세션▸새 AI 터미널▸프로필 관리 — aiprofileui.rs).
    pub ai_prof_open: bool,
    /// 시작 스플래시가 뜬 시각. None이면 이미 걷혔거나 설정에서 껐다는 뜻(splash.rs).
    pub splash_since: Option<std::time::Instant>,
    /// 지난 실행이 비정상 종료라 되살릴 수 있는 미저장 문서들(padrecover). 비면 물어보지 않는다.
    pub pad_recover: Vec<crate::padrecover::Recovered>,
    /// 진단 로그 보기 창(Some=열림). 안쪽 None은 '로그 파일이 없다'는 뜻이다.
    pub log_view: Option<Option<crate::logview::LogText>>,
    /// 환경 관리자 창(도구 메뉴). 열려 있을 때만 Some.
    pub env_mgr: Option<crate::envmgr::EnvMgr>,
    /// 명령 기록 창이 열려 있는가(도구 메뉴·팔레트).
    pub cmd_hist_open: bool,
    /// 원격 파일 미리보기 창(SFTP 우클릭).
    pub preview: Option<crate::sftppreviewui::PreviewState>,
    /// '모든 창에서 찾기' 상태(Some=열림).
    pub find_all: Option<crate::findallui::FindAll>,
    /// 위험 명령 확인 대기 — 붙잡힌 입력(guard.rs).
    pub pending_send: Option<crate::guard::PendingSend>,
    /// 밖에서 바뀐 파일을 덮어쓰기 전에 묻는 중인 문서.
    pub editor_conflict: Option<nabi_types::PaneId>,
    /// 업데이트 뒤 첫 실행의 '새로워진 점'(Some=열림, 안쪽 None=노트를 못 챙김).
    pub whatsnew: Option<Option<String>>,
    /// 시작 시간 계측 시계 — 첫 프레임에서 기록하고 비운다(boottime).
    pub boot: Option<crate::boottime::Boot>,
    /// 파일 속성 창(Some=열림).
    pub file_props: Option<crate::fileprops::Props>,
    /// 가져오기 한 화면(Some=열림, 훑어 본 결과).
    pub import_screen: Option<Vec<crate::importscan::Source>>,
    /// 곁 스레드 해시 결과 통로(경로, 해시).
    pub hash_tx: std::sync::mpsc::Sender<(std::path::PathBuf, String)>,
    pub hash_rx: std::sync::mpsc::Receiver<(std::path::PathBuf, String)>,
    /// 프로필 창을 열 때 찍어 두는 원본 — '취소'로 되돌리기 위한 것. 창이 닫히면 비운다.
    pub ai_prof_backup: Option<Vec<nabi_config::AiProfileCfg>>,
    /// pane별 AI 명령 바에서 고른 모델·노력(버튼에 현재 상태 표시 — aicmdbar.rs).
    pub ai_picks: std::collections::HashMap<PaneId, crate::aicmdbar::AiPicks>,
    /// pane별 AI 화면 판독 캐시(모드·모델·노력·제목 — aimode.rs).
    pub ai_screen: std::collections::HashMap<PaneId, crate::aimode::AiScreen>,
    /// pane별 영문 팁 감지 결과(번역 오버레이 — tipoverlay.rs).
    pub tip_cache: std::collections::HashMap<PaneId, crate::tipoverlay::TipHit>,
    /// 팁 AI 번역 캐시(선택 기능, 디스크 영속).
    pub tip_ai: crate::tipai::TipAi,
    /// 명령 바에서 방금 고른 값 신호 ("model"|"effort", 값) — view가 설정에 저장한다
    /// (뷰어가 config를 가변 차용할 수 없어 out-param으로 넘긴다).
    pub ai_pick_out: Option<(String, String)>,
    /// 인코딩 제안 메모이즈 (pane, render_gen, 현재 인코딩, 결과) — 매 프레임 화면 60줄
    /// 덤프를 피한다(성능 리뷰 2026-08-19, encsuggest.rs).
    pub enc_cache: Option<(PaneId, u64, String, Option<&'static str>)>,
    /// 디렉터리 비교 맵을 마지막으로 읽은 시각(0.5초 스로틀 — editsftp.rs).
    pub compare_at: Option<std::time::Instant>,
    pub forward: crate::forwardui::ForwardForm,
    pub sftp: crate::sftppanel::SftpPanel, // 현재 활성(포커스) 원격 패널.
    pub sftp_pane: Option<nabi_types::PaneId>, // 활성 원격 패널의 도킹 PaneId.
    /// 비활성(배경) 원격 패널들(PaneId별) — 멀티 탭 지원. 활성은 self.sftp.
    pub sftp_bg: std::collections::HashMap<nabi_types::PaneId, crate::sftppanel::SftpPanel>,
    /// 원격 연결/탭 전역 카운터(연결 id + 고유 PaneId 파생).
    pub sftp_seq: u64,
    /// 전송 큐 항목 식별자 발급기(단조 증가). 진행률·완료 이벤트가 이 값으로 항목을 지목한다.
    pub xfer_seq: u64,
    /// 편집: 외부 편집기 감시 목록(다운로드→재업로드) + 내장 텍스트 에디터(인앱).
    /// 편집: 외부 편집기 감시 목록 + 내장 에디터 문서들(PaneId별 도크 탭).
    pub edits: Vec<crate::editsftp::EditWatch>, pub editors: HashMap<PaneId, crate::editor::EditorDoc>,
    /// 디렉터리 비교 모드(로컬↔원격 차이 색칠).
    pub compare_on: bool,
    /// 동기 브라우징(로컬·원격 함께 이동) + 토글 시 캡처한 루트.
    pub sync_browse: bool,
    pub sync_local_root: std::path::PathBuf,
    pub sync_remote_root: String,
    pub sessions: nabi_session::SessionTree,
    pub session_path: std::path::PathBuf,
    /// 로컬 파일 브라우저 상태(사이드패널 + 탭 공유).
    pub browser: crate::browserpanel::BrowserPanel,
    /// 탭으로 열린 브라우저들(PaneId→독립 상태) — 그룹마다 여러 개 가능.
    pub browser_tabs: HashMap<PaneId, crate::browserpanel::BrowserPanel>,
    pub sidebar_filter: String, // 세션 사이드바 필터.
    /// SSH 끊김 재연결 제안(끊긴 pane) / 미지 호스트키 TOFU 모달(id·host·port·algo·fp).
    pub reconnect_ask: Option<(PaneId, String)>, pub hostkey_prompt: Option<crate::hostkeyui::HostKeyAsk>,
    /// 탭바 빈 공간 우클릭 메뉴 위치(Some=표시 중).
    /// 탭바 우클릭 메뉴 위치 + 터미널 링크 길게 누름 메뉴((URL, 위치)).
    /// 마지막 필드: 인라인 이미지(Sixel) 텍스처 캐시(이미지 id → egui 텍스처).
    pub tabbar_menu: Option<egui::Pos2>, pub link_menu: Option<(String, egui::Pos2)>, pub floating_link: Option<(String, egui::Pos2)>, pub img_textures: std::collections::HashMap<u64, egui::TextureHandle>,
    /// 현재 창 크기(매 프레임 추적, 종료 시 config 저장).
    pub last_win: (f32, f32),
    /// 마지막으로 본 창 화면 위치(x, y). 종료 시 설정에 저장한다(winpos).
    pub last_pos: Option<(f32, f32)>,
    /// 창을 앞으로 가져와야 하는가(탐색기 '여기서 열기' 등 밖에서 부른 경우).
    pub raise_window: bool,
    /// 에이전트 제어 평면 권한 정책(서버와 공유) + 승인 요청 수신.
    pub control_policy: nabi_control::policy::ControlPolicy,
    pub control_ask_rx: crossbeam_channel::Receiver<(u64, nabi_control::policy::Group)>,
    /// 승인 대기 중인 (요청자 pane, verb 그룹) — 그룹별 별도 승인(CP-7).
    pub control_pending: Option<(u64, nabi_control::policy::Group)>,
    /// 다음 PaneSpawned를 별도 OS 창으로(제어 dock=new-window).
    pub control_float: bool,
    /// 제어 평면 앱 동작(브라우저/SFTP 탭) 수신 + 이벤트 fan-out 허브.
    pub control_app_rx: crossbeam_channel::Receiver<nabi_proto::AppCtl>,
    pub control_events: nabi_control::subscribe::EventHub,
    /// 미완료 스폰의 출처/명령/백로그/레이아웃 ordinal을 seq로 보관(PaneSpawned.seq로 매핑 — 순서 무관).
    pub pending_spawns: HashMap<u64, crate::workspace::PendingSpawn>,
    /// 앱 발급 스폰 seq(제어평면 저범위와 분리하려 고범위에서 시작).
    /// 복원 중 다음 스폰의 (레이아웃 ordinal, 로컬 백로그, 분리창 기하[x,y,w,h]).
    pub next_spawn_seq: u64, pub spawn_ctx: Option<SpawnCtx>,
    pub config: nabi_config::AppConfig, pub config_path: std::path::PathBuf,
    /// nabiPad 설정(터미널과 분리된 nabipad.toml) + 그 경로.
    pub editor_config: nabi_config::EditorConfig, pub editor_config_path: std::path::PathBuf,
    /// settings_open=메인 설정 창. editor_settings_for=nabiPad 자체 설정 창을 연 pane(분리 창이면 그 vctx에 렌더).
    pub settings_open: bool, pub editor_settings_for: Option<PaneId>,
    /// 설정 창 열릴 때의 config·editor_config 스냅샷(취소 시 되돌리기 — 실시간 미리보기용).
    pub settings_backup: Option<nabi_config::AppConfig>, pub settings_editor_backup: Option<nabi_config::EditorConfig>,
    /// nabiPad 설정 창을 열 때 찍어 두는 원본 — '취소'로 되돌리기 위한 것(주 설정과 같은 방식).
    pub editor_settings_backup: Option<nabi_config::EditorConfig>,
    /// 실시간 미리보기에서 마지막으로 적용한 글꼴 경로(바뀔 때만 폰트 재설치).
    pub settings_live_font: String,
    /// floating=분리 OS 창 pane, floating_geom=그 창 위치·크기[x,y,w,h](복원, P10),
    /// floating_shown=기하를 이미 적용한 pane(첫 프레임에만 적용해 창이 계속 커지는 것 방지),
    /// docked_float="창 안에 띄우기" 메인 창 내 오버레이 pane(egui_dock Eject 대체, 닫으면 재도킹).
    pub floating: Vec<PaneId>, pub floating_geom: HashMap<PaneId, [f32; 4]>, pub floating_shown: std::collections::HashSet<PaneId>, pub docked_float: Vec<PaneId>,
    pub close_signal: std::sync::Arc<std::sync::Mutex<Vec<PaneId>>>,
    pub floating_grid: std::sync::Arc<std::sync::Mutex<HashMap<PaneId, GridSize>>>,
    pub vault: Option<nabi_secret::Vault>,
    pub vault_password: Option<String>,
    /// vault 경로 + 알려진 호스트(known_hosts) 관리 대화상자(C3) 상태·파일 경로.
    pub vault_path: std::path::PathBuf, pub known_hosts_open: bool, pub known_hosts_path: std::path::PathBuf,
    pub vault_unlock_open: bool,
    pub vault_pw_input: String,
    pub vault_status: String,
    pub pending_arrange: Option<crate::arrange::ArrangeMode>,
    pub broadcast: bool,
    pub palette_open: bool,
    /// 워크스페이스 스냅샷 모달 상태(T7-2) + 저장 이름 입력.
    pub snap_save_open: bool,
    /// 브로드캐스트 결과 집계 뷰(T7-3).
    pub bcast_view_open: bool,
    pub snap_list_open: bool,
    pub snap_name: String,
    /// nabiPad LSP 허브(T6-4) — rust-analyzer 진단·정의 이동.
    pub lsp: crate::editorlsp::LspHub,
    /// SFTP 전송 히스토리(S6-60) + 목록 창 열림.
    pub xfer_history: Vec<crate::sftphistory::XferRecord>,
    pub xfer_history_open: bool,
    /// 제어평면 SFTP 요청 상관 상태(S6-55).
    pub ctl_sftp: crate::controlsftp::CtlSftp,
    /// SSH 키 생성 모달 상태(Some=열림).
    pub keygen: Option<crate::sshkeygenui::KeygenState>,
    /// 폴더 동기화 다이얼로그(S6-51) + 트리 수집 상관 seq.
    pub sync_dlg: Option<crate::sftpsyncui::SyncDlg>,
    pub sync_seq: u64,
    /// 원격 파일 찾기 창(sftpfindui). None이면 닫혀 있다.
    pub sftp_find: Option<crate::sftpfindui::SftpFind>,
    /// 원격 내용 찾기의 진행 상태(배치 AD) — 창을 닫으면 버린다.
    pub sftp_grep: Option<crate::sftpgrepui::GrepRun>,
    /// 확인 대기 중인 원격 명령(remotecmdui). 실행 전에 전문을 보여 준다.
    pub rcmd_pending: Option<crate::remotecmdui::PendingCmd>,
    /// 원격 명령 결과 창.
    pub rcmd_result: Option<crate::remotecmdui::CmdResult>,
    /// 접속 이력(파일로 남는다 — connhist). 창이 열려 있으면 conn_hist_open.
    pub conn_hist: Vec<crate::connhist::Entry>,
    pub conn_hist_open: bool,
    /// 원격 최신유지 감시(S6-54, Some=켜짐).
    pub sync_watch: Option<crate::sftpwatch::SyncWatch>,
    /// 방금 끝난 명령(pane별) — 실패 AI 인계 컨텍스트(run_cmd는 종료 시 비워짐).
    pub last_run_cmd: std::collections::HashMap<nabi_types::PaneId, String>,
    /// 첫 실행 환영 화면(OOBE) 표시 중 — 완료 전엔 기본 셸 자동 스폰 보류.
    pub onboarding_open: bool,
    pub palette_query: String,
    pub find_open: bool, pub find_query: String, pub find_regex: bool, pub find_whole: bool, pub replace_open: bool, pub replace_find: String, pub replace_to: String, pub replace_count: Option<(usize, usize)>,
    pub tab_names: HashMap<PaneId, String>,
    pub tab_colors: HashMap<PaneId, egui::Color32>,
    /// 터미널 `파일:줄` 더블클릭 시 (경로, 0기반 줄) — 모델 락 해제 후 에디터로 연다(deferred).
    pub pending_pathline: Option<(String, usize)>, pub bell_flash: Option<std::time::Instant>,
    pub last_bell: HashMap<PaneId, usize>,
    pub broadcast_group: std::collections::HashSet<PaneId>, // MultiExec 대상(비면 전체).
    /// "휠을 키로 보내기"를 켠 pane — 스크롤백을 남기지 않는 TUI(codex CLI 등)용 수동 대응.
    pub wheel_keys: std::collections::HashSet<PaneId>,
    /// pane별 마지막 Ctrl+T(오버레이 열기) 전송 시각 — 화면 반영 전 재전송 방지 래치.
    pub tui_overlay: HashMap<PaneId, std::time::Instant>,
    /// 휠 도우미를 명시적으로 끈 pane(자동 감지보다 우선).
    pub wheel_keys_off: std::collections::HashSet<PaneId>,
    /// pane별 출처(로컬 셸/SSH). 워크스페이스 저장에 사용.
    pub pane_origins: HashMap<PaneId, nabi_session::SessionKind>,
    /// 최근 닫힌 탭의 출처 스택(실수로 닫은 세션 재열기용). 비밀번호 아닌 참조만 보관.
    pub closed_sessions: Vec<nabi_session::SessionKind>,
    pub workspace_path: std::path::PathBuf,
    /// 마우스 텍스트 선택(드래그→릴리스 시 자동 복사).
    pub selection: Option<crate::selection::Sel>,
    pub blink_start: std::time::Instant,
    pub window_title: String,
    /// pane별 cwd(OSC 7) + 실행 중 명령(OSC 633;E, 복원 재실행용) + 상태바 네트워크 정보(NIC/공인 IP).
    pub cwds: HashMap<PaneId, String>, pub run_cmd: HashMap<PaneId, String>, pub net_info: crate::netinfo::NetInfo,
    /// 비포커스 상태에서 출력이 발생한 pane(탭 활동 표시).
    pub activity: std::collections::HashSet<PaneId>,
    /// pane별 마지막 명령 종료 코드(OSC 133;D). 상태바 ✓/✗.
    pub last_exit: HashMap<PaneId, i32>,
    pub cmd_start: HashMap<PaneId, std::time::Instant>,
    pub last_duration: HashMap<PaneId, u128>,
    pub always_on_top: bool,
    /// 항상 위에 상태 변경 시 뷰포트 명령 전송 대기.
    pub pending_on_top: Option<bool>,
    /// 전체화면 상태 + 변경 대기.
    pub fullscreen: bool,
    pub pending_fullscreen: Option<bool>,
    /// 여러 탭 닫기 확인 대화상자 표시 여부.
    pub confirm_close: bool,
    pub did_startup: bool, // 첫 프레임 시작 처리(워크스페이스 자동 복원) 완료 여부.
    pub about_open: bool,  // About 대화상자 표시 여부.
    /// 자동 업데이트(GitHub 릴리스) + 인스톨러 실행 후 종료 플래그.
    pub updater: nabi_release::UpdateChecker, pub update_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 세션 UI 플래그: 새 버전 알림 모달(seen=중복 방지) + 셸 통합 설치 권장 +
    /// 도움말>정보 진입 시 업데이트 자동검사 1회(창 닫으면 리셋).
    pub update_modal: bool, pub update_seen: bool, pub shellinteg_prompt: bool, pub help_update_checked: bool,
    pub font_installer: crate::fontinstall::FontInstaller, // 코딩 폰트 클릭 다운로드.
    pub ime_preedit: String, // 포커스 pane IME 조합 텍스트(커서 오버레이).
    /// 창 핸들(OS 드롭 위치 판정) + 이번 프레임 드롭 존 rect들(브라우저/SFTP 라우팅).
    pub hwnd: Option<isize>, pub drop_zones: Vec<(crate::dnd::DropTarget, egui::Rect)>,
    /// 사이드바 편집 상태 + SSH 연결테스트(포트 도달성) 결과.
    /// 사이드바 다중 선택 집합(Ctrl/Shift 클릭) — 비어 있으면 평소 동작.
    /// ⋯ 더보기 메뉴가 열려 있는 행 이름 — 그 행의 아이콘을 계속 그려 메뉴가 살아 있게 한다.
    pub sidebar_menu_row: Option<String>,
    /// 선택 모드 — 켜면 사이드바 클릭이 '연결'이 아니라 '선택'이 된다.
    pub sidebar_pick_mode: bool,
    pub sidebar_marked: std::collections::HashSet<String>,
    /// Shift 범위 선택의 기준점(마지막으로 Ctrl/평클릭한 항목).
    pub sidebar_anchor: Option<String>,
    /// 일괄 연결 확인 대기 목록(자격증명 없는 세션이 섞였을 때).
    pub bulk_ask: Option<Vec<nabi_session::SavedSession>>,
    pub sidebar_selected: Option<String>, pub sidebar_new_group: String,
    pub sidebar_rename_group: Option<String>, pub sidebar_rename_to: String,
    pub reach: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// 워크스페이스 복원: 분할 레이아웃 재구성 대기 + 볼트 우선 복원 지연(브라우저 pane 포함).
    pub pending_layout: Option<crate::workspace::PendingLayout>, pub pending_restore: Option<Vec<nabi_types::PaneId>>,
    /// Quake 모드 전역 핫키 상태(등록 실패 시 None).
    pub quake: Option<crate::quake::QuakeState>,
    /// 최근 데스크톱 알림(OSC 9) — 토스트로 잠시 표시.
    pub notify: Option<(String, std::time::Instant)>,
    /// trzsz 파일 전송(확인 대화상자 + pane별 진행률).
    pub trzsz: crate::trzszui::TrzszUi,
    /// 화면 규칙 기반 에이전트 상태 감시(A1 — 훅 미설치 에이전트 폴백).
    pub agent_watch: crate::agentwatch::AgentWatch,
    /// pane 상태 키의 만료 시각(B7 TTL) — 지나면 tick이 삭제한다.
    pub pane_status_ttl: HashMap<(PaneId, String), std::time::Instant>,
    /// AI CLI 자동 업데이트 작업(시작 시 1회). 끝나면 무엇을 올렸는지 토스트로 알린다.
    pub ai_cli_auto: Option<crate::aicliupd::AutoJob>,
    /// 포커스 pane 리사이즈 직후 잠시 표시할 크기 배지(열×행, 시각).
    pub resize_badge: Option<(GridSize, std::time::Instant)>,
    /// 탭 바 "+" 클릭 신호 + 눌린 (surface,node); 비활성 pane 우클릭 포커스 요청; 탭 메뉴 열림 여부.
    pub add_requested: bool, pub add_target: Option<(egui_dock::SurfaceIndex, egui_dock::NodeIndex)>,
    pub focus_req: Option<PaneId>,
    /// 이번 프레임에 **컨텍스트 메뉴가 열려 있는 탭**(egui_dock이 그 탭의 closure를 부른다).
    pub tab_ctx_tab: Option<PaneId>,
    /// 직전 프레임에 팝업(탭 메뉴·상단 메뉴·툴팁 등)이 열려 있었는가 — 탭바 빈 공간
    /// 우클릭이 "방금 연 팝업"인지 "열려 있던 팝업을 닫는 클릭"인지 가르는 데 쓴다.
    pub popup_was_open: bool,
    /// 탭바 메뉴를 **방금 연 프레임**인가 — 여는 클릭이 '메뉴 밖 클릭'으로 잡혀
    /// 즉시 닫히는 것을 막는다.
    pub tabbar_menu_fresh: bool,
    /// 마우스/분리 창 붙여넣기 요청 — 프레임 끝에서 확인 경로로 보낸다.
    pub paste_req: Option<(PaneId, String)>,
    /// 차단형 프롬프트 때문에 메인 창을 이미 앞으로 불렀는지(뜨는 순간 한 번만).
    pub prompt_raised: bool,
    pub pending_ssh: Option<String>, pub pending_link: Option<(PaneId, String)>, pub telegram: crate::telegrambridge::TelegramBridge, pub telegram_targets: HashMap<i64, PaneId>,
    /// DM 페어링 대기(chat, 코드, 만료) — dm_policy=pairing일 때 미지 chat의 승인 요청(C1).
    pub telegram_pending: Vec<(i64, String, std::time::Instant)>,
    /// 하트비트(C5): 마지막 발신 시각·요약(변화 없으면 무발신).
    pub telegram_heartbeat: (Option<std::time::Instant>, String),
    /// 워크트리 만들기 입력(B6, 브랜치 이름) / 목록 모달((기준 cwd, 항목들)).
    pub worktree_prompt: Option<String>,
    pub worktree_list: Option<(String, Vec<crate::worktree::Wt>)>,
    /// 내장 스케줄러(C3): 잡 목록·영속 경로·마지막 틱.
    pub schedules: Vec<crate::scheduler::Job>,
    pub schedules_path: std::path::PathBuf,
    pub sched_last_tick: std::time::Instant,
    /// 여러 줄 붙여넣기 확인 대기((대상 pane, 보낼 바이트)).
    pub pending_paste: Option<(PaneId, Vec<u8>)>,
    /// pane별 작업 진행률(OSC 9;4) + SSH 서버 통계 + AI 도구 등 커스텀 상태 키-값(상태바/탭 표시).
    pub progress: HashMap<PaneId, u8>,
    /// 화면에서 읽은 진행률을 마지막으로 본 때. 오래 조용하면 지운다(배치 AM).
    pub progress_seen: HashMap<PaneId, std::time::Instant>,
    /// 프로그램이 `OSC 9;4` 로 직접 말한 pane. 그쪽이 권위라 화면을 읽지 않는다.
    pub progress_osc: std::collections::HashSet<PaneId>,
    pub server_stats: HashMap<PaneId, nabi_proto::stats::ServerStats>,
    pub pane_status: HashMap<PaneId, std::collections::BTreeMap<String, String>>,
    pub ssh_connect_time: HashMap<PaneId, std::time::Instant>,
    /// pane별 스크롤백 표식 — 긴 로그에서 되짚을 자리(scrollmark).
    /// 진행 중인 공개키 설치 한 건(읽기 → 확인 → 넣기).
    pub copy_id: Option<crate::copyidui::CopyId>,
    /// 돌아가는 중인 낱말 완성(되풀이해 누르면 다음 후보).
    pub word_cycle: Option<crate::wordcompui::WordCycle>,
    /// 내려받기를 기다리는 원격 비교 한 건(둘 다 오면 열린다).
    pub pending_diff: Option<crate::sftpdiff::PendingDiff>,
    /// **동기 스크롤** — 켜면 한 창을 굴릴 때 같은 그룹이 함께 움직인다.
    ///
    /// 대상은 동시 입력과 **같은 규칙**을 쓴다(`panegroup::targets`). 둘이 다르면
    /// "입력은 넷에 갔는데 스크롤은 셋만 움직이는" 일이 생긴다.
    pub sync_scroll: bool,
    /// **고정한 탭** — 실수로 닫히지 않는다.
    ///
    /// 편집기만이 아니라 아무 탭이나 고정할 수 있게 뒀다. 자리를 지키고 싶은 것은
    /// 문서만이 아니고(운영 서버 셸이 더 그렇다), 드는 품도 같기 때문이다.
    pub pinned_tabs: std::collections::HashSet<PaneId>,
    /// 명령 블록 목록 창이 열려 있나.
    pub block_list_open: bool,
    /// 블록 목록의 거르는 글자(명령 글자에서 찾는다).
    pub block_list_filter: String,
    /// 그 창에서 실패한 것만 보고 있나.
    pub block_list_failed_only: bool,
    pub scroll_marks: HashMap<PaneId, crate::scrollmark::Marks>,
    /// 세션(접속 정보)별 마지막 연결 실패 — 목록에서 왜 안 붙었는지 보여 준다.
    pub last_fail: crate::lastfail::FailMap,
    pub ssh_alert_on: HashMap<PaneId, bool>, pub ctx_alert_on: HashMap<PaneId, bool>,
    pub blocked_alert: HashMap<PaneId, bool>, pub ai_dash_open: bool, pub floating_on_top: bool,
    pub snippet_prompt: Option<SnippetPrompt>, pub dir_save_at: std::time::Instant,
    pub quick_select_open: bool, pub editor_close_ask: Option<PaneId>,
    /// 삭제 확인 대기 중인 저장 세션 이름(sessiondel).
    pub session_delete_ask: Option<String>,
    pub file_preview: Option<(String, String)>, pub clip_history: Vec<String>,
    /// 스크롤백 일치 개수 캐시 — **열쇠는 `FindKey`**(배치 AI).
    ///
    /// 예전에는 `(질의, 정규식, 개수)` 였는데 개수는 **단어 단위(`whole`)에도 의존**한다.
    /// 그래서 그 단추를 누르면 화면이 옛 개수를 그대로 보여 줬다 — 화면이 틀린 말을 하는 것이다.
    pub find_count_cache: Option<(crate::find::FindKey, usize)>,
    pub session_logs: HashMap<PaneId, crate::sessionlog::SessionLog>,
    /// 재생 중인 기록(pane별). 비어 있으면 아무 일도 하지 않는다.
    pub replays: crate::replay::Replays,
    /// 에이전트 행동 기록 창이 열려 있는가.
    pub agent_trail_open: bool,
    /// 거부를 이미 한 번 알렸는가 — 자율 에이전트는 막혀도 계속 시도하므로 한 번만 알린다.
    pub denial_noticed: bool,
    /// 해시 검증을 못 했다고 이미 알렸는가(배치 AF) — 한 번만 말한다.
    pub verify_skip_noticed: bool,
    /// 세션 로그 폴더를 못 만들었다고 이미 알렸는가(배치 AF) — 한 번만 말한다.
    pub autolog_fail_noticed: bool,
    /// 깨진 감지 규칙을 이미 알렸는가(배치 AF) — 한 번만 말한다.
    pub rules_drop_noticed: bool,
    /// 로컬 탐색기 일괄 이름변경 창(배치 AJ) — 닫으면 통째로 버린다.
    pub batch_rename: Option<crate::browserrename::BatchRename>,
    /// 방금 연 기록 — pane이 생기면 그 pane에 건다(PaneSpawned 를 기다린다).
    pub pending_replay: Option<Vec<(f64, String)>>,
    pub editor_mtimes: HashMap<PaneId, std::time::SystemTime>,
    pub editor_extcheck: std::time::Instant, pub autosave_at: std::time::Instant,
    pub note_edit: Option<(String, String)>, pub alert_marks: HashMap<PaneId, usize>,
    pub alert_check: std::time::Instant,
    /// 자동 응답: 마지막으로 본 화면 끝(같은 화면에 두 번 답하지 않기 위해).
    pub auto_reply_seen: HashMap<PaneId, String>,
    /// 자동 응답: (규칙 번호, 연속 발동 횟수) — 되먹임 차단.
    pub auto_reply_streak: HashMap<PaneId, (usize, u32)>,
    pub auto_reply_check: std::time::Instant,
    /// 명령이 시작된 시각(소요 시간 계측). 끝나면 지운다.
    pub cmd_started: HashMap<PaneId, std::time::Instant>,
    /// 비교 상대를 고르는 중인 문서(열린 문서끼리 비교).
    pub diff_pick: Option<PaneId>,
    /// 자동 터널을 편집 중인 세션 이름.
    pub fwd_edit: Option<String>,
    /// 세션별 환경변수 편집 창(세션 이름).
    pub env_edit: Option<String>,
    /// 재접속을 기다리는 pane — (물러서기 상태, 다음 시도 시각, 끊김 사유).
    pub reconnecting: HashMap<PaneId, (crate::backoff::Backoff, std::time::Instant, String)>,
    /// 방금 시도한 재접속의 상태 — 새 pane이 뜨면 그 pane으로 옮겨 붙인다.
    pub reconnect_carry: Option<(crate::backoff::Backoff, String)>,
    /// 진단 묶음 창(도움말·팔레트).
    pub bundle: Option<Vec<crate::supportbundle::Piece>>,
    /// 세션 도달성 일괄 확인 결과 — (호스트, 포트) → 상태.
    pub reach_all: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(String, u16), crate::reachall::Reach>>>,
    /// 이번 시작에서 정리한 오래된 로그 개수(진단 묶음에 적는다).
    pub pruned_logs: usize,
    /// 에이전트가 든 키(빠른 연결이 열릴 때 한 번 묻는다).
    pub agent_keys: Option<crate::agentkeys::Keys>,
    /// 최근에 닫은 문서 경로(다시 열기). 최신이 앞.
    pub closed_docs: Vec<String>,
    /// 다음 PaneSpawned를 분할 배치(Some(true)=오른쪽, Some(false)=아래).
    pub pending_split: Option<bool>,
    /// pane별 글꼴 크기 오버라이드(Ctrl+휠 확대/축소). 없으면 전역 font_size.
    pub pane_font: HashMap<PaneId, f32>,
    /// 페인 최대화(줌) 모드(tmux식). 켜지면 분할에서 포커스 터미널만 전체 영역에 렌더.
    pub pane_zoom: bool,
}

impl NabiApp {
    /// 저장 세션 트리를 디스크에 영속화한다.
    pub fn save_sessions(&self) {
        let _ = nabi_session::save_tree(&self.session_path, &self.sessions);
    }

    /// 포커스된 pane id.
    pub fn focused_pane(&mut self) -> Option<PaneId> {
        self.dock.find_active_focused().map(|(_, t)| *t)
    }

}

// eframe::App(프레임 루프)은 update.rs에 분리.

