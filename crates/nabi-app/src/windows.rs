//! 탭을 별도 OS 창(egui 멀티 뷰포트)으로 분리한다.
//!
//! deferred 뷰포트 클로저는 Send+Sync+'static이어야 하므로, 공유 상태(Arc/채널)만
//! 캡처한다. pane 상태는 오케스트레이터 SharedPanes에서 읽고, 입력은 cmd_tx로 보낸다.

use crate::app::NabiApp;
use nabi_types::PaneId;

impl NabiApp {
    pub(crate) fn show_floating(&mut self, ctx: &egui::Context) {
        // immediate 뷰포트로 그려 앱 전체 상태(&mut self)에 접근 → 메뉴바·SFTP까지 렌더한다.
        let floating = self.floating.clone();
        let on_top = self.floating_on_top; // B8: 분리 창 항상 위 고정.
        for pane in floating {
            let closed = self.close_signal.clone();
            // 에디터 창은 nabiPad 브랜딩, 그 외(터미널/SFTP)는 nabiTerm.
            let win_title = if let Some(e) = self.editors.get(&pane) {
                let star = if e.dirty { "*" } else { "" };
                format!("nabiPad — {}{star}", e.title)
            } else {
                let title = self
                    .orch
                    .panes
                    .read()
                    .ok()
                    .and_then(|m| m.get(&pane).map(|v| v.title.clone()))
                    .unwrap_or_default();
                format!("nabiTerm — {title}")
            };
            // 기하는 첫 프레임에만 적용한다(매 프레임 with_inner_size를 다시 주면 테두리만큼
            // 창이 계속 커지므로). 이후엔 OS/사용자가 크기·위치를 제어한다.
            let mut builder = egui::ViewportBuilder::default().with_title(win_title);
            if self.floating_shown.insert(pane) {
                // 저장된 위치·크기가 유효하면 그 자리/크기로 복원(P10), 아니면 기본 크기. 좌표 검증으로 네이티브 크래시 방지.
                let geom = self.floating_geom.get(&pane).copied().filter(|g| {
                    g.iter().all(|v| v.is_finite())
                        && (200.0..=20_000.0).contains(&g[2])
                        && (120.0..=20_000.0).contains(&g[3])
                        && (-10_000.0..=40_000.0).contains(&g[0])
                        && (-10_000.0..=40_000.0).contains(&g[1])
                });
                builder = match geom {
                    Some(g) => builder.with_position([g[0], g[1]]).with_inner_size([g[2], g[3]]),
                    None => builder.with_inner_size(crate::floatsize::first_size(
                        self.editors.contains_key(&pane),
                        ctx.input(|i| i.viewport().monitor_size),
                    )),
                };
            }
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("nabi-float", pane.get())),
                builder,
                |ui, _class| {
                    let vctx = &ui.ctx().clone();
                    // 항상 위 고정 토글(매 프레임 idempotent하게 적용 — 라이브 반영).
                    vctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        if on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal },
                    ));
                    self.floating_body(ui, pane); // 터미널 또는 SFTP 패널(메뉴바는 메인 전용).
                    // 현재 창 위치(outer.min)·내부 크기(inner)를 기억(저장·재오픈 복원용, P10).
                    // 크기는 inner_rect라야 with_inner_size 복원과 일치(outer면 테두리만큼 어긋남).
                    if let (Some(o), Some(r)) =
                        (vctx.input(|i| i.viewport().outer_rect), vctx.input(|i| i.viewport().inner_rect))
                    {
                        self.floating_geom.insert(pane, [o.min.x, o.min.y, r.width(), r.height()]);
                    }
                    if vctx.input(|i| i.viewport().close_requested()) {
                        // 미저장 에디터는 OS 닫기를 취소하고 확인 모달을 띄운다(이 창 안에서).
                        if self.editors.get(&pane).map(|e| e.dirty).unwrap_or(false) {
                            vctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                            self.editor_close_ask = Some(pane);
                        } else if let Ok(mut c) = closed.lock() {
                            c.push(pane);
                        }
                    }
                    // 분리 에디터의 닫기 확인 모달은 이 viewport(vctx)에서 그려 최상위로(메인 창 뒤로 안 가게).
                    if self.editor_close_ask == Some(pane) {
                        self.render_editor_close_confirm(vctx, pane);
                    }
                    // 이 창에서 시작된 붙여넣기 확인도 여기서 그린다. 메인 창에만 그리면
                    // 분리 창을 보고 있는 사용자에게는 아무 일도 안 일어난 것처럼 보인다.
                    if self.pending_paste.as_ref().is_some_and(|(p, _)| *p == pane) {
                        self.show_paste_confirm(vctx);
                    }
                    // 위험 명령 확인도 같은 이유로 이 창에 그린다. 붙잡아 놓고 물어보는
                    // 창이 뒤에 있으면, 사용자에게는 엔터가 먹히지 않는 것으로 보인다.
                    if self.pending_send.as_ref().is_some_and(|p| p.pane == pane) {
                        self.show_guard(vctx);
                    }
                },
            );
        }

        // 닫힌 창의 pane을 메인 도크로 되돌린다.
        let returned: Vec<PaneId> = self
            .close_signal
            .lock()
            .map(|mut c| c.drain(..).collect())
            .unwrap_or_default();
        for p in returned {
            self.floating.retain(|x| *x != p);
            self.floating_shown.remove(&p); // 재오픈 시 저장된 기하를 다시 적용하도록.
            // 에디터 창을 닫으면 문서를 닫는다(재도킹 아님). 그 외는 메인 도크로 복귀.
            if self.editors.contains_key(&p) {
                self.editors.remove(&p);
            } else {
                self.add_pane(p);
            }
        }
    }

    /// 분리 창 본문: SFTP=파일브라우저, 에디터=내장 에디터, 아니면 터미널.
    fn floating_body(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let vctx = &ui.ctx().clone();
        if Some(pane) == self.sftp_pane || self.sftp_bg.contains_key(&pane) {
            self.floating_sftp(ui, pane);
            return;
        }
        if self.editors.contains_key(&pane) {
            self.floating_editor(ui, pane);
            return;
        }
        if self.browser_tabs.contains_key(&pane) {
            self.floating_browser(ui, pane);
            return;
        }
        if self.web_tabs.contains_key(&pane) {
            self.floating_web(ui, pane);
            return;
        }
        let font_size = self.pane_font.get(&pane).copied().unwrap_or(self.font_size);
        let blink_on = self.blink_on();
        let find = self.find_highlight(); // 탭과 같은 규칙(정규식·단어 단위는 강조 생략).
        let mut zoom = None;
        let fk = self.wheel_keys_effective(pane); // 가변 차용 전에 계산(차용 충돌 회피).
        // 명령 바가 쓰는 설정 문자열도 미리 복사한다(아래에서 self를 가변 차용하므로).
        let (ai_last_model, ai_last_effort) = (
            self.config.terminal.ai_last_model.clone(),
            self.config.terminal.ai_last_effort.clone(),
        );
        // 이 창의 위험 표식(운영/스테이징) — 표식은 창이 아니라 세션에 붙는다.
        let risky = self.risky_set(pane);
        let guard_on = self.config.terminal.guard_dangerous;
        crate::floatterm::render_floating(
            ui,
            &self.orch.panes,
            &self.orch.cmd_tx,
            &self.floating_grid,
            pane,
            font_size,
            &self.theme,
            self.broadcast,
            find.as_deref(),
            blink_on,
            &mut self.floating_link,
            &mut zoom,
            &mut self.pending_link,
            &mut self.paste_req,
            self.config.terminal.warn_paste_newline,
            fk,
            &mut self.tui_overlay,
            &mut crate::aicmdbar::AiBarState {
                enabled: self.config.terminal.ai_cmd_bar,
                run_cmd: &self.run_cmd,
                pane_status: &self.pane_status,
                picks: &mut self.ai_picks,
                screen: &mut self.ai_screen,
                last_model: &ai_last_model,
                last_effort: &ai_last_effort,
                pick_out: &mut self.ai_pick_out,
            },
            &mut crate::tipoverlay::TipState {
                enabled: self.config.terminal.tip_overlay,
                ai_on: self.config.terminal.tip_translate_ai,
                cache: &mut self.tip_cache,
                ai: &mut self.tip_ai,
            },
            self.lang,
            &self.trzsz,
            &mut crate::floatparity::FloatParity {
                sel: &mut self.selection,
                pathline: &mut self.pending_pathline,
                keywords: &self.config.terminal.highlight_keywords,
                copy_on_select: self.config.appearance.copy_on_select,
                guard_on,
                risky: &risky,
                pending_send: &mut self.pending_send,
            },
        );
        if let Some((p, d)) = zoom {
            self.zoom_pane(p, d);
        }
        self.schedule_next_frame(vctx); // 이 분리 창도 필요한 시점에만 깨운다.
        self.show_floating_link_menu(vctx); // 링크 길게누르기 팝업(P2).
    }

    /// 분리/오버레이 창의 Ctrl+휠 확대/축소 — 그 pane의 글꼴만 조정한다(뒤 창으로 안 샘).
    fn zoom_pane(&mut self, p: PaneId, delta: f32) {
        let cur = self.pane_font.get(&p).copied().unwrap_or(self.font_size);
        self.pane_font.insert(p, (cur + delta).clamp(6.0, 40.0));
    }

    /// "창 안에 띄우기"(P3) — 메인 창 안에 떠 있는 오버레이로 터미널 pane을 렌더한다.
    /// egui::Window는 독립 Area 레이어라 입력 가림이 정확(egui_dock Eject 누수 문제 해결).
    /// 창 닫기(X) → 메인 도크로 재도킹(닫기=종료 아님).
    pub(crate) fn show_docked_floats(&mut self, ctx: &egui::Context) {
        if self.docked_float.is_empty() {
            return;
        }
        let blink_on = self.blink_on();
        let find = self.find_highlight();
        let mut closed = Vec::new();
        for pane in self.docked_float.clone() {
            let title = self
                .orch
                .panes
                .read()
                .ok()
                .and_then(|m| m.get(&pane).map(|v| v.title.clone()))
                .unwrap_or_default();
            let font_size = self.pane_font.get(&pane).copied().unwrap_or(self.font_size);
            let mut open = true;
            let fk = self.wheel_keys_effective(pane); // 가변 차용 전에 계산.
            let mut zoom = None;
            // 명령 바가 쓰는 설정 문자열 사전 복사(아래 클로저에서 self를 가변 차용).
            let (ai_last_model, ai_last_effort) = (
                self.config.terminal.ai_last_model.clone(),
                self.config.terminal.ai_last_effort.clone(),
            );
            let risky = self.risky_set(pane);
            let guard_on = self.config.terminal.guard_dangerous;
            egui::Window::new(format!("\u{2750} {title}"))
                .id(egui::Id::new(("nabi_docked_float", pane.get())))
                .open(&mut open)
                .resizable(true)
                .default_size([700.0, 440.0])
                .min_width(260.0)
                .min_height(160.0)
                .collapsible(true)
                .show(ctx, |ui| {
                    crate::floatterm::paint_floating_term(
                        ui,
                        &self.orch.panes,
                        &self.orch.cmd_tx,
                        &self.floating_grid,
                        pane,
                        font_size,
                        &self.theme,
                        self.broadcast,
                        find.as_deref(),
                        blink_on,
                        &mut self.floating_link,
                        &mut zoom,
                        &mut self.pending_link,
                        &mut self.paste_req,
                        self.config.terminal.warn_paste_newline,
                        fk,
                        &mut self.tui_overlay,
                        &mut crate::aicmdbar::AiBarState {
                            enabled: self.config.terminal.ai_cmd_bar,
                            run_cmd: &self.run_cmd,
                            pane_status: &self.pane_status,
                            picks: &mut self.ai_picks,
                            screen: &mut self.ai_screen,
                            last_model: &ai_last_model,
                            last_effort: &ai_last_effort,
                            pick_out: &mut self.ai_pick_out,
                        },
                        &mut crate::tipoverlay::TipState {
                            enabled: self.config.terminal.tip_overlay,
                            ai_on: self.config.terminal.tip_translate_ai,
                            cache: &mut self.tip_cache,
                            ai: &mut self.tip_ai,
                        },
                        self.lang,
                        &self.trzsz,
                        &mut crate::floatparity::FloatParity {
                            sel: &mut self.selection,
                            pathline: &mut self.pending_pathline,
                            keywords: &self.config.terminal.highlight_keywords,
                            copy_on_select: self.config.appearance.copy_on_select,
                            guard_on,
                            risky: &risky,
                            pending_send: &mut self.pending_send,
                        },
                    );
                });
            if let Some((p, d)) = zoom {
                self.zoom_pane(p, d);
            }
            if !open {
                closed.push(pane);
            }
        }
        for p in closed {
            self.docked_float.retain(|x| *x != p);
            self.add_pane(p); // 닫으면 재도킹(종료 아님).
        }
        self.show_floating_link_menu(ctx); // 오버레이 내 링크 팝업도 메인 ctx에 렌더.
    }
}

