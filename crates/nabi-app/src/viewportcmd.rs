//! 대기 중인 뷰포트 명령 처리(창 정렬 · 항상 위에 · 전체화면).

use crate::app::NabiApp;

impl NabiApp {
    /// 글꼴 크기 설정(클램프 + config 영속).
    pub(crate) fn set_font_size(&mut self, v: f32) {
        self.font_size = v.clamp(6.0, 40.0);
        self.config.appearance.font_size = self.font_size;
        self.save_config();
    }

    /// 페인 최대화(줌) 토글(tmux식). 분할 보기에서 포커스 터미널만 전체 영역에 렌더한다.
    pub(crate) fn toggle_pane_zoom(&mut self) {
        self.pane_zoom = !self.pane_zoom;
        let key = if self.pane_zoom { "zoom.on" } else { "zoom.off" };
        self.notify = Some((nabi_i18n::tr(self.lang, key).to_string(), std::time::Instant::now()));
    }

    /// 포커스 pane을 이전/다음 프롬프트(OSC 133;A 기록)로 스크롤한다(팔레트 액션).
    /// **실패한 명령으로만** 오간다. 없으면 없다고 말한다 — 눌렀는데 아무 일도 없으면
    /// 고장으로 읽힌다.
    pub(crate) fn jump_failed(&mut self, next: bool) {
        let Some(p) = self.focused_pane() else { return };
        let moved = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).cloned())
            .and_then(|v| v.model.lock().ok().map(|mut md| md.jump_failed_prompt(!next)))
            .unwrap_or(false);
        if !moved {
            self.notify = Some((
                nabi_i18n::tr(self.lang, "prompt.nofail").to_string(),
                std::time::Instant::now(),
            ));
        }
    }

    pub(crate) fn jump_prompt(&mut self, next: bool) {
        let Some(p) = self.focused_pane() else { return };
        if let Some(v) = self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()) {
            if let Ok(mut md) = v.model.lock() {
                if next {
                    md.jump_next_prompt();
                } else {
                    md.jump_prev_prompt();
                }
            }
        }
    }

    /// 키 입력이 있으면 커서 깜빡임 위상을 리셋한다(입력 직후 커서가 보이도록).
    pub(crate) fn reset_blink_on_input(&mut self, ctx: &egui::Context) {
        let typed = ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Text(_)
                        | egui::Event::Key { pressed: true, .. }
                        | egui::Event::Ime(_)
                )
            })
        });
        if typed {
            self.blink_start = std::time::Instant::now();
        }
    }

    /// 현재 시점의 커서 표시 여부(깜빡임 위상). 깜빡임이 꺼져 있으면 항상 표시.
    pub(crate) fn blink_on(&self) -> bool {
        if !self.config.appearance.cursor_blink {
            return true;
        }
        let half = self.config.appearance.blink_ms.max(1) as u128;
        (self.blink_start.elapsed().as_millis() / half).is_multiple_of(2)
    }

    /// 다음 프레임을 **필요한 시점에만** 예약한다.
    ///
    /// 예전에는 매 프레임 무조건 하트비트를 걸어 앱이 영원히 잠들지 못했다(최소화 상태에서도).
    /// 지금은 실제로 화면이 바뀔 시점 — 깜빡임 토글 경계, 벨 플래시 — 만 예약하고,
    /// 창이 안 보이면(비포커스·최소화) 아무 것도 예약하지 않는다. 출력·입력은 각자 repaint를
    /// 요청하므로 반응성에는 영향이 없다.
    pub(crate) fn schedule_next_frame(&self, ctx: &egui::Context) {
        use std::time::Duration;
        // 벨 플래시는 짧게 사라지는 애니메이션이라 진행 중에는 촘촘히.
        if self.bell_flash.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if !self.config.appearance.cursor_blink {
            return; // 깜빡이지 않으면 유휴 시 그릴 이유가 없다.
        }
        // 최소화되었거나 포커스가 없으면 커서 깜빡임이 보이지 않는다 → 깨울 필요 없음.
        let visible = ctx.input(|i| {
            let vp = i.viewport();
            vp.focused.unwrap_or(true) && !vp.minimized.unwrap_or(false)
        });
        if !visible {
            return;
        }
        // 다음 토글 경계까지만 잔다(자유 주기 폴링이면 토글과 어긋나 헛 프레임이 생긴다).
        let half = self.config.appearance.blink_ms.max(1) as u128;
        let phase = self.blink_start.elapsed().as_millis() % (2 * half);
        let next = if phase < half { half - phase } else { 2 * half - phase };
        ctx.request_repaint_after(Duration::from_millis((next as u64).max(1)));
    }

    pub(crate) fn process_pending(&mut self, ctx: &egui::Context) {
        if let Some(mode) = self.pending_arrange.take() {
            self.apply_arrange(ctx, mode);
        }
        if let Some(on) = self.pending_on_top.take() {
            let level = if on {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        }
        if let Some(full) = self.pending_fullscreen.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(full));
        }
    }
}
