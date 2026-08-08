//! 터미널 pane 렌더/입력 처리 — TermTabViewer::ui 본문(라인 한도로 tabs.rs에서 분리).

use crate::paneio::mouse_reports;
use crate::tabs::TermTabViewer;
use bytes::Bytes;
use nabi_proto::Command;
use nabi_types::{GridSize, PaneId};

impl TermTabViewer<'_> {
    /// 한 터미널 pane을 렌더하고 입력/마우스/선택/스크롤을 처리한다.
    pub(crate) fn paint_term(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let is_focused = self.focused == Some(pane); // 분할 시 입력/스크롤은 포커스 pane에만.
        let font =
            egui::FontId::monospace(self.pane_font.get(&pane).copied().unwrap_or(self.font_size));
        let (cw, ch) = nabi_render::cell_size(ui, &font);
        let rect = ui.available_rect_before_wrap().shrink(4.0); // 내부 여백.
        let grid = GridSize::new(
            (rect.width() / cw).floor().max(1.0) as u16,
            (rect.height() / ch).floor().max(1.0) as u16,
        );
        let prev_grid = self.last_grid.get(&pane).copied();
        if prev_grid != Some(grid) {
            self.last_grid.insert(pane, grid);
            self.orch.send(Command::Resize { pane, size: grid });
            // 최초 배치가 아닌 실제 리사이즈일 때만 포커스 pane 크기 배지를 띄운다.
            if is_focused && prev_grid.is_some() {
                *self.resized = Some(grid);
            }
        }

        let (mut scroll, to_top, to_bottom) = if is_focused {
            crate::paneio::read_scroll_keys(ui, grid.rows() as i32)
        } else {
            (0, false, false)
        };

        let events = ui.input(|i| i.events.clone());
        let (app_cursor, bracketed, mouse_on, mouse_release, mouse_sgr, mouse_motion) = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).map(|v| v.model.clone()))
            .and_then(|model| {
                model.lock().ok().map(|md| {
                    (
                        md.app_cursor(),
                        md.bracketed_paste(),
                        md.mouse_on(),
                        md.mouse_wants_release(),
                        md.mouse_sgr(),
                        md.mouse_wants_motion(),
                    )
                })
            })
            .unwrap_or((false, false, false, false, false, false));
        // 조합 초기 상태 = 직전 프레임 조합 유지 여부. 이번 프레임의 Preedit/Commit은
        // events_to_bytes가 '순서대로' 반영한다(Commit 직후 같은 프레임의 Enter는 PTY로 감).
        let composing = is_focused && !self.ime_preedit.is_empty();
        let bytes = nabi_render::events_to_bytes(&events, app_cursor, bracketed, composing);
        // IME 조합 상태는 포커스 pane만 추적(한글 초성·중성·종성 진행을 커서에 표시).
        if is_focused && nabi_render::update_preedit(&events, self.ime_preedit) {
            ui.ctx().request_repaint();
        }
        // 터미널 활성 시 보이지 않는 포커스 싱크를 잡아 Tab/화살표/Esc가 egui 포커스 이동에
        // 쓰이지 않고 모두 PTY로 가게 한다(Tab=셸 자동완성). 싱크는 blocked 판정에서 제외.
        crate::paneio::grab_term_focus(ui, is_focused);
        let blocked = crate::paneio::term_input_blocked(ui.ctx());
        let typed = is_focused && !bytes.is_empty() && !blocked;
        if typed {
            if self.broadcast {
                if self.broadcast_group.is_empty() {
                    // 그룹 미지정 = "이 창의 터미널 전부". 테두리로 표시하는 대상과 정확히 일치시킨다.
                    self.orch.send(Command::Broadcast {
                        panes: self.window_panes.iter().copied().collect(),
                        data: Bytes::from(bytes),
                    });
                } else {
                    for p in self.broadcast_group.iter() {
                        self.orch.send(Command::WriteInput {
                            pane: *p,
                            data: Bytes::from(bytes.clone()),
                        });
                    }
                }
            } else {
                self.orch.send(Command::WriteInput {
                    pane,
                    data: Bytes::from(bytes),
                });
            }
        }

        // Ctrl+휠 폰트 확대축소는 마우스 리포팅 모드(Claude Code·vim·less 등 TUI)보다 우선한다.
        // 주류 에뮬레이터(Windows Terminal·iTerm2)처럼 에뮬레이터 레벨에서 가로채 앱엔 보내지
        // 않는다 — 휠을 소비해 아래 마우스 보고/스크롤백 경로로 새지 않게 한다.
        let over = ui.rect_contains_pointer(rect);
        let ctrl_wheel = over && ui.input(|i| i.modifiers.command && i.raw_scroll_delta.y != 0.0);
        if ctrl_wheel {
            // 포커스 여부와 무관하게 포인터가 올라간 이 pane을 확대/축소(+포커스).
            let wheel = ui.input(|i| i.raw_scroll_delta.y);
            *self.zoom_req = Some((pane, wheel));
            ui.input_mut(|i| {
                i.raw_scroll_delta = egui::Vec2::ZERO;
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }

        if mouse_on {
            let rep = mouse_reports(ui, rect, cw, ch, mouse_sgr, mouse_release, mouse_motion);
            if !rep.is_empty() {
                self.orch.send(Command::WriteInput {
                    pane,
                    data: Bytes::from(rep),
                });
            }
        } else if over {
            let wheel = ui.input(|i| i.raw_scroll_delta.y);
            if wheel != 0.0 {
                // 스크롤백 휠 속도 ×3(기본 1줄/notch는 너무 느림). 최소 1줄은 움직이게.
                let lines = (wheel / ch * 3.0).round() as i32;
                scroll += if lines == 0 { wheel.signum() as i32 } else { lines };
            }
        }

        if !mouse_on {
            // 분할에서 비활성 pane을 우클릭하면 첫 클릭은 활성화만(값이 활성창으로 새지 않게).
            let sec_here = ui.rect_contains_pointer(rect) && ui.input(|i| i.pointer.secondary_clicked());
            if sec_here && !is_focused {
                *self.focus_req = Some(pane);
            } else if let Some(t) = crate::paneio::right_click_paste_text(ui, rect) {
                // 곧장 보내지 않는다 — 여러 줄 확인을 거치도록 app으로 넘긴다.
                *self.paste_req = Some((pane, t));
            }
        }

        let pane_model = self.orch.panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.model.clone()));
        if let Some(pane_model) = pane_model {
            if let Ok(mut model) = pane_model.lock() {
                if typed || to_bottom {
                    model.scroll_to_bottom();
                } else if to_top {
                    model.scroll_to_top();
                } else if !mouse_on && scroll != 0 && !model.alt_screen() {
                    model.scroll_by(scroll);
                }
                // 텍스트 선택 추적(track_selection 내부에서 시각열→render 인덱스 변환=와이드 보정).
                // 링크 메뉴가 열려 있으면 드래그로 덮어쓰지 않고 현재 선택(링크 전체)을 유지한다.
                // 스크롤바(우측) 위에서는 선택 대신 스크롤이 동작하도록 선택을 억제한다.
                let menu_open = self.link_menu.is_some();
                let over_sb = !model.alt_screen()
                    && model.history_size() > 0
                    && crate::scrollbar::over_scrollbar(rect, ui.input(|i| i.pointer.interact_pos()));
                // 포인터가 이 pane 위에 있지만 더 위 레이어(분리 창)가 가린 경우(#5),
                // press가 pane 밖(탭 제목 DnD 등)에서 시작(#4), 또는 press가 더 위 레이어(오버레이 창·
                // 그 리사이즈 핸들)에서 시작한 경우 → 새 선택 억제. press 시작 레이어로 판정하면
                // 드래그 중 포인터가 창 밖으로 나가도 뒤 pane을 선택하지 않는다.
                let occluded = ui.input(|i| i.pointer.interact_pos())
                    .is_some_and(|p| rect.contains(p) && ui.ctx().layer_id_at(p) != Some(ui.layer_id()));
                let press = ui.input(|i| i.pointer.press_origin());
                let press_outside = press.is_some_and(|p| !rect.contains(p));
                let press_on_layer = press.is_some_and(|p| ui.ctx().layer_id_at(p) != Some(ui.layer_id()));
                let (mut sel_norm, do_copy, mut sel_rect) = if mouse_on || over_sb {
                    (None, false, false)
                } else if menu_open || occluded || press_outside || press_on_layer {
                    // 기존 선택은 유지(덮어쓰기/확장만 막는다).
                    let s = (*self.selection).filter(|s| s.pane == pane && !s.is_empty());
                    (s.map(|s| s.span()), false, s.is_some_and(|s| s.rect))
                } else {
                    crate::selection::track_selection(ui, rect, cw, ch, pane, &model, self.selection)
                };
                let focused = ui.ctx().input(|i| i.focused);
                // 조합 텍스트는 포커스된 이 pane에서만 표시(다른 pane은 빈 문자열).
                let preedit = if is_focused { self.ime_preedit.as_str() } else { "" };
                // 링크 길게 누르기: 메뉴를 띄우기 '전에' 링크 전체를 선택해 그 프레임부터 보여준다.
                if let Some((sel, url, pos)) =
                    crate::paneurl::link_longpress(ui, rect, cw, ch, pane, &model, &self.theme)
                {
                    sel_norm = Some(sel.span());
                    sel_rect = sel.rect;
                    *self.selection = Some(sel);
                    *self.link_menu = Some((url, pos));
                }
                nabi_render::paint(
                    ui, rect, font, &model, &self.theme, self.find.as_deref(),
                    self.highlights, sel_norm, sel_rect, focused, self.blink_on, preedit,
                );
                model.set_cell_px(ch); // 이미지 높이→줄 변환 기준(폰트 줌 반영).
                self.draw_inline_images(ui, rect, ch, &model);
                crate::scrollbar::draw(ui, rect, pane, &mut model); // 우측 스크롤바(스크롤백 있을 때).
                if crate::paneio::draw_scroll_badge(ui, rect, model.scrollback_offset()) {
                    model.scroll_to_bottom();
                }
                // 브로드캐스트 안전 표시: 실제 입력을 받는 pane(그룹 비면 전체, 아니면 멤버)만 테두리.
                if self.broadcast && (self.broadcast_group.is_empty() || self.broadcast_group.contains(&pane)) {
                    ui.painter_at(rect).rect_stroke(
                        rect.shrink(1.0),
                        egui::Rounding::ZERO,
                        egui::Stroke::new(2.5, crate::theme_ui::BROADCAST),
                    );
                }
                if !mouse_on && !occluded && !press_on_layer && !menu_open {
                    if let Some(text) = crate::clicks::handle_click_select(
                        ui, rect, cw, ch, pane, &model, &self.theme, self.selection,
                    ) {
                        // 더블클릭한 토큰이 `파일:줄`이면 복사 대신 에디터로 점프(락 해제 후 처리).
                        let dbl = ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
                        match dbl.then(|| crate::pathline::parse_path_line(&text)).flatten() {
                            Some(pl) => *self.pending_pathline = Some(pl),
                            None => ui.ctx().copy_text(text),
                        }
                    }
                }
                if do_copy && self.copy_on_select {
                    if let Some((sr, sc, er, ec)) = sel_norm {
                        let rows = model.render_rows(&self.theme);
                        let wrapped: Vec<bool> =
                            (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
                        let text = crate::selection::extract_selection(
                            &rows, sr, sc, er, ec, &wrapped, sel_rect,
                        );
                        if !text.is_empty() {
                            ui.ctx().copy_text(text);
                        }
                    }
                }
                crate::paneurl::hover_url_cursor(ui, rect, cw, ch, &model, &self.theme);
                if let Some(url) = crate::paneurl::ctrl_click_url(ui, &model, &self.theme, rect, cw, ch) {
                    // ssh://는 Quick Connect, 그 외(파일참조·경로·URL)는 nabiPad/OS 분기로.
                    if let Some(rest) = url.strip_prefix("ssh://") {
                        *self.ssh_click = Some(rest.to_string());
                    } else {
                        *self.link_click = Some((pane, url));
                    }
                }
            }
        }
    }

    /// 인라인 이미지(Sixel)를 그 앵커 줄 위치에 그린다. 텍스처는 id별로 1회만 GPU 업로드.
    fn draw_inline_images(&mut self, ui: &egui::Ui, rect: egui::Rect, ch: f32, model: &nabi_vt::TermModel) {
        let imgs = model.visible_images();
        if imgs.is_empty() {
            return;
        }
        if self.img_textures.len() > 128 {
            self.img_textures.clear(); // 단순 상한(드물게 초과 시 재업로드).
        }
        let painter = ui.painter_at(rect);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        for (top, rows, id, img) in imgs {
            let tex = self.img_textures.entry(id).or_insert_with(|| {
                let ci = egui::ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &img.rgba,
                );
                ui.ctx().load_texture(format!("nabi_img_{id}"), ci, egui::TextureOptions::LINEAR)
            });
            let h = f32::from(rows) * ch;
            let w = ((img.width as f32) * (h / img.height as f32)).min(rect.width());
            let r = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + top as f32 * ch),
                egui::vec2(w, h),
            );
            painter.image(tex.id(), r, uv, egui::Color32::WHITE);
        }
    }
}
