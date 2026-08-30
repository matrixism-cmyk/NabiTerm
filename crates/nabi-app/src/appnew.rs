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
        // **버린 키를 함께 받는다.** 설정 한 줄이 어긋나면 그 키만 버리고 나머지는
        // 읽는데(tolerant), 지금까지 무엇을 버렸는지 아무도 말해 주지 않았다. 그래서
        // 사용자는 자기 설정이 왜 원래대로 돌아갔는지 알 수 없었다 — 만들어 두고 쓰지
        // 않던 `load_reporting` 을 이제 쓴다(2026-08-30 전수 점검).
        let (config, dropped_keys) = nabi_config::load_reporting(&layout);
        // **어제 남은 토큰은 오늘도 토큰이다.** 가리기를 켜기 전에 쌓인 기록에는
        // 비밀이 그대로 있다 — 불러올 때 한 번 훑어 지운다(디스크에도 다음 저장에 반영).
        let config = crate::redact::sweep_history(config);
        // 기본 셸이 이 PC 에서 실행되지 않으면 **열리는 셸로 바꾼다**(배치 AK).
        //
        // 스토어판 PowerShell 7 처럼 설치는 되어 있는데 실행되지 않는 셸이 기본으로 잡혀
        // 있으면, 탐색기 우클릭도 새 탭도 전부 안 열린다. 그런데 무엇이 잘못됐는지는
        // 화면 어디에도 나오지 않는다.
        //
        // 바꿨다는 사실은 아래에서 알린다. 사용자가 고른 값을 우리가 바꾸는 일이라,
        // 말하지 않으면 "내가 설정한 게 왜 다른 걸로 되어 있지?" 하고 헤매게 된다.
        let (config, shell_swap) = crate::appnew::fix_default_shell(config, &layout);
        // 나가도 되는지·공인 IP를 볼지는 **여기서 한 번** 읽는다(아래 초기화에서 config가
        // 옮겨진 뒤에는 못 읽는다).
        let ip_lookup = config.terminal.public_ip_lookup && !config.terminal.offline_mode;
        crate::egress::set_offline(config.terminal.offline_mode);
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
        // 제어 요청은 **UI 를 깨워야** 처리된다.
        //
        // egui 는 할 일이 있을 때만 다시 그린다. 그래서 앱이 놀고 있으면 `update()` 가
        // 돌지 않고, 앱이 처리해야 하는 제어 요청(web-*, layout export, screenshot …)이
        // 통에 담긴 채 그대로 있다가 시간 초과로 끝난다.
        //
        // 실제로 그렇게 걸렸다(2026-08-30). 웹 조종 열세 개가 전부 "웹 탭 응답 시간 초과"로
        // 나왔는데, 로그를 보면 요청은 앱까지 도착해 있었다. 마우스를 움직이면 되고 가만두면
        // 안 되는 것이 증상이었다 — 그래서 예전 시험은 통과했다.
        //
        // 그래서 통에 넣는 길 가운데에 한 겹을 둔다. 넣을 때마다 UI 를 깨운다.
        let (control_app_tx, control_app_rx) = crossbeam_channel::unbounded();
        let control_app_tx = {
            let (raw_tx, raw_rx) = crossbeam_channel::unbounded::<nabi_proto::AppCtl>();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(a) = raw_rx.recv() {
                    if control_app_tx.send(a).is_err() {
                        break; // 앱이 끝났다.
                    }
                    ctx.request_repaint();
                }
            });
            raw_tx
        };
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
            ai_prof_open: false, ai_prof_backup: None, splash_since: None, pad_recover: Vec::new(), log_view: None, env_mgr: None, cmd_hist_open: false,
            preview: None, find_all: None, pending_send: None, editor_conflict: None, whatsnew: None, boot: None,
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
            web_tabs: HashMap::new(),
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
            // 탐색기 우클릭 "nabiPad로 편집" 으로 떴는가(main.rs 가 넣어 준다).
            pad_only: std::env::var("NABI_PAD_ONLY").is_ok(),
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
            cwds: HashMap::new(), run_cmd: HashMap::new(), net_info: crate::netinfo::NetInfo::new(ip_lookup),
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
            hist_view: None,
            self_update_pending: false,
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
            // 시작할 때 알릴 것을 모은다. 둘 다 생겼으면 둘 다 말한다 — 하나가 다른
            // 하나를 덮으면 그 사실은 사용자가 영영 모른다(배치 AK).
            notify: crate::appnew::startup_notice(lang, session_backup, shell_swap, &dropped_keys),
            agent_watch: crate::agentwatch::AgentWatch::new(Some(&layout.base)),
            pane_status_ttl: HashMap::new(),
            ai_cli_auto: None,
            resize_badge: None,
            add_requested: false, add_target: None, focus_req: None, tab_ctx_tab: None, popup_was_open: false, tabbar_menu_fresh: false, paste_req: None, prompt_raised: false,
            pending_ssh: None, pending_link: None, telegram: Default::default(), telegram_targets: HashMap::new(), telegram_pending: Vec::new(),
            telegram_heartbeat: (None, String::new()), worktree_prompt: None, worktree_list: None,
            schedules, schedules_path, sched_last_tick: std::time::Instant::now(),
            pending_paste: None,
            pane_rects: HashMap::new(),
            wheel_hinted: Default::default(),
            progress: HashMap::new(),
            progress_seen: HashMap::new(),
            progress_osc: Default::default(), server_stats: HashMap::new(),
            pane_status: HashMap::new(), ssh_connect_time: HashMap::new(), last_fail: Default::default(), scroll_marks: HashMap::new(), pinned_tabs: Default::default(), sync_scroll: false,
            pending_diff: None, word_cycle: None, copy_id: None, block_list_open: false,
            block_list_failed_only: false, block_list_filter: String::new(), sftp_find: None, sftp_grep: None, rcmd_pending: None, rcmd_result: None,
            conn_hist: crate::connhist::load(&nabi_config::resolve_base()), conn_hist_open: false,
            ssh_alert_on: HashMap::new(), ctx_alert_on: HashMap::new(),
            blocked_alert: HashMap::new(), ai_dash_open: false, floating_on_top: false,
            snippet_prompt: None, dir_save_at: std::time::Instant::now(),
            quick_select_open: false, editor_close_ask: None, session_delete_ask: None, file_preview: None,
            clip_history: Vec::new(), find_count_cache: None, session_logs: HashMap::new(), replays: Default::default(), pending_replay: None,
            agent_trail_open: false, denial_noticed: false, verify_skip_noticed: false, autolog_fail_noticed: false, rules_drop_noticed: false, batch_rename: None,
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

