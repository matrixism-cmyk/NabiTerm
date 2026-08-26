//! 설정(Preferences) 다이얼로그 — config.toml을 편집·영속·즉시 적용.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn show_settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        // 창이 막 열렸으면 현재 설정을 스냅샷(취소 시 되돌리기).
        if self.settings_backup.is_none() {
            self.settings_backup = Some(self.config.clone());
            self.settings_editor_backup = Some(self.editor_config.clone());
            self.settings_live_font = self.config.appearance.font_family.clone();
        }
        let lang = self.lang;
        let mut open = true;
        let mut save = false;
        let mut reset = false;
        let mut close = false;
        {
            let policy = self.control_policy.clone();
            let font_installer = self.font_installer.clone();
            // 페어링 목록은 페이지 클로저와 cfg 가변 차용이 겹쳐 RefCell로 잠깐 옮겼다 되돌린다.
            let tg_pending = std::cell::RefCell::new(std::mem::take(&mut self.telegram_pending));
            let sched = std::cell::RefCell::new(std::mem::take(&mut self.schedules));
            let sched_path = self.schedules_path.clone();
            let page_cx = crate::settingsui::PageCtx {
                policy: &policy,
                font_installer: &font_installer,
                tg_pending: &tg_pending,
                sched: &sched,
                sched_path: &sched_path,
            };
            let cfg = &mut self.config;
            let editor_cfg = &mut self.editor_config;
            // 콘텐츠에 맞는 고정 크기 모달(전체 화면 덮기 → 우측 여백만 남던 문제 해소).
            let screen = ctx.content_rect();
            let size = egui::vec2(840.0, 600.0).min(screen.size() - egui::vec2(60.0, 48.0));
            let win = egui::Rect::from_center_size(screen.center(), size);
            // 선택된 카테고리(프레임 간 유지).
            let cat_id = egui::Id::new("settings_cat");
            let mut cat = ctx.data(|d| d.get_temp::<usize>(cat_id)).unwrap_or(0);
            egui::Window::new(tr(lang, "settings.title"))
                .open(&mut open)
                .collapsible(false)
                .fixed_rect(win)
                .show(ctx, |ui| {
                    // 검색 줄 — 60항목을 여섯 페이지에서 눈으로 훑지 않아도 되게(M1).
                    let q_id = egui::Id::new("settings_query");
                    let mut query: String = ui.data(|d| d.get_temp(q_id)).unwrap_or_default();
                    if let Some(p) = crate::settingsearchui::bar(ui, lang, &mut query) {
                        cat = p;
                        query.clear(); // 갔으면 결과 목록은 치운다.
                    }
                    ui.data_mut(|d| d.insert_temp(q_id, query));
                    ui.separator();
                    // 좌측 카테고리 내비게이션 + 우측 콘텐츠(현대식 설정 레이아웃).
                    let body_h = (ui.available_height() - 44.0).max(200.0);
                    ui.horizontal_top(|ui| {
                        ui.set_min_height(body_h);
                        ui.vertical(|ui| {
                            ui.set_width(160.0);
                            for (i, key) in crate::settingsui::PAGE_KEYS.iter().enumerate() {
                                let lbl = egui::Button::selectable(cat == i, tr(lang, key));
                                if ui.add_sized([150.0, 26.0], lbl).clicked() {
                                    cat = i;
                                }
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("settings_scroll")
                                .auto_shrink([false, false])
                                .max_height(body_h)
                                .show(ui, |ui| {
                                    ui.heading(tr(lang, crate::settingsui::PAGE_KEYS[cat]));
                                    ui.add_space(8.0);
                                    crate::settingsui::page(ui, cfg, editor_cfg, lang, cat, &page_cx);
                                });
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "settings.save")).clicked() {
                            save = true;
                        }
                        if ui.button(tr(lang, "settings.reset")).clicked() {
                            reset = true;
                        }
                        if ui.button(tr(lang, "qc.cancel")).clicked() {
                            close = true;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                });
            ctx.data_mut(|d| d.insert_temp(cat_id, cat));
            self.telegram_pending = tg_pending.into_inner(); // 편집(승인/거부) 반영해 되돌린다.
            self.schedules = sched.into_inner();
        }
        if reset {
            // 기본값으로 되돌리되 디스크 저장은 보류 — 미리보기로 보여주고 저장은 Save에서.
            self.config = nabi_config::AppConfig::default();
            self.editor_config = nabi_config::EditorConfig::default();
        }
        // 매 프레임 실시간 미리보기: 글꼴·테마·언어를 앱 전체에 즉시 반영(저장은 안 함).
        self.live_apply_settings(ctx);
        if save {
            self.apply_settings(ctx); // 디스크 저장 + 인코딩/항상위 등 전체 적용.
            self.settings_backup = None;
            self.settings_editor_backup = None;
            self.settings_open = false;
            return;
        }
        // 창 X·Cancel·Esc는 모두 취소 — 스냅샷으로 되돌리고 미리보기 변경을 원복.
        if close || !open {
            if let Some(backup) = self.settings_backup.take() {
                self.config = backup;
            }
            if let Some(backup) = self.settings_editor_backup.take() {
                self.editor_config = backup;
            }
            self.live_apply_settings(ctx);
            self.settings_open = false;
        }
    }

    /// 설정 창이 열린 동안의 가벼운 실시간 적용(디스크 저장 없음).
    /// 글꼴 파일은 경로가 바뀔 때만 재설치(매 프레임 reload 회피).
    fn live_apply_settings(&mut self, ctx: &egui::Context) {
        self.font_size = self.config.appearance.font_size;
        self.lang = nabi_i18n::Lang::from_code(&self.config.appearance.language);
        self.theme = build_theme(&self.config);
        // 사용자 링크 규칙을 렌더러에 넣는다 — 설정에서 고치면 그 자리에서 반영된다.
        nabi_render::urlrules::set_rules(&self.config.terminal.link_rules);
        let fam = self.config.appearance.font_family.clone();
        if fam != self.settings_live_font {
            self.settings_live_font = fam.clone();
            crate::fonts::install_cjk_fonts(ctx, &fam);
        }
    }

    fn apply_settings(&mut self, ctx: &egui::Context) {
        let _ = nabi_config::save(&self.config_path, &self.config);
        // SSH keepalive 간격을 라이브 반영(다음 연결부터 적용).
        nabi_ssh::session::SSH_KEEPALIVE_SECS.store(self.config.terminal.ssh_keepalive_secs, std::sync::atomic::Ordering::Relaxed);
        nabi_ssh::conntimeout::CONNECT_TIMEOUT_SECS.store(self.config.terminal.ssh_connect_timeout_secs, std::sync::atomic::Ordering::Relaxed);
        // 팔레트도 같은 길로 — `ansi16` 한 곳이 이 값을 본다.
        nabi_types::palette::set_active(nabi_types::palette::Palette::from_name(&self.config.appearance.palette));
        crate::egress::set_offline(self.config.terminal.offline_mode);
        // SFTP 해시 검증 스위치 — 워커 풀 연결까지 전역으로 즉시 적용.
        nabi_sftp::SFTP_VERIFY_HASH.store(self.config.terminal.sftp_verify_hash, std::sync::atomic::Ordering::Relaxed);
        nabi_sftp::set_name_charset(&self.config.terminal.sftp_name_charset); // 파일명 인코딩 라이브 반영.
        nabi_sftp::set_upload_mode(&self.config.terminal.sftp_upload_mode); // 업로드 권한 정규화.
        // 팁 번역 캐시 경로 변경을 즉시 반영(다른 파일이면 새로 읽는다).
        let base = self.config_path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        self.tip_ai.retarget(&base, &self.config.terminal.tip_cache_path);
        // nabiPad 설정은 분리 파일(nabipad.toml)에 별도 저장(독립 프로그램화 대비).
        let _ = nabi_config::save(&self.editor_config_path, &self.editor_config);
        // 구문 강조 테마·확장자 매핑을 라이브 반영(다음 하이라이트부터 적용).
        crate::editorsyntax::set_theme(self.editor_config.theme.clone());
        crate::editorsyntax::set_ext_map(self.editor_config.ext_map.clone());
        nabi_editor::editortext::set_wrap_col(self.editor_config.wrap_col); // 줄바꿈 칸 수.
        // 탭 폭·들여쓰기는 이미 열린 편집 버퍼에도 바로 반영한다(재시작/재열기 불필요).
        let (tab, spaces) = (self.editor_config.tab_size.max(1), self.editor_config.indent_spaces);
        for d in self.editors.values_mut() {
            if let Some(eb) = d.edit.as_mut() {
                (eb.tab, eb.spaces, eb.seen_cols) = (tab, spaces, 0); // 탭 폭이 바뀌면 가로 범위도 다시.
            }
        }
        // 에이전트 제어 모드(off/ask/on)를 라이브 정책에 즉시 반영 — 안 하면 설정을 "켬"으로
        // 바꿔도 재시작 전까지 계속 승인을 요구한다(버그 수정).
        self.control_policy
            .set_mode(nabi_control::policy::Mode::parse(&self.config.terminal.control_mode));
        // 글꼴 파일 변경을 즉시 반영(재시작 불필요).
        crate::fonts::install_cjk_fonts(ctx, &self.config.appearance.font_family);
        self.font_size = self.config.appearance.font_size;
        self.lang = nabi_i18n::Lang::from_code(&self.config.appearance.language);
        self.theme = build_theme(&self.config);
        self.always_on_top = self.config.appearance.always_on_top;
        self.pending_on_top = Some(self.always_on_top);
        // 인코딩 변경을 기존 pane에도 즉시 적용.
        let enc = self.config.terminal.encoding.clone();
        let mut panes: Vec<_> = self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
        panes.extend(self.floating.iter().copied());
        for pane in panes {
            self.orch
                .send(nabi_proto::Command::SetEncoding { pane, label: enc.clone() });
        }
    }
}

/// config로부터 Theme(프리셋 + 커서 모양/색 + 선택 색)를 구성한다(new·apply 공용).
pub(crate) fn build_theme(cfg: &nabi_config::AppConfig) -> nabi_vt::Theme {
    let a = &cfg.appearance;
    let mut t = nabi_vt::Theme::preset(&a.theme);
    t.cursor_shape = nabi_vt::CursorShape::from_label(&a.cursor_shape);
    t.cursor_color = nabi_types::Rgba::from_hex(&a.cursor_color);
    if let Some(c) = nabi_types::Rgba::from_hex(&a.selection_color) {
        t.sel_color = c;
    }
    if let Some(c) = nabi_types::Rgba::from_hex(&a.match_color) {
        t.match_color = c;
    }
    if let Some(c) = nabi_types::Rgba::from_hex(&a.fg_color) {
        t.fg = c;
    }
    if let Some(c) = nabi_types::Rgba::from_hex(&a.bg_color) {
        t.bg = c;
    }
    t
}
