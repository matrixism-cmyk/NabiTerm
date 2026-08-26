//! **마지막 고친 자리로 이동**의 동작 — 자리 고르기는 `nabi_editor::editspots`가 한다.
//!
//! 여기서 하는 일은 고른 자리로 커서를 옮기고 **화면에 보이게** 하는 것뿐이다.
//! 옮기기만 하고 화면을 안 따라가면 아무 일도 안 한 것처럼 보인다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 최근 고친 자리로 옮긴다. 갈 곳이 없으면 그렇다고 말한다.
    pub(crate) fn goto_last_edit(&mut self, pane: PaneId) {
        let target = self.editors.get_mut(&pane).and_then(|d| {
            let eb = d.edit.as_mut()?;
            let cur = eb.cursor();
            eb.spots.clamp(eb.rope.len_chars());
            eb.spots.next(cur)
        });
        match target {
            Some(at) => {
                if let Some(eb) = self.editors.get_mut(&pane).and_then(|d| d.edit.as_mut()) {
                    eb.set_cursor(at);
                    eb.ensure_visible = true; // 옮기기만 하면 화면 밖일 수 있다.
                }
            }
            // 눌렀는데 조용하면 고장으로 읽힌다 — 왜 안 움직였는지 말해 준다.
            None => {
                let msg = tr(self.lang, "editor.lastedit.none").to_string();
                self.notify = Some((msg, std::time::Instant::now()));
            }
        }
    }
}
