//! 탭을 별도 OS 창(egui 멀티 뷰포트)으로 분리한다.
//!
//! deferred 뷰포트 클로저는 Send+Sync+'static이어야 하므로, 공유 상태(Arc/채널)만
//! 캡처한다. pane 상태는 오케스트레이터 SharedPanes에서 읽고, 입력은 cmd_tx로 보낸다.

use crate::app::NabiApp;
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::Command;
use nabi_types::{GridSize, PaneId};
use nabi_vt::Theme;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
                    None => builder.with_inner_size([820.0, 540.0]),
                };
            }
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("nabi-float", pane.get())),
                builder,
                |vctx, _class| {
                    // 항상 위 고정 토글(매 프레임 idempotent하게 적용 — 라이브 반영).
                    vctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        if on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal },
                    ));
                    self.floating_body(vctx, pane); // 터미널 또는 SFTP 패널(메뉴바는 메인 전용).
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
    fn floating_body(&mut self, vctx: &egui::Context, pane: PaneId) {
        if Some(pane) == self.sftp_pane || self.sftp_bg.contains_key(&pane) {
            self.floating_sftp(vctx, pane);
            return;
        }
        if self.editors.contains_key(&pane) {
            self.floating_editor(vctx, pane);
            return;
        }
        if self.browser_tabs.contains_key(&pane) {
            self.floating_browser(vctx, pane);
            return;
        }
        let font_size = self.pane_font.get(&pane).copied().unwrap_or(self.font_size);
        let blink_on = self.blink_on();
        let find = if self.find_open && !self.find_query.is_empty() {
            Some(self.find_query.clone())
        } else {
            None
        };
        let mut zoom = None;
        render_floating(
            vctx,
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
        let find = (self.find_open && !self.find_query.is_empty()).then(|| self.find_query.clone());
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
            let mut zoom = None;
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

/// 분리 창(별도 OS 창)에서 터미널 pane을 렌더한다 — CentralPanel로 감싼 thin 래퍼.
#[allow(clippy::too_many_arguments)]
fn render_floating(
    ctx: &egui::Context,
    panes: &SharedPanes,
    cmd_tx: &Sender<Command>,
    grids: &Arc<Mutex<HashMap<PaneId, GridSize>>>,
    pane: PaneId,
    font_size: f32,
    theme: &Theme,
    broadcast: bool,
    find: Option<&str>,
    blink_on: bool,
    link: &mut Option<(String, egui::Pos2)>,
    zoom: &mut Option<(PaneId, f32)>,
    link_click: &mut Option<(PaneId, String)>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        crate::floatterm::paint_floating_term(
            ui, panes, cmd_tx, grids, pane, font_size, theme, broadcast, find, blink_on, link, zoom, link_click,
        );
    });
    // 재그리기 예약은 호출측(floating_body)이 메인 창과 같은 규칙으로 한다.
    // 여기서 무조건 request_repaint()를 걸면 이 뷰포트가 영원히 최대 프레임으로 돌아
    // 코어 하나를 계속 먹는다(출력이 없어도).
}

