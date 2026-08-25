//! 스크롤백 표식의 동작 — 남기기·오가기·지우기(순수 로직은 `scrollmark`).
//!
//! 표식이 가리키는 것은 **절대 줄 번호**이므로, 실제로 그 자리로 보내는 일은 이미 있는
//! `scroll_to_abs_line`을 쓴다(`findall`의 결과 클릭과 같은 통로 — 같은 일을 두 곳이 다르게
//! 하면 언젠가 어긋난다).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 지금 화면 맨 위 줄에 표식을 켜거나 끈다.
    ///
    /// "지금 보는 자리"의 기준을 화면 맨 위로 잡는 이유: 사용자가 무언가를 보려고 스크롤을
    /// 멈춘 자리는 대개 화면 맨 위에 그 줄이 오도록 맞춘 자리다. 가운데나 아래로 잡으면
    /// 되돌아왔을 때 화면이 한 판 어긋나 보인다.
    pub(crate) fn toggle_scroll_mark(&mut self) {
        let Some(pane) = self.focused_pane() else { return };
        let Some((top, first)) = self.pane_view_bounds(pane) else { return };
        let marks = self.scroll_marks.entry(pane).or_default();
        marks.drop_trimmed(first); // 잘려 나간 표식을 먼저 버린다.
        let on = marks.toggle(top);
        let key = if on { "mark.added" } else { "mark.removed" };
        let n = marks.all().len();
        self.notify = Some((format!("{} ({n})", tr(self.lang, key)), std::time::Instant::now()));
    }

    /// 표식 사이를 오간다. `forward`면 더 최근 쪽.
    pub(crate) fn jump_scroll_mark(&mut self, forward: bool) {
        let Some(pane) = self.focused_pane() else { return };
        let Some((top, first)) = self.pane_view_bounds(pane) else { return };
        let target = {
            let Some(marks) = self.scroll_marks.get_mut(&pane) else { return };
            marks.drop_trimmed(first);
            if marks.is_empty() {
                self.notify = Some((tr(self.lang, "mark.none").to_string(), std::time::Instant::now()));
                return;
            }
            match forward {
                true => marks.next_after(top),
                false => marks.prev_before(top),
            }
        };
        match target {
            Some(line) => self.scroll_pane_to(pane, line),
            // 끝에 닿았으면 말해 준다 — 눌렀는데 아무 일도 없으면 고장으로 읽힌다.
            None => {
                let key = if forward { "mark.atlast" } else { "mark.atfirst" };
                self.notify = Some((tr(self.lang, key).to_string(), std::time::Instant::now()));
            }
        }
    }

    /// 이 pane의 표식을 모두 지운다.
    pub(crate) fn clear_scroll_marks(&mut self) {
        let Some(pane) = self.focused_pane() else { return };
        if let Some(m) = self.scroll_marks.get_mut(&pane) {
            m.clear();
        }
        self.notify = Some((tr(self.lang, "mark.cleared").to_string(), std::time::Instant::now()));
    }

    /// (화면 맨 위 절대 줄, 스크롤백에 남아 있는 첫 줄). 모델을 못 잡으면 None.
    fn pane_view_bounds(&self, pane: nabi_types::PaneId) -> Option<(usize, usize)> {
        let view = self.orch.panes.read().ok()?.get(&pane).cloned()?;
        let model = view.model.lock().ok()?;
        let total = model.total_abs_lines();
        let limit = self.config.terminal.scrollback;
        // 상한을 넘겨 잘려 나간 앞부분은 더 이상 가리킬 수 없다.
        Some((model.top_abs_line(), total.saturating_sub(limit)))
    }

    /// 그 pane을 표식 자리로 보낸다.
    fn scroll_pane_to(&mut self, pane: nabi_types::PaneId, line: usize) {
        if let Some(view) = self.orch.panes.read().ok().and_then(|m| m.get(&pane).cloned()) {
            if let Ok(mut model) = view.model.lock() {
                model.scroll_to_abs_line(line);
            }
        }
    }
}
