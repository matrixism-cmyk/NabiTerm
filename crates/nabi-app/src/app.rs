//! eframe 앱 상태 + 업데이트 루프.

use eframe::CreationContext;
use nabi_orchestrator::{start, OrchestratorHandle};
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
    pub reconnect_ask: Option<(PaneId, String)>, pub hostkey_prompt: Option<(u64, String, u16, String, String)>,
    /// 탭바 빈 공간 우클릭 메뉴 위치(Some=표시 중).
    /// 탭바 우클릭 메뉴 위치 + 터미널 링크 길게 누름 메뉴((URL, 위치)).
    /// 마지막 필드: 인라인 이미지(Sixel) 텍스처 캐시(이미지 id → egui 텍스처).
    pub tabbar_menu: Option<egui::Pos2>, pub link_menu: Option<(String, egui::Pos2)>, pub floating_link: Option<(String, egui::Pos2)>, pub img_textures: std::collections::HashMap<u64, egui::TextureHandle>,
    /// 현재 창 크기(매 프레임 추적, 종료 시 config 저장).
    pub last_win: (f32, f32),
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
    pub palette_query: String,
    pub find_open: bool, pub find_query: String, pub find_regex: bool, pub replace_open: bool, pub replace_find: String, pub replace_to: String, pub replace_count: Option<(usize, usize)>,
    pub tab_names: HashMap<PaneId, String>,
    pub tab_colors: HashMap<PaneId, egui::Color32>,
    /// 터미널 `파일:줄` 더블클릭 시 (경로, 0기반 줄) — 모델 락 해제 후 에디터로 연다(deferred).
    pub pending_pathline: Option<(String, usize)>, pub bell_flash: Option<std::time::Instant>,
    pub last_bell: HashMap<PaneId, usize>,
    pub broadcast_group: std::collections::HashSet<PaneId>, // MultiExec 대상(비면 전체).
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
    pub sidebar_selected: Option<String>, pub sidebar_new_group: String, pub sidebar_rename_group: Option<String>, pub sidebar_rename_to: String, pub reach: std::sync::Arc<std::sync::Mutex<Option<String>>>, // 사이드바 상태 + SSH 연결테스트(포트 도달성) 결과.
    /// 워크스페이스 복원: 분할 레이아웃 재구성 대기 + 볼트 우선 복원 지연(브라우저 pane 포함).
    pub pending_layout: Option<crate::workspace::PendingLayout>, pub pending_restore: Option<Vec<nabi_types::PaneId>>,
    /// Quake 모드 전역 핫키 상태(등록 실패 시 None).
    pub quake: Option<crate::quake::QuakeState>,
    /// 최근 데스크톱 알림(OSC 9) — 토스트로 잠시 표시.
    pub notify: Option<(String, std::time::Instant)>,
    /// 포커스 pane 리사이즈 직후 잠시 표시할 크기 배지(열×행, 시각).
    pub resize_badge: Option<(GridSize, std::time::Instant)>,
    /// 탭 바 "+" 클릭 신호 + 눌린 (surface,node); 비활성 pane 우클릭 포커스 요청; 탭 메뉴 열림 여부.
    pub add_requested: bool, pub add_target: Option<(egui_dock::SurfaceIndex, egui_dock::NodeIndex)>,
    pub focus_req: Option<PaneId>, pub tab_ctx_open: bool,
    pub pending_ssh: Option<String>, pub pending_link: Option<(PaneId, String)>, pub telegram: crate::telegrambridge::TelegramBridge, pub telegram_target: Option<PaneId>,
    /// 여러 줄 붙여넣기 확인 대기((대상 pane, 보낼 바이트)).
    pub pending_paste: Option<(PaneId, Vec<u8>)>,
    /// pane별 작업 진행률(OSC 9;4) + SSH 서버 통계 + AI 도구 등 커스텀 상태 키-값(상태바/탭 표시).
    pub progress: HashMap<PaneId, u8>, pub server_stats: HashMap<PaneId, nabi_proto::stats::ServerStats>, pub pane_status: HashMap<PaneId, std::collections::BTreeMap<String, String>>, pub ssh_connect_time: HashMap<PaneId, std::time::Instant>, pub ssh_alert_on: HashMap<PaneId, bool>, pub ctx_alert_on: HashMap<PaneId, bool>, pub blocked_alert: HashMap<PaneId, bool>, pub ai_dash_open: bool, pub floating_on_top: bool, pub snippet_prompt: Option<SnippetPrompt>, pub dir_save_at: std::time::Instant, pub quick_select_open: bool, pub editor_close_ask: Option<PaneId>, pub file_preview: Option<(String, String)>, pub clip_history: Vec<String>, pub find_count_cache: Option<(String, bool, usize)>, pub session_logs: HashMap<PaneId, crate::sessionlog::SessionLog>, pub editor_mtimes: HashMap<PaneId, std::time::SystemTime>, pub editor_extcheck: std::time::Instant, pub autosave_at: std::time::Instant, pub note_edit: Option<(String, String)>, pub alert_marks: HashMap<PaneId, usize>, pub alert_check: std::time::Instant,
    /// 다음 PaneSpawned를 분할 배치(Some(true)=오른쪽, Some(false)=아래).
    pub pending_split: Option<bool>,
    /// pane별 글꼴 크기 오버라이드(Ctrl+휠 확대/축소). 없으면 전역 font_size.
    pub pane_font: HashMap<PaneId, f32>,
    /// 페인 최대화(줌) 모드(tmux식). 켜지면 분할에서 포커스 터미널만 전체 영역에 렌더.
    pub pane_zoom: bool,
}

