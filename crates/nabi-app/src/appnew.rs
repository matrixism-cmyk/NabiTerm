//! `NabiApp` 생성자 — 설정 로드·서브시스템 기동·초기 상태 구성. 상태 정의는 app.rs.
//!
//! 필드가 많아 생성자만으로도 파일 하나 분량이라 따로 둔다.
//! app.rs는 "무엇을 들고 있는가", 여기는 "어떻게 시작하는가"만 담는다.

use crate::app::NabiApp;
use eframe::CreationContext;
use nabi_orchestrator::start;
use std::collections::HashMap;
impl NabiApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let hwnd = crate::windnd::hwnd_of(cc); // OS 파일 드롭 위치 판정용 창 핸들.
        let layout = nabi_config::StorageLayout::resolve();
        let first_run = !layout.config_file.exists(); // OOBE: 설정 파일이 없으면 첫 실행.
        let config = nabi_config::load(&layout);
        // 구문 강조 자산 등록(사용자 폴더 base/nabipad/{syntaxes,themes}·테마·확장자 매핑).
        let editor_config = nabi_config::load_editor(&layout);
        let editor_config_path = layout.editor_file.clone();
        crate::editorsyntax::init(&layout.base, editor_config.theme.clone(), editor_config.ext_map.clone());
        nabi_editor::editortext::set_wrap_col(editor_config.wrap_col); // '줄바꿈' 변환 폭.
        crate::fonts::install_cjk_fonts(&cc.egui_ctx, &config.appearance.font_family);
        // 사용자 링크 규칙을 시작할 때 넣어 둔다. 설정을 열어야만 먹으면 규칙이 있는데도
        // 링크가 안 생긴다(내가 앞 배치에서 이 줄을 빠뜨렸다).
        nabi_render::urlrules::set_rules(&config.terminal.link_rules);
        // 오래된 진단 로그를 한 번 정리한다(보관 일수 0이면 아무것도 안 한다).
        // 시작 때 한 번이면 충분하다 — 하루에 한 파일씩만 늘어난다.
        let pruned_logs = {
            use chrono::Datelike;
            let t = chrono::Local::now();
            crate::logprune::prune(
                &layout.base.join("logs"),
                (t.year(), t.month(), t.day()),
                config.terminal.log_keep_days,
            )
        };
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
        // 내장 스케줄러(C3): 설정 폴더의 schedules.toml에서 로드(재시작 생존).
        let schedules_path = layout.base.join("schedules.toml");
        let schedules = crate::scheduler::load(&schedules_path);
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
        // 파일 속성 창의 해시 계산은 곁 스레드가 돌린다 — 큰 파일에 창이 멈추지 않게.
        let (hash_tx, hash_rx) = std::sync::mpsc::channel();
        let control_events = nabi_control::subscribe::EventHub::new();
        if mode != nabi_control::policy::Mode::Off {
            if let (Ok(pipe), Ok(token)) =
                (std::env::var("NABI_CONTROL_PIPE"), std::env::var("NABI_CONTROL_TOKEN"))
            {
                // 밖에서(탐색기 우클릭 등) 우리를 찾아올 수 있게 접속 정보를 남긴다.
                // 정상 종료 때 지우고, 남아 있어도 PID가 죽었으면 무시된다(discovery).
                nabi_control::discovery::write(&layout.base, &pipe, &token);
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
            ai_prof_open: false, ai_prof_backup: None, splash_since: None, pad_recover: Vec::new(), log_view: None, env_mgr: None, cmd_hist_open: false, preview: None, find_all: None, whatsnew: None, boot: None,
            file_props: None, hash_tx, hash_rx, import_screen: None,
            ai_picks: std::collections::HashMap::new(),
            ai_pick_out: None,
            ai_screen: std::collections::HashMap::new(),
            tip_cache: std::collections::HashMap::new(),
            tip_ai: crate::tipai::TipAi::load(&layout.base, &config.terminal.tip_cache_path),
            enc_cache: None,
            compare_at: None,
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
            trzsz: Default::default(),
            tabbar_menu: None, link_menu: None, floating_link: None, img_textures: std::collections::HashMap::new(),
            last_win: (0.0, 0.0), last_pos: None, raise_window: false,
            control_policy,
            control_ask_rx,
            control_pending: None,
            control_float: false,
            control_app_rx,
            control_events,
            pending_spawns: HashMap::new(), next_spawn_seq: 1_000_000_000, spawn_ctx: None,
            config, config_path, editor_config, editor_config_path, editor_settings_for: None,
            settings_open: false,
            settings_backup: None, settings_editor_backup: None, editor_settings_backup: None, settings_live_font: String::new(),
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
            snap_save_open: false, snap_list_open: false, snap_name: String::new(), bcast_view_open: false,
            lsp: Default::default(),
            xfer_history: Vec::new(), xfer_history_open: false,
            ctl_sftp: Default::default(),
            keygen: None,
            sync_dlg: None, sync_seq: 0, sync_watch: None, last_run_cmd: Default::default(),
            onboarding_open: first_run,
            palette_query: String::new(),
            find_open: false,
            find_query: String::new(), find_regex: false, find_whole: false, replace_open: false, replace_find: String::new(), replace_to: String::new(), replace_count: None,
            tab_names: HashMap::new(),
            tab_colors: HashMap::new(), pending_pathline: None,
            bell_flash: None,
            last_bell: HashMap::new(),
            broadcast_group: std::collections::HashSet::new(),
            wheel_keys: std::collections::HashSet::new(),
            tui_overlay: HashMap::new(),
            wheel_keys_off: std::collections::HashSet::new(),
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
            font_installer: crate::fontinstall::FontInstaller::default(),
            ime_preedit: String::new(), hwnd, drop_zones: Vec::new(),
            sidebar_pick_mode: false, sidebar_menu_row: None, sidebar_marked: Default::default(), sidebar_anchor: None, bulk_ask: None,
            sidebar_selected: None, sidebar_new_group: String::new(),
            sidebar_rename_group: None, sidebar_rename_to: String::new(),
            reach: std::sync::Arc::new(std::sync::Mutex::new(None)),
            pending_layout: None, pending_restore: None,
            quake,
            // 세션 파일이 손상돼 백업했다면 첫 화면에서 경로를 알린다(조용한 소멸 방지).
            notify: session_backup.map(|b| {
                let msg = nabi_i18n::tr(lang, "sessions.corrupt");
                (format!("\u{26a0} {msg} \u{2192} {}", b.display()), std::time::Instant::now())
            }),
            agent_watch: crate::agentwatch::AgentWatch::new(Some(&layout.base)),
            pane_status_ttl: HashMap::new(),
            ai_cli_auto: None,
            resize_badge: None,
            add_requested: false, add_target: None, focus_req: None, tab_ctx_tab: None, popup_was_open: false, tabbar_menu_fresh: false, paste_req: None, prompt_raised: false,
            pending_ssh: None, pending_link: None, telegram: Default::default(), telegram_targets: HashMap::new(), telegram_pending: Vec::new(),
            telegram_heartbeat: (None, String::new()), worktree_prompt: None, worktree_list: None,
            schedules, schedules_path, sched_last_tick: std::time::Instant::now(),
            pending_paste: None,
            progress: HashMap::new(), server_stats: HashMap::new(),
            pane_status: HashMap::new(), ssh_connect_time: HashMap::new(), last_fail: Default::default(), scroll_marks: HashMap::new(), pinned_tabs: Default::default(), block_list_open: false,
            block_list_failed_only: false, sftp_find: None, rcmd_pending: None, rcmd_result: None,
            conn_hist: crate::connhist::load(&nabi_config::resolve_base()), conn_hist_open: false,
            ssh_alert_on: HashMap::new(), ctx_alert_on: HashMap::new(),
            blocked_alert: HashMap::new(), ai_dash_open: false, floating_on_top: false,
            snippet_prompt: None, dir_save_at: std::time::Instant::now(),
            quick_select_open: false, editor_close_ask: None, session_delete_ask: None, file_preview: None,
            clip_history: Vec::new(), find_count_cache: None, session_logs: HashMap::new(),
            editor_mtimes: HashMap::new(), editor_extcheck: std::time::Instant::now(),
            autosave_at: std::time::Instant::now(), note_edit: None,
            alert_marks: HashMap::new(), alert_check: std::time::Instant::now(),
            auto_reply_seen: HashMap::new(), auto_reply_streak: HashMap::new(), auto_reply_check: std::time::Instant::now(), cmd_started: HashMap::new(),
            diff_pick: None, fwd_edit: None, env_edit: None, reconnecting: HashMap::new(), reconnect_carry: None, bundle: None, reach_all: Default::default(), pruned_logs, agent_keys: None, closed_docs: Vec::new(),
            pending_split: None,
            pane_font: HashMap::new(),
            pane_zoom: false,
        }
    }
}