/// 기본 셸이 실행되지 않으면 바꾼다. `(고친 설정, 바꿨으면 (이전, 새것))`.
///
/// 설정 파일에도 바로 적는다 — 다음에 켤 때 또 같은 일을 겪지 않도록.
pub(crate) fn fix_default_shell(
    mut config: nabi_config::AppConfig,
    layout: &nabi_config::StorageLayout,
) -> (nabi_config::AppConfig, Option<(String, String)>) {
    let usable: Vec<nabi_proto::ShellKind> =
        crate::menu::installed_shells().into_iter().map(|(_, k)| k).collect();
    let Some(next) = crate::shellfallback::pick(&config.terminal.default_shell, &usable) else {
        return (config, None);
    };
    let prev = std::mem::replace(&mut config.terminal.default_shell, next.clone());
    // 설정 파일에도 바로 적는다 — 다음에 켤 때 또 같은 일을 겪지 않도록.
    // 여기서 실패해도 프로그램은 그대로 뜬다. 이번에 켠 동안은 바뀐 값으로 동작한다.
    // 삼킴: 창이 뜨기 전이라 알릴 자리가 없다. 이번에 켠 동안은 바뀐 값으로 동작한다.
    let _ = nabi_config::save(&layout.config_file, &config);
    (config, Some((prev, next)))
}

