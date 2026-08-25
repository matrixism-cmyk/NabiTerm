//! 프레임 루프(eframe::App) — 매 프레임 처리 순서를 한곳에 모은다(app.rs에서 분리).

use crate::app::NabiApp;

impl eframe::App for NabiApp {
    // 0.34: App::update → App::ui(&mut Ui). 우리 UI는 전부 ctx 레벨(패널/Area)이라
    // 루트 ui는 쓰지 않고 ctx만 꺼내 기존 본문을 그대로 돌린다.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // 전역 현재 언어 동기화 — 네트워크 계층(trc)이 새 에러를 이 언어로 만든다(T8-1).
        nabi_i18n::set_current(self.lang);
        // UI 배율(ppp) 적용 — 포인터를 누르고 있는 동안(배율 슬라이더 드래그/클릭 포함)에는
        // 재적용을 미룬다. 매 프레임 적용하면 ppp 변경이 슬라이더 좌표계를 바꿔 포인터→값 매핑이
        // 어긋나며 값이 극단으로 튀는 피드백 루프가 생긴다 → 버튼을 뗄 때 1회만 반영.
        let scale = self.config.appearance.ui_scale.clamp(0.5, 3.0);
        if !ctx.input(|i| i.pointer.any_down()) && (ctx.pixels_per_point() - scale).abs() > 1e-3 {
            ctx.set_pixels_per_point(scale);
        }
        let sz = ctx.input(|i| i.viewport_rect().size());
        self.last_win = (sz.x, sz.y); // 종료 시 창 크기 저장용 추적.
        // 창 위치도 추적한다(종료 시 저장 → 다음 실행에 그 자리로).
        if let Some(p) = ctx.input(|i| i.viewport().outer_rect).map(|r| r.min) {
            self.last_pos = Some((p.x, p.y));
        }
        if !self.did_startup {
            self.did_startup = true;
            // 시작 스플래시(설정에서 끌 수 있다) — 첫 프레임이 실제로 뜬 지금부터 센다.
            if self.config.appearance.splash {
                self.splash_since = Some(std::time::Instant::now());
            }
            // 지난 실행이 비정상 종료였는지 확인 — 미저장 문서가 남아 있으면 되살릴지 묻는다.
            self.load_pad_recovery();
            self.load_whatsnew(); // 업데이트 뒤 첫 실행이면 '새로워진 점'을 준비.
            // 첫 프레임이 떴다 = 사용자가 창을 본 순간. 시작 시간을 기록한다.
            if let Some(b) = self.boot.take() {
                b.first_frame();
            }
            // 그래픽 초기화를 무사히 통과했다. 표식을 지운다(gpupick).
            crate::gpupick::mark_ok();
            // 저장된 SSH keepalive 설정을 시작 시 반영(설정 열기 전 첫 연결에도 적용).
            nabi_ssh::session::SSH_KEEPALIVE_SECS.store(self.config.terminal.ssh_keepalive_secs, std::sync::atomic::Ordering::Relaxed);
            nabi_sftp::SFTP_VERIFY_HASH.store(self.config.terminal.sftp_verify_hash, std::sync::atomic::Ordering::Relaxed);
            nabi_sftp::set_name_charset(&self.config.terminal.sftp_name_charset); // 파일명 인코딩(CP949 서버 한글).
            nabi_sftp::set_upload_mode(&self.config.terminal.sftp_upload_mode); // 업로드 권한 정규화.
            // 과거 영속된 egui zoom_factor가 초기 창을 축소했을 수 있으므로
            // 줌 리셋 후 저장된 크기를 다시 적용 + 최소화 해제 + 전면 포커스.
            ctx.set_zoom_factor(1.0);
            let a = &self.config.appearance;
            let (w, h) = if a.window_w >= 400.0 && a.window_h >= 300.0 {
                (a.window_w, a.window_h)
            } else {
                (1200.0, 760.0)
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            // 브라우저 탭을 먼저 복원해 PaneId를 확보 → 레이아웃 서수(1000+i)와 매핑.
            let bpanes = if self.config.terminal.restore_workspace {
                self.restore_browser_tabs()
            } else {
                Vec::new()
            };
            // 볼트 기반 세션이 있고 볼트가 잠겨 있으면, 볼트 잠금해제 모달을 먼저 띄우고 복원을
            // 미룬다(잠금해제 후 자격증명 세션이 자동 연결되어 제자리에 복원되도록 — 사용자 제안).
            if self.config.terminal.restore_workspace && self.workspace_wants_vault() {
                self.vault_unlock_open = true;
                self.pending_restore = Some(bpanes); // 볼트 해제/창닫힘 후 update 루프에서 복원.
            } else {
                let ws_exists = self.workspace_path.exists();
                let restored =
                    self.config.terminal.restore_workspace && self.restore_workspace(bpanes);
                // 기본 셸은 첫 실행(워크스페이스 파일 없음)이나 복원 비활성일 때만. 복원 활성+파일 존재인데
                // 복원된 게 없으면(=이전에 모든 탭을 닫고 종료) 사용자 요청대로 빈 화면을 유지한다.
                // OOBE(첫 실행) 중엔 자동 스폰 보류 — 환영 화면의 "시작하기"가 고른 셸을 띄운다.
                if !(restored || self.onboarding_open || self.config.terminal.restore_workspace && ws_exists) {
                    let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
                    self.spawn_local(shell);
                }
            }
            // 시작 시 자동 업데이트 확인(옵션) — 백그라운드 스레드, UI 비차단.
            // "일주일 후에" 스누즈 기간 중이면 건너뛴다.
            if self.config.terminal.auto_check_update
                && crate::updatemodal::now_unix() >= self.config.terminal.update_remind_after
            {
                self.updater.check_async();
            }
            self.start_ai_cli_auto_update(); // AI CLI 자동 업데이트(설정에서 켠 경우, 하루 1회).
            self.maybe_prompt_shellinteg(); // 셸 통합 미설치면 설치 권장 모달.
        }
        self.poll_ai_cli_auto_update();
        // 팁 AI 번역이 끝났으면 캐시에 넣고 한 번 다시 그린다(오버레이 갱신).
        if self.tip_ai.poll() {
            ctx.request_repaint();
        }
        self.tick_agent_watch(); // 화면 규칙 에이전트 상태 감시(600ms 스로틀).
        // 업데이트 인스톨러가 실행됐으면 이 앱을 즉시 종료(설치 진행).
        // 확인 대화상자를 거치지 않고 바로 quit() — 인스톨러가 파일 교체를 빨리 시작하도록.
        if self.update_quit.load(std::sync::atomic::Ordering::Relaxed) {
            self.quit(); // 워크스페이스 저장 후 exit(0)(닫기 확인 건너뜀).
        }
        self.poll_quake(ctx);
        self.reset_blink_on_input(ctx);
        // 제어 AppCtl을 이벤트보다 먼저 적용 — DockNext가 같은 프레임의
        // PaneSpawned에 반영되도록(분할/새 창 도킹 순서 보장, CP-7).
        self.drain_control_app();
        self.handle_events(ctx);
        self.poll_edits(); // 편집 임시파일 저장 감지 → 원격 재업로드.
        self.flush_session_logs(); // 세션 로깅(활성 pane 출력→파일).
        self.check_output_alerts(ctx); // 출력 트리거 패턴 알림.
        self.check_auto_reply(); // 자동 응답(-> reply: 규칙, 기본 꺼짐).
        if self.tick_auto_reconnect() { ctx.request_repaint(); } // 물러서며 재접속(S1).
        self.check_external_changes(ctx); // 외부 파일 변경 감지(자동 리로드/경고).
        self.autosave_tick(); // 자동 저장(설정 켜짐 시 주기적).
        self.tick_telegram(); // 텔레그램 브리지: 설정 동기화 + 수신 메시지→pane 주입.
        self.update_compare_map(); // 디렉터리 비교 색칠용 로컬 맵.
        self.persist_view_prefs(); // 정렬/보기/숨김 변경 시 설정 저장.
        self.handle_shortcuts(ctx);
        self.menu_bar(ui);
        self.show_quickconnect_bar(ui);
        self.status_bar(ui);
        self.show_sessions_sidebar(ui);
        self.drop_zones.clear(); // 이번 프레임 드롭 존 재수집(브라우저/SFTP 렌더 시).
        self.show_browser(ui);
        self.central(ui);
        self.show_quick_connect(ctx);
        self.show_ai_profiles(ctx); // AI 터미널 프로필 관리 독립창.
        self.show_forward(ctx);
        self.show_settings(ctx);
        self.show_onboarding(ctx); // 첫 실행 환영 화면(T3-3).
        self.show_link_menu(ctx); // 터미널 링크 길게 누름 메뉴.
        self.show_update_prompt(ctx); // 새 버전 발견 시 알림 모달(업데이트/다음에/일주일후/끄기).
        self.show_shellinteg_prompt(ctx); // 셸 통합 설치 권장 모달.
        self.show_floating(ctx);
        self.show_docked_floats(ctx); // "창 안에 띄우기" 오버레이(P3).
        self.show_pad_recovery(ctx); // 지난 실행에서 잃을 뻔한 문서 되살리기.
        self.show_log_view(ctx); // 진단 로그 보기(도움말).
        self.show_env_mgr(ctx); // 환경 관리자(도구 메뉴).
        self.show_cmd_history(ctx); // 명령 기록(도구 메뉴).
        self.show_preview(ctx); // 원격 파일 미리보기(SFTP).
        self.show_compare_picker(ctx); // 열린 문서끼리 비교 상대 고르기.
        self.show_auto_forwards(ctx); // 세션별 자동 터널 편집.
        self.show_session_env(ctx); // 세션별 환경변수 편집.
        self.show_sftp_find(ctx); // 원격 파일 찾기.
        self.show_support_bundle(ctx); // 진단 묶음.
        self.show_find_all(ctx); // 모든 창에서 찾기.
        self.show_whatsnew(ctx); // 업데이트 뒤 첫 실행 안내.
        // 밖에서 부른 요청(탐색기 '여기서 열기')이면 창을 앞으로 — 뒤에서 열리면 열린 줄 모른다.
        if std::mem::take(&mut self.raise_window) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.drain_hashes(); // 곁 스레드가 낸 파일 해시 반영.
        self.show_file_props(ctx); // 파일 속성 창.
        self.show_import_screen(ctx); // 가져오기 한 화면.
        self.show_vault_unlock(ctx);
        // 스플래시는 **맨 마지막**에 그린다 — 무엇 위에든 덮여야 한다.
        if let Some(t) = self.splash_since {
            if !crate::splash::show(ctx, t, self.lang) {
                self.splash_since = None;
            }
        }
        // 볼트 우선 복원: 볼트가 풀렸거나(자격증명 세션 자동연결 가능) 사용자가 볼트 창을 닫으면
        // 미뤄둔 워크스페이스 복원을 1회 진행한다(panes를 단계적으로 활성화).
        if self.pending_restore.is_some() && (self.vault.is_some() || !self.vault_unlock_open) {
            if let Some(bpanes) = self.pending_restore.take() {
                // 파일이 없을 때(첫 실행)만 기본 셸 — 비어 있으면(모두 닫고 종료) 빈 화면 유지.
                if !self.restore_workspace(bpanes) && !self.workspace_path.exists() {
                    let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
                    self.spawn_local(shell);
                }
            }
        }
        // 마우스/분리 창에서 들어온 붙여넣기 요청을 확인 경로로 보낸다(입구는 달라도 규칙은 하나).
        if let Some((pane, text)) = self.paste_req.take() {
            self.paste_text_to_pane(pane, text);
        }
        // 차단형 프롬프트는 메인 창에서만 그려진다. 분리 창에서 일하는 중에 뜨면
        // 사용자는 못 보고 연결은 대답을 기다리며 멈춘다 — 메인 창을 앞으로 부른다.
        let pending = self.hostkey_prompt.is_some()
            || self.control_pending.is_some()
            || self.reconnect_ask.is_some();
        if crate::promptfocus::should_raise(pending, self.prompt_raised, !self.floating.is_empty()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.prompt_raised = pending;
        self.show_paste_confirm(ctx);
        self.show_bulk_confirm(ctx); // 세션 일괄 연결 확인(자격증명 필요 개수 안내).
        self.show_reconnect(ctx);
        self.render_note_dialog(ctx);
        self.show_hostkey_prompt(ctx);
        self.show_trzsz_ask(ctx);
        self.show_control_approval(ctx);
        self.show_about(ctx);
        self.show_known_hosts(ctx);
        self.ai_dashboard(ctx);
        self.file_preview_modal(ctx);
        self.editor_close_modal(ctx);
        self.session_delete_modal(ctx);
        self.quick_select_popup(ctx);
        self.snippet_prompt_modal(ctx);
        self.show_worktree_modals(ctx); // git 워크트리 만들기/목록(B6).
        self.show_snapshot_modals(ctx); // 워크스페이스 스냅샷(T7-2).
        self.show_broadcast_results(ctx); // 일괄 명령 결과 집계(T7-3).
        self.lsp_tick(); // nabiPad LSP 동기화·진단·정의 응답(T6-4).
        self.show_xfer_history(ctx); // SFTP 전송 히스토리(S6-60).
        self.show_keygen_modal(ctx); // SSH 키 생성(ed25519).
        self.show_sync_dialog(ctx); // 폴더 동기화(S6-51).
        self.sync_watch_tick(ctx); // 원격 최신유지(S6-54).
        self.tick_scheduler(); // 내장 스케줄러(C3, 2초 스로틀).
        self.show_toast(ctx);
        self.show_resize_badge(ctx);
        self.show_command_palette(ctx);
        self.show_find_bar(ctx);
        self.show_replace_in_files(ctx);
        if let Some(msg) = self.reach.lock().ok().and_then(|mut g| g.take()) { self.notify = Some((msg, std::time::Instant::now())); } // SSH 연결 테스트 결과.
        self.dispatch_dropped_files(ctx); // OS 파일 드롭을 커서 위치의 패널로 라우팅.
        self.update_window_title(ctx);
        self.visual_bell(ctx);
        self.handle_close(ctx);
        self.process_pending(ctx);
        perf_overlay(ctx, frame); // NABI_PERF 설정 시 프레임 CPU 시간 HUD(P5 측정).
        // 유휴 CPU 절약: 출력·입력은 각자 repaint를 요청하므로, 여기서는 화면이 실제로
        // 바뀔 시점(깜빡임 토글·벨 플래시)만 예약한다. 안 보이는 창은 아예 깨우지 않는다.
        self.schedule_next_frame(ctx);
    }
}

/// 환경변수 `NABI_PERF`가 설정돼 있으면 직전 프레임 CPU 시간(ms)을 우상단에 표시한다.
/// eframe이 이미 측정해 둔 값(`cpu_usage`)을 읽기만 하므로 평소(미설정)엔 비용이 없다.
fn perf_overlay(ctx: &egui::Context, frame: &eframe::Frame) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("NABI_PERF").is_some()) {
        return;
    }
    let ms = frame.info().cpu_usage.unwrap_or(0.0) * 1000.0;
    let txt = format!("{ms:.2} ms / {:.0} fps", if ms > 0.0 { 1000.0 / ms } else { 0.0 });
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("nabi_perf_hud"),
    ));
    let g = p.layout_no_wrap(txt, egui::FontId::monospace(12.0), egui::Color32::WHITE);
    let top_right = ctx.input(|i| i.content_rect()).right_top() + egui::vec2(-8.0, 8.0);
    let min = egui::pos2(top_right.x - g.size().x, top_right.y);
    let textrect = egui::Rect::from_min_size(min, g.size());
    p.rect_filled(textrect.expand(3.0), 3.0, egui::Color32::from_black_alpha(180));
    p.galley(textrect.min, g, egui::Color32::from_rgb(0x9d, 0xff, 0x9d));
}
