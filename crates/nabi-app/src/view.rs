//! 중앙 터미널 뷰: egui_dock 탭/분할로 pane들을 렌더한다.

use crate::app::NabiApp;

impl NabiApp {
    pub(crate) fn central(&mut self, ctx: &egui::Context) {
        // 포커스된 탭의 활동 표시는 해제.
        let focused = self.focused_pane();
        if let Some(p) = focused {
            self.activity.remove(&p);
        }
        // Ctrl+휠 글꼴 확대/축소: pane 위에서 굴리면 그 pane(포커스 불필요)을, 빈 영역이면 전역.
        // 실제 대상 pane은 paint_term이 포인터 위치로 판별해 zoom_req에 담는다(아래에서 적용).
        let ctrl_wheel = ctx.input(|i| if i.modifiers.command { i.raw_scroll_delta.y } else { 0.0 });
        let mut zoom_req: Option<(nabi_types::PaneId, f32)> = None;
        let mut resized: Option<nabi_types::GridSize> = None;
        // 탭 우클릭 신호: tear_off=새 OS 창, dock_float=창 안에 띄우기(P3), sftp_open=SFTP 열기.
        let (mut tear_off, mut dock_float, mut sftp_open): (Option<nabi_types::PaneId>, _, _) = (None, None, None);
        // 브라우저 탭: 액션/닫힘 수집 + 렌더에 필요한 비교맵·업로드 가능 여부(차용 전 계산).
        let mut browser_act: Vec<(nabi_types::PaneId, crate::browser::BrowserAct)> = Vec::new();
        let mut browser_closed: Option<nabi_types::PaneId> = None;
        let mut editor_act: Vec<(nabi_types::PaneId, crate::editor::EditorAct)> = Vec::new();
        let mut editor_closed: Option<nabi_types::PaneId> = None;
        let remote_map = self.remote_compare_map();
        let can_upload = self.sftp.open && self.sftp.id.is_some();
        // 줌 모드: 포커스가 유효한 터미널 pane이면 그것만 전체 영역에 렌더(원격 패널은 제외→dock).
        let zoom_pane = if self.pane_zoom {
            focused.filter(|p| {
                self.dock.find_tab(p).is_some()
                    && Some(*p) != self.sftp_pane
                    && !self.browser_tabs.contains_key(p)
                    && !self.sftp_bg.contains_key(p)
            })
        } else {
            None
        };
        let mut spawn_empty = false;
        let mut connect_empty = false;
        let mut pick: Option<nabi_session::SavedSession> = None;
        let mut sftp_act = crate::sftptab::SftpAct::default();
        let mut sftp_closed: Option<nabi_types::PaneId> = None;
        // 포커스된 탭이 배경 원격 패널이면 활성으로 스왑(렌더 전에 self.sftp = 그 패널).
        if let Some(fp) = self.dock.find_active_focused().map(|(_, t)| *t) {
            if self.sftp_bg.contains_key(&fp) {
                if let Some(old) = self.sftp_pane {
                    self.sftp_bg.insert(old, std::mem::take(&mut self.sftp));
                }
                if let Some(p) = self.sftp_bg.remove(&fp) {
                    self.sftp = p;
                }
                self.sftp_pane = Some(fp);
            }
        }
        // 기본 8px 여백은 콘텐츠 둘레 띠가 두꺼워 보임 — 2px로 축소.
        let cframe = egui::Frame::central_panel(&ctx.style())
            .inner_margin(egui::Margin::same(2.0));
        egui::CentralPanel::default().frame(cframe).show(ctx, |ui| {
            if self.dock.iter_all_tabs().next().is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    if ui.button(nabi_i18n::tr(self.lang, "central.newtab")).clicked() {
                        spawn_empty = true;
                    }
                    if ui.button(nabi_i18n::tr(self.lang, "qc.title")).clicked() {
                        connect_empty = true;
                    }
                    // 저장 세션 빠른 접속 런처.
                    if !self.sessions.sessions.is_empty() {
                        ui.add_space(10.0);
                        ui.label(nabi_i18n::tr(self.lang, "status.sessions"));
                        for s in &self.sessions.sessions {
                            if ui.button(s.name.as_str()).clicked() {
                                pick = Some(s.clone());
                            }
                        }
                    }
                });
                return;
            }
            let blink_on = self.blink_on();
            self.tab_ctx_open = false; // 매 프레임 리셋 — dock.show 중 탭 메뉴가 열리면 true.
            // egui_dock 내장 'Eject'(같은 창 안에 떠 있는 패널로) 라벨을 알기 쉽게 — lang 바뀔 때만 갱신.
            let eject = nabi_i18n::tr(self.lang, "tab.eject");
            if self.dock.translations.tab_context_menu.eject_button != eject {
                self.dock.translations.tab_context_menu.eject_button = eject.to_string();
            }
            // 이 창의 터미널 pane 집합(에디터·브라우저·SFTP 탭 제외) — 브로드캐스트 대상 범위.
            let window_panes: std::collections::HashSet<nabi_types::PaneId> = self
                .dock
                .iter_all_tabs()
                .map(|(_, p)| *p)
                .filter(|p| {
                    !self.editors.contains_key(p)
                        && !self.browser_tabs.contains_key(p)
                        && Some(*p) != self.sftp_pane
                        && !self.sftp_bg.contains_key(p)
                })
                .collect();
            let mut viewer = crate::tabs::TermTabViewer {
                orch: &self.orch,
                theme: self.theme,
                font_size: self.font_size,
                last_grid: &mut self.last_grid,
                broadcast: self.broadcast,
                show_pane_ids: self.config.appearance.show_pane_ids,
                // 하이라이트는 리터럴 셀 매칭만(정규식 모드는 탐색·카운트만, 하이라이트 생략 — B7).
                find: if self.find_open && !self.find_regex && !self.find_query.is_empty() {
                    Some(self.find_query.clone())
                } else {
                    None
                },
                highlights: &self.config.terminal.highlight_keywords,
                tab_names: &mut self.tab_names,
                lang: self.lang,
                broadcast_group: &mut self.broadcast_group,
                window_panes: &window_panes,
                selection: &mut self.selection,
                tab_colors: &mut self.tab_colors,
                pending_pathline: &mut self.pending_pathline,
                blink_on,
                copy_on_select: self.config.appearance.copy_on_select,
                activity: &self.activity,
                pane_status: &self.pane_status,
                run_cmd: &self.run_cmd,
                running: &self.cmd_start,
                ime_preedit: &mut self.ime_preedit,
                add_requested: &mut self.add_requested,
                add_target: &mut self.add_target,
                ssh_click: &mut self.pending_ssh,
                link_click: &mut self.pending_link,
                focused,
                focus_req: &mut self.focus_req,
                paste_req: &mut self.paste_req,
                tab_ctx_open: &mut self.tab_ctx_open,
                pane_font: &self.pane_font,
                cwds: &self.cwds,
                sftp: &mut self.sftp,
                sftp_pane: self.sftp_pane,
                sftp_bg: &self.sftp_bg,
                sftp_act: &mut sftp_act,
                sftp_bookmarks: &self.config.terminal.sftp_bookmarks,
                sort_desc: self.browser.sort_desc,
                sftp_closed: &mut sftp_closed,
                zoom_req: &mut zoom_req,
                resized: &mut resized,
                tear_off: &mut tear_off,
                dock_float: &mut dock_float,
                browser_tabs: &mut self.browser_tabs,
                browser_act: &mut browser_act,
                browser_closed: &mut browser_closed,
                remote_map: &remote_map,
                can_upload,
                editors: &mut self.editors,
                recent_files: &self.editor_config.recent_files,
                editor_act: &mut editor_act,
                editor_closed: &mut editor_closed,
                link_menu: &mut self.link_menu,
                img_textures: &mut self.img_textures,
                pane_origins: &self.pane_origins,
                sftp_open: &mut sftp_open,
            };
            if let Some(zp) = zoom_pane {
                viewer.paint_term(ui, zp); // tmux식 줌: 단일 pane 전체화면.
            } else {
                egui_dock::DockArea::new(&mut self.dock)
                    .style(crate::theme_ui::dock_style(ui.style()))
                    .show_add_buttons(true)
                    .show_inside(ui, &mut viewer);
            }
        });
        // Ctrl+휠 확대/축소 적용: 포인터가 올라간 pane이 있으면 그 pane(+활성화), 없으면 전역.
        if let Some((p, wheel)) = zoom_req {
            let cur = self.pane_font.get(&p).copied().unwrap_or(self.font_size);
            self.pane_font.insert(p, (cur + wheel.signum()).clamp(6.0, 40.0));
            if let Some(loc) = self.dock.find_tab(&p) {
                self.dock.set_active_tab(loc);
            }
        } else if ctrl_wheel.abs() > 0.5 && !ctx.is_pointer_over_area() {
            // 포인터가 떠 있는 창(창 안에 띄우기 오버레이·메뉴 등) 위면 전역 줌을 막는다
            // — 오버레이가 자기 pane만 줌하므로 뒤 도크까지 함께 확대되지 않게(P3 누수 수정).
            self.set_font_size(self.font_size + ctrl_wheel.signum());
        }
        // 포커스 pane 리사이즈 시 크기 배지를 잠시 띄운다(현대 터미널 관례).
        if let Some(g) = resized {
            self.resize_badge = Some((g, std::time::Instant::now()));
        }
        // 탭을 도크에서 빼 별도 OS 창(tear_off)·메인 창 내 오버레이(dock_float, P3)로. 닫으면 재도킹.
        if let Some(p) = tear_off.or(dock_float) {
            if let Some(idx) = self.dock.find_tab(&p) { self.dock.remove_tab(idx); }
            if dock_float == Some(p) { self.docked_float.push(p); } else { self.floating.push(p); }
        }
        self.open_terminal_pathline(); // 터미널 `파일:줄` 더블클릭 → 에디터로 점프(pending 처리).
        if let Some(p) = sftp_open {
            self.open_sftp_from_pane(p);
        }
        self.apply_browser_tab_acts(ctx, browser_act, browser_closed);
        self.apply_editor_tab_acts(editor_act, editor_closed);
        // 도크 에디터가 연 nabiPad 설정 창은 메인 ctx에 렌더(분리 창은 floating_editor에서 vctx).
        if self.editor_settings_for.is_some_and(|p| !self.floating.contains(&p)) {
            self.render_editor_settings(ctx);
        }
        self.apply_pending_focus(); // 비활성 pane 우클릭 포커스 요청 적용(첫 우클릭=활성화).
        // 탭바 빈 공간 우클릭 메뉴(그룹별 띠 감지 + 해당 그룹 포커스).
        self.detect_tabbar_menu(ctx);
        self.show_tabbar_menu(ctx);
        // 활성 원격 탭의 액션 처리.
        if self.sftp_pane.is_some() {
            if let Some(r) = sftp_act.rect {
                self.drop_zones.push((crate::dnd::DropTarget::Sftp, r));
            }
            self.process_sftp_act(sftp_act, ctx);
        }
        // 원격 탭 ✕(활성 또는 배경) 정리.
        if let Some(pane) = sftp_closed {
            self.close_remote_tab(pane);
        }
        // 탭 바 "+" 버튼/빈 화면 버튼 → 새 로컬 탭.
        if self.add_requested || spawn_empty {
            self.add_requested = false;
            // "+"가 눌린 분할을 포커스로 → 이어지는 add_pane이 활성 탭이 아닌 거기에 생성.
            if let Some(t) = self.add_target.take() { self.dock.set_focused_node_and_surface(t); }
            self.spawn_local(crate::workspace::shell_from_str(&self.config.terminal.default_shell));
        }
        if connect_empty {
            self.open_quick_connect();
        }
        if let Some(s) = pick {
            self.connect_saved(s);
        }
        // ssh:// 링크 Ctrl+클릭 → Quick Connect 프리필.
        if let Some(rest) = self.pending_ssh.take() {
            self.connect_ssh_url(&rest);
        }
        // 파일 참조/경로 Ctrl+클릭 → nabiPad(해당 줄) 또는 OS 기본 앱.
        if let Some((pane, url)) = self.pending_link.take() { self.open_term_link(pane, &url); }
    }
}