/// 시작할 때 한 번 띄울 알림을 모은다.
///
/// 둘 다 생겼으면 **둘 다** 말한다. 알림 자리는 하나뿐이라 예전 같으면 하나가 다른 하나를
/// 덮었을 텐데, 덮인 쪽은 사용자가 영영 모른다. 짧게 이어 붙여 함께 보여 준다.
pub(crate) fn startup_notice(
    lang: nabi_i18n::Lang,
    session_backup: Option<std::path::PathBuf>,
    shell_swap: Option<(String, String)>,
    dropped_keys: &[String],
) -> Option<(String, std::time::Instant)> {
    let mut parts: Vec<String> = Vec::new();
    if !dropped_keys.is_empty() {
        // 몇 개인지와 **어느 키인지**를 함께 말한다. 개수만 말하면 어디를 고쳐야 할지 모른다.
        // 많으면 앞의 셋만 — 알림 한 줄이 화면을 가로지르면 아무도 안 읽는다.
        let shown: Vec<&str> = dropped_keys.iter().take(3).map(String::as_str).collect();
        let more = dropped_keys.len().saturating_sub(shown.len());
        let tail = match more {
            0 => String::new(),
            n => format!(" \u{2026}+{n}"),
        };
        parts.push(format!(
            "\u{26a0} {} {}{tail}",
            nabi_i18n::tr(lang, "config.dropped"),
            shown.join(", ")
        ));
    }
    if let Some(b) = session_backup {
        parts.push(format!("\u{26a0} {} \u{2192} {}", nabi_i18n::tr(lang, "sessions.corrupt"), b.display()));
    }
    if let Some((prev, next)) = shell_swap {
        parts.push(format!("{} {prev} \u{2192} {next}", nabi_i18n::tr(lang, "shell.swapped")));
    }
    (!parts.is_empty()).then(|| (parts.join(" \u{00b7} "), std::time::Instant::now()))
}

#[cfg(test)]
mod notice_tests {
    use super::startup_notice;
    use nabi_i18n::Lang;

    /// 아무 일도 없었으면 아무 말도 하지 않는다.
    #[test]
    fn 조용할_때는_알리지_않는다() {
        assert!(startup_notice(Lang::Ko, None, None, &[]).is_none());
    }

    /// 읽지 못한 설정이 있으면 **어느 키인지** 말한다.
    ///
    /// 개수만 말하면 어디를 고쳐야 할지 모른다 — 그러면 알리지 않은 것과 비슷해진다.
    #[test]
    fn 버린_설정_키를_이름으로_말한다() {
        let (msg, _) = startup_notice(Lang::Ko, None, None, &["appearance.font_size".into()])
            .expect("알려야 한다");
        assert!(msg.contains("appearance.font_size"), "{msg}");
    }

    /// 많으면 앞의 셋만 적고 나머지는 개수로 — 한 줄이 화면을 가로지르면 아무도 안 읽는다.
    #[test]
    fn 너무_많으면_줄여서_말한다() {
        let keys: Vec<String> = (0..7).map(|i| format!("k{i}")).collect();
        let (msg, _) = startup_notice(Lang::Ko, None, None, &keys).expect("알려야 한다");
        assert!(msg.contains("k0") && msg.contains("k2"), "{msg}");
        assert!(!msg.contains("k3"), "넷째부터는 이름을 적지 않는다: {msg}");
        assert!(msg.contains("+4"), "남은 개수를 말해야 한다: {msg}");
    }

    /// 여러 가지가 한꺼번에 생겨도 **덮지 않고 함께** 말한다.
    #[test]
    fn 여러_가지가_함께_나온다() {
        let (msg, _) = startup_notice(
            Lang::Ko,
            None,
            Some(("pwsh".into(), "powershell".into())),
            &["terminal.scrollback".into()],
        )
        .expect("알려야 한다");
        assert!(msg.contains("terminal.scrollback"), "{msg}");
        assert!(msg.contains("pwsh") && msg.contains("powershell"), "{msg}");
    }
}
