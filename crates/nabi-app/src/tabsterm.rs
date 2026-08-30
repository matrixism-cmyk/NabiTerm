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
        // AI 명령 바: AI CLI 실행 중이면 pane 최상단에 슬래시 명령 버튼 줄(클릭=주입).
        // 터미널 rect 계산 '앞'에 그려 세로 공간을 차지한다(grid가 자동으로 줄어듦).
        self.ai_bar(ui, pane);
        // 전송 중이면 그 pane 위에 진행률 한 줄(터미널에서 시작한 일이라 눈이 여기 있다).
        if crate::trzszui::overlay(ui, self.lang, self.trzsz, pane) {
            self.orch.send(Command::TrzszCancel { pane });
        }
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

        // 키 입력은 포커스 pane만 수집·인코딩한다(비포커스 낭비 제거 — 성능 리뷰 2026-08-19).
        let events = crate::paneio::focused_events(ui, is_focused, pane);
        let (app_cursor, bracketed, mouse_on, mouse_release, mouse_sgr, mouse_motion, alt_screen, alt_scroll, kitty) = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).map(|v| v.model.clone()))
            .and_then(|model| {
                model.lock().ok().map(|md| {
                    (
                        md.app_cursor(), md.bracketed_paste(), md.mouse_on(),
                        md.mouse_wants_release(), md.mouse_sgr(), md.mouse_wants_motion(),
                        md.alt_screen(), md.alt_scroll(), md.kitty_keys(),
                    )
                })
            })
            .unwrap_or((false, false, false, false, false, false, false, false, 0));
        // 조합 초기 상태 = 직전 프레임 조합 유지 여부. 이번 프레임의 Preedit/Commit은
        // events_to_bytes가 '순서대로' 반영한다(Commit 직후 같은 프레임의 Enter는 PTY로 감).
        let composing = is_focused && !self.ime_preedit.is_empty();
        let bytes = nabi_render::events_to_bytes_kitty(&events, app_cursor, bracketed, composing, kitty);
        // IME 조합 상태는 포커스 pane만 추적(한글 초성·중성·종성 진행을 커서에 표시).
        if is_focused && nabi_render::update_preedit(&events, self.ime_preedit) {
            ui.ctx().request_repaint();
        }
        // 터미널 활성 시 보이지 않는 포커스 싱크를 잡아 Tab/화살표/Esc가 egui 포커스 이동에
        // 쓰이지 않고 모두 PTY로 가게 한다(Tab=셸 자동완성). 싱크는 blocked 판정에서 제외.
        crate::paneio::grab_term_focus(ui, is_focused);
        let blocked = crate::paneio::term_input_blocked(ui.ctx());
        let typed = is_focused && !bytes.is_empty() && !blocked;
        // 휠 도우미: 명시 켬 ∪ (codex 자동 감지 − 명시 끔). 토글은 메모리 전용이라 재시작에
        // 날아가므로, codex pane은 감지로 기본 동작해야 한다.
        let force_keys = self.wheel_keys.contains(&pane)
            || (!self.wheel_keys_off.contains(&pane)
                && self.run_cmd.get(&pane).is_some_and(|c| crate::panewheel::is_tui_history_app(c)));
        if typed {
            self.clear_ai_active(pane); // 키보드로 화면을 닫았을 수 있다 — 바의 열림 표시 해제.
            // 대상 규칙은 panegroup 한 곳에만 있다(동기 스크롤과 같은 뜻을 써야 한다).
            let bpanes = self.broadcast.then(|| crate::panegroup::targets(self.broadcast_group, self.window_panes));
            // 운영 표식 세션에서 되돌릴 수 없는 명령이면 붙잡아 확인한다(guard.rs).
            // 붙잡지 않으면 바이트는 손대지 않은 채 그대로 나간다.
            let targets: &[PaneId] = bpanes.as_deref().unwrap_or(&[]);
            let held = crate::guard::guard_input(
                self.guard_dangerous, self.risky_panes, &self.orch.panes, pane, &bytes, targets,
            );
            if let Some(p) = held {
                *self.pending_send = Some(p);
            } else if let Some(panes) = bpanes {
                self.orch.send(Command::Broadcast { panes, data: Bytes::from(bytes) });
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
        let (wheel, ctrl_wheel, shift_wheel) = ui.input(|i| {
            let wheel = crate::paneio::raw_wheel(i).y;
            (wheel, over && i.modifiers.command && wheel != 0.0,
             over && i.modifiers.shift && !i.modifiers.command && wheel != 0.0)
        });
        if ctrl_wheel {
            // 포커스 여부와 무관하게 포인터가 올라간 이 pane을 확대/축소(+포커스).
            *self.zoom_req = Some((pane, wheel));
            ui.input_mut(|i| {
                crate::paneio::consume_wheel(i); // 0.34: 이벤트 제거 = 원시 델타 소비.
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }

        // 오버레이 열림은 화면 하단 안내줄로 판정(+방금 연 공백기는 래치로 흡수).
        let overlay = force_keys && wheel != 0.0
            && crate::panewheel::overlay_open(&self.orch.panes, pane, self.tui_overlay.get(&pane));
        // 휠의 목적지는 한 규칙(panewheel::wheel_target)으로 정한다 — 탭과 분리 창이 같아야 한다.
        let target = crate::panewheel::wheel_target(crate::panewheel::WheelCtx {
            alt_screen, alt_scroll, mouse_on, force_keys, shift: shift_wheel,
            overlay, up: wheel > 0.0,
        });
        if mouse_on {
            let to_app = target == crate::panewheel::WheelTo::Nothing;
            let rep = mouse_reports(
                ui, rect, cw, ch, mouse_sgr, mouse_release, mouse_motion,
                shift_wheel || (over && to_app && wheel != 0.0 && !ctrl_wheel),
            );
            if !rep.is_empty() { self.orch.send(Command::WriteInput { pane, data: Bytes::from(rep) }); }
        }
        // 마우스를 가져가는 프로그램에서 처음 휠을 굴리면 **왜 이렇게 되는지** 한 번 알린다.
        // 말해 주지 않으면 "기록이 사라졌다"로 보인다 — 실제로 두 번 그런 보고를 받았다.
        if over && crate::panewheel::needs_wheel_hint(mouse_on, alt_screen, wheel)
            && self.wheel_hinted.insert(pane)
        {
            *self.wheel_hint = Some(nabi_i18n::tr(self.lang, "wheel.apptook").to_string());
        }
        if over && wheel != 0.0 && !ctrl_wheel {
            match crate::panewheel::wheel_bytes(target, wheel, app_cursor) {
                // 오버레이를 여는 휠이었다면 다음 휠부터 페이지 키가 그 안을 스크롤한다.
                Some(data) => {
                    if target == crate::panewheel::WheelTo::OpenTui { self.tui_overlay.insert(pane, std::time::Instant::now()); }
                    self.orch.send(Command::WriteInput { pane, data: Bytes::from(data) });
                }
                // 스크롤백은 한 눈금에 3줄(주류 에뮬레이터 관례).
                None if target == crate::panewheel::WheelTo::Scrollback => {
                    let lines = (wheel / ch * 3.0).round() as i32;
                    scroll += if lines == 0 { wheel.signum() as i32 } else { lines };
                }
                None => {}
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

        // 동기 스크롤로 남에게 옮겨 줄 양. 자물쇠를 쥔 채 남의 자물쇠를 잡지 않으려고
        // 여기서 값만 받아 두고 **블록을 벗어난 뒤** 적용한다(교착 회피).
        let mut sync = 0i32;
        let pane_model = self.orch.panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.model.clone()));
        if let Some(pane_model) = pane_model {
            if let Ok(mut model) = pane_model.lock() {
                if typed || to_bottom {
                    model.scroll_to_bottom();
                } else if to_top {
                    model.scroll_to_top();
                } else if scroll != 0 && !alt_screen {
                    model.scroll_by(scroll);
                    sync = scroll; // 같은 그룹을 같은 만큼 옮긴다(아래, 자물쇠를 놓은 뒤).
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
                    ui, rect, font.clone(), &model, &self.theme, self.find.as_deref(),
                    self.highlights, sel_norm, sel_rect, focused, self.blink_on, preedit,
                );
                // 영문 팁(Tip:/Note:) 줄 위에 한글 번역을 덧그린다(그리드는 불변 — tipoverlay.rs).
                self.tip_overlay(ui, rect, ch, &font, pane, &model);
                model.set_cell_px(ch); // 이미지 높이→줄 변환 기준(폰트 줌 반영).
                model.set_query_colors(&self.theme); // OSC 10/11 색 질의에 현재 테마로 답하도록.
                self.draw_inline_images(ui, rect, ch, &model);
                crate::scrollbar::draw(ui, rect, pane, &mut model); // 우측 스크롤바(스크롤백 있을 때).
                if crate::paneio::draw_scroll_badge(ui, rect, model.scrollback_offset()) {
                    model.scroll_to_bottom();
                }
                // 브로드캐스트 안전 표시: 실제 입력을 받는 pane(그룹 비면 전체, 아니면 멤버)만 테두리.
                if self.broadcast && (self.broadcast_group.is_empty() || self.broadcast_group.contains(&pane)) {
                    ui.painter_at(rect).rect_stroke(
                        rect.shrink(1.0),
                        egui::CornerRadius::ZERO,
                        egui::Stroke::new(2.5, crate::theme_ui::BROADCAST),
                        egui::StrokeKind::Inside,
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
        // **자물쇠를 다 놓은 뒤에** 남을 옮긴다 — 하나를 쥔 채 다른 하나를 잡으면 교착한다.
        if self.sync_scroll && sync != 0 {
            let others = crate::panegroup::targets(self.broadcast_group, self.window_panes);
            // 읽기 자물쇠를 먼저 놓는다 — 모델 자물쇠를 잡기 전에 지도에서 손을 뗀다.
            let models: Vec<_> = match self.orch.panes.read() {
                Ok(map) => others
                    .into_iter()
                    .filter(|p| *p != pane)
                    .filter_map(|p| map.get(&p).map(|v| v.model.clone()))
                    .collect(),
                Err(_) => Vec::new(),
            };
            for m in models {
                if let Ok(mut md) = m.lock() {
                    // 대체 화면(TUI)은 스크롤백이 없다 — 굴려도 뜻이 없으므로 건너뛴다.
                    if !md.alt_screen() {
                        md.scroll_by(sync);
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
