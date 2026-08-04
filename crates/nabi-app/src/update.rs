//! 프레임 루프(eframe::App) — 매 프레임 처리 순서를 한곳에 모은다(app.rs에서 분리).

use crate::app::NabiApp;

impl eframe::App for NabiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // UI 배율(ppp) 적용 — 포인터를 누르고 있는 동안(배율 슬라이더 드래그/클릭 포함)에는
        // 재적용을 미룬다. 매 프레임 적용하면 ppp 변경이 슬라이더 좌표계를 바꿔 포인터→값 매핑이
        // 어긋나며 값이 극단으로 튀는 피드백 루프가 생긴다 → 버튼을 뗄 때 1회만 반영.
        let scale = self.config.appearance.ui_scale.clamp(0.5, 3.0);
        if !ctx.input(|i| i.pointer.any_down()) && (ctx.pixels_per_point() - scale).abs() > 1e-3 {
            ctx.set_pixels_per_point(scale);
        }
        let sz = ctx.input(|i| i.screen_rect.size());
        self.last_win = (sz.x, sz.y); // 종료 시 창 크기 저장용 추적.
        if !self.did_startup {
            self.did_startup = true;
            // 저장된 SSH keepalive 설정을 시작 시 반영(설정 열기 전 첫 연결에도 적용).
            nabi_ssh::session::SSH_KEEPALIVE_SECS.store(self.config.terminal.ssh_keepalive_secs, std::sync::atomic::Ordering::Relaxed);
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
                if !(restored || self.config.terminal.restore_workspace && ws_exists) {
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
            self.maybe_prompt_shellinteg(); // 셸 통합 미설치면 설치 권장 모달.
        }
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
        self.check_external_changes(ctx); // 외부 파일 변경 감지(자동 리로드/경고).
        self.autosave_tick(); // 자동 저장(설정 켜짐 시 주기적).
        self.tick_telegram(); // 텔레그램 브리지: 설정 동기화 + 수신 메시지→pane 주입.
        self.update_compare_map(); // 디렉터리 비교 색칠용 로컬 맵.
        self.persist_view_prefs(); // 정렬/보기/숨김 변경 시 설정 저장.
        self.handle_shortcuts(ctx);
        self.menu_bar(ctx);
        self.show_quickconnect_bar(ctx);
        self.status_bar(ctx);
        self.show_sessions_sidebar(ctx);
        self.drop_zones.clear(); // 이번 프레임 드롭 존 재수집(브라우저/SFTP 렌더 시).
        self.show_browser(ctx);
        self.central(ctx);
        self.show_quick_connect(ctx);
        self.show_forward(ctx);
        self.show_settings(ctx);
        self.show_link_menu(ctx); // 터미널 링크 길게 누름 메뉴.
        self.show_update_prompt(ctx); // 새 버전 발견 시 알림 모달(업데이트/다음에/일주일후/끄기).
        self.show_shellinteg_prompt(ctx); // 셸 통합 설치 권장 모달.
        self.show_floating(ctx);
        self.show_docked_floats(ctx); // "창 안에 띄우기" 오버레이(P3).
        self.show_vault_unlock(ctx);
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
        self.show_paste_confirm(ctx);
        self.show_reconnect(ctx);
        self.render_note_dialog(ctx);
        self.show_hostkey_prompt(ctx);
        self.show_control_approval(ctx);
        self.show_about(ctx);
        self.show_known_hosts(ctx);
        self.ai_dashboard(ctx);
        self.file_preview_modal(ctx);
        self.editor_close_modal(ctx);
        self.quick_select_popup(ctx);
        self.snippet_prompt_modal(ctx);
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
        // 유휴 CPU 절약: 출력 이벤트는 즉시 repaint를 요청하므로, 하트비트만 조정.
        ctx.request_repaint_after(std::time::Duration::from_millis(self.idle_ms()));
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
    let top_right = ctx.input(|i| i.screen_rect).right_top() + egui::vec2(-8.0, 8.0);
    let min = egui::pos2(top_right.x - g.size().x, top_right.y);
    let textrect = egui::Rect::from_min_size(min, g.size());
    p.rect_filled(textrect.expand(3.0), 3.0, egui::Color32::from_black_alpha(180));
    p.galley(textrect.min, g, egui::Color32::from_rgb(0x9d, 0xff, 0x9d));
}