impl NabiApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let hwnd = crate::windnd::hwnd_of(cc); // OS 파일 드롭 위치 판정용 창 핸들.
        let layout = nabi_config::StorageLayout::resolve();
        let config = nabi_config::load(&layout);
        // 구문 강조 자산 등록(사용자 폴더 base/nabipad/{syntaxes,themes}·테마·확장자 매핑).
        let editor_config = nabi_config::load_editor(&layout); let editor_config_path = layout.editor_file.clone(); crate::editorsyntax::init(&layout.base, editor_config.theme.clone(), editor_config.ext_map.clone());
        crate::fonts::install_cjk_fonts(&cc.egui_ctx, &config.appearance.font_family);
        crate::theme_ui::apply_theme(&cc.egui_ctx);
        // egui의 ID 충돌 디버그 경고("First use of … ID …")는 개발자 진단용 UI 오버레이로,
        // egui 내부에서 영어로 생성돼 현지화가 불가능하다. 최종 사용자에게 노출하지 않도록 끈다.
        cc.egui_ctx.options_mut(|o| o.warn_on_id_clash = false);
        let quake = crate::quake::init(&config.appearance.quake_hotkey);
        let config_path = layout.config_file.clone();
        let workspace_path = config_path
            .parent()
            .map(|p| p.join("workspace.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from("workspace.toml"));
        let vault_path = layout.vault.clone(); let known_hosts_path = layout.known_hosts.clone();
        // F1: vault_remember면 OS 자격증명으로 시작 시 자동 잠금 해제 시도.
        let (vault, vault_password) = crate::vault::auto_unlock(&config, &vault_path);
        let session_path = layout.sessions_file.clone();
        // 세션 파일이 깨졌으면 원본을 백업해 두고(데이터 보존) 그 사실을 사용자에게 알린다.
        let (sessions, session_backup) = nabi_session::load_tree_reporting(&session_path);

        let font_size = config.appearance.font_size;
        let lang = nabi_i18n::Lang::from_code(&config.appearance.language);
        let theme = crate::settings::build_theme(&config);
        let aot = config.appearance.always_on_top;
        // 시작 셸은 첫 프레임(did_startup)에서, 워크스페이스 복원으로 아무 것도 안 떴을 때만 띄운다.
        // 오케스트레이터가 출력/에코를 처리할 때마다 UI를 깨워(request_repaint) 입력 지연을 없앤다.
        let orch = { let ctx = cc.egui_ctx.clone(); start(move || ctx.request_repaint()) };
        // 에이전트 제어 평면(named pipe) — main이 심은 디스커버리 env로 서버 가동.
        let mode = nabi_control::policy::Mode::parse(&config.terminal.control_mode);
        let (control_policy, control_ask_rx) = nabi_control::policy::ControlPolicy::new(mode);
        let (control_app_tx, control_app_rx) = crossbeam_channel::unbounded();
        let control_events = nabi_control::subscribe::EventHub::new();
        if mode != nabi_control::policy::Mode::Off {
            if let (Ok(pipe), Ok(token)) =
                (std::env::var("NABI_CONTROL_PIPE"), std::env::var("NABI_CONTROL_TOKEN"))
            {
                let ctx = nabi_control::server::ServerCtx {
                    panes: orch.panes.clone(),
                    cmd_tx: orch.cmd_tx.clone(),
                    app_tx: control_app_tx,
                    policy: control_policy.clone(),
                    cfg: nabi_control::dispatch::SpawnCfg {
                        scrollback: config.terminal.scrollback,
                        encoding: config.terminal.encoding.clone(),
                        cols: 80, rows: 24,
                    },
                    events: control_events.clone(),
                };
                nabi_control::server::start(pipe, token, ctx);
            }
        }
        Self {
            orch,
            dock: egui_dock::DockState::new(vec![]),
            last_grid: HashMap::new(),
            font_size,
            theme,
            lang,
            quick_connect: crate::connect::QuickConnect::default(),
            forward: crate::forwardui::ForwardForm::default(),
            sftp: crate::sftppanel::SftpPanel::default(),
            sftp_pane: None,
            sftp_bg: std::collections::HashMap::new(),
            sftp_seq: 0,
            xfer_seq: 0,
            edits: Vec::new(), editors: HashMap::new(),
            compare_on: false,
            sync_browse: false,
            sync_local_root: std::path::PathBuf::new(),
            sync_remote_root: String::new(),
            sessions,
            session_path,
            browser: crate::browserpanel::BrowserPanel {
                sort: crate::browserfs::Sort::from_u8(config.terminal.browser_sort),
                sort_desc: config.terminal.browser_sort_desc,
                view: crate::sftpview::ViewMode::from_u8(config.terminal.browser_view),
                show_hidden: config.terminal.browser_show_hidden,
                ..Default::default()
            },
            browser_tabs: HashMap::new(),
            sidebar_filter: String::new(),
            reconnect_ask: None, hostkey_prompt: None,
            tabbar_menu: None, link_menu: None, floating_link: None, img_textures: std::collections::HashMap::new(),
            last_win: (0.0, 0.0),
            control_policy,
            control_ask_rx,
            control_pending: None,
            control_float: false,
            control_app_rx,
            control_events,
            pending_spawns: HashMap::new(), next_spawn_seq: 1_000_000_000, spawn_ctx: None,
            config, config_path, editor_config, editor_config_path, editor_settings_for: None,
            settings_open: false,
            settings_backup: None, settings_editor_backup: None, settings_live_font: String::new(),
            floating: Vec::new(), floating_geom: HashMap::new(), floating_shown: std::collections::HashSet::new(), docked_float: Vec::new(),
            close_signal: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            floating_grid: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            vault, vault_password, vault_path, known_hosts_open: false, known_hosts_path,
            vault_unlock_open: false,
            vault_pw_input: String::new(),
            vault_status: String::new(),
            pending_arrange: None,
            broadcast: false,
            palette_open: false,
            palette_query: String::new(),
            find_open: false,
            find_query: String::new(), find_regex: false, replace_open: false, replace_find: String::new(), replace_to: String::new(), replace_count: None,
            tab_names: HashMap::new(),
            tab_colors: HashMap::new(), pending_pathline: None,
            bell_flash: None,
            last_bell: HashMap::new(),
            broadcast_group: std::collections::HashSet::new(),
            pane_origins: HashMap::new(),
            closed_sessions: Vec::new(),
            workspace_path,
            selection: None,
            blink_start: std::time::Instant::now(),
            window_title: String::new(),
            cwds: HashMap::new(), run_cmd: HashMap::new(), net_info: crate::netinfo::NetInfo::new(),
            activity: std::collections::HashSet::new(),
            last_exit: HashMap::new(),
            cmd_start: HashMap::new(),
            last_duration: HashMap::new(),
            always_on_top: aot,
            pending_on_top: aot.then_some(true),
            fullscreen: false,
            pending_fullscreen: None,
            confirm_close: false,
            did_startup: false,
            about_open: false,
            updater: nabi_release::UpdateChecker::new(), update_quit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            update_modal: false, update_seen: false, shellinteg_prompt: false, help_update_checked: false,
            font_installer: crate::fontinstall::FontInstaller::default(), ime_preedit: String::new(), hwnd, drop_zones: Vec::new(), sidebar_selected: None, sidebar_new_group: String::new(), sidebar_rename_group: None, sidebar_rename_to: String::new(), reach: std::sync::Arc::new(std::sync::Mutex::new(None)),
            pending_layout: None, pending_restore: None,
            quake,
            // 세션 파일이 손상돼 백업했다면 첫 화면에서 경로를 알린다(조용한 소멸 방지).
            notify: session_backup.map(|b| {
                let msg = nabi_i18n::tr(lang, "sessions.corrupt");
                (format!("\u{26a0} {msg} \u{2192} {}", b.display()), std::time::Instant::now())
            }),
            resize_badge: None,
            add_requested: false, add_target: None, focus_req: None, tab_ctx_open: false,
            pending_ssh: None, pending_link: None, telegram: Default::default(), telegram_target: None,
            pending_paste: None,
            progress: HashMap::new(), server_stats: HashMap::new(), pane_status: HashMap::new(), ssh_connect_time: HashMap::new(), ssh_alert_on: HashMap::new(), ctx_alert_on: HashMap::new(), blocked_alert: HashMap::new(), ai_dash_open: false, floating_on_top: false, snippet_prompt: None, dir_save_at: std::time::Instant::now(), quick_select_open: false, editor_close_ask: None, file_preview: None, clip_history: Vec::new(), find_count_cache: None, session_logs: HashMap::new(), editor_mtimes: HashMap::new(), editor_extcheck: std::time::Instant::now(), autosave_at: std::time::Instant::now(), note_edit: None, alert_marks: HashMap::new(), alert_check: std::time::Instant::now(),
            pending_split: None,
            pane_font: HashMap::new(),
            pane_zoom: false,
        }
    }

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

