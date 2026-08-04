//! 대기 중인 뷰포트 명령 처리(창 정렬 · 항상 위에 · 전체화면).

use crate::app::NabiApp;

impl NabiApp {
    /// 글꼴 크기 설정(클램프 + config 영속).
    pub(crate) fn set_font_size(&mut self, v: f32) {
        self.font_size = v.clamp(6.0, 40.0);
        self.config.appearance.font_size = self.font_size;
        let _ = nabi_config::save(&self.config_path, &self.config);
    }

    /// 페인 최대화(줌) 토글(tmux식). 분할 보기에서 포커스 터미널만 전체 영역에 렌더한다.
    pub(crate) fn toggle_pane_zoom(&mut self) {
        self.pane_zoom = !self.pane_zoom;
        let key = if self.pane_zoom { "zoom.on" } else { "zoom.off" };
        self.notify = Some((nabi_i18n::tr(self.lang, key).to_string(), std::time::Instant::now()));
    }

    /// 포커스 pane을 이전/다음 프롬프트(OSC 133;A 기록)로 스크롤한다(팔레트 액션).
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

    /// 유휴 repaint 하트비트(ms): 벨 플래시 50 · 커서 깜빡임 260 · 그 외 500.
    pub(crate) fn idle_ms(&self) -> u64 {
        if self.bell_flash.is_some() {
            50
        } else if self.config.appearance.cursor_blink {
            260
        } else {
            500
        }
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
