//! diff 문서에서 **한쪽 글을 복원**해 새 문서로 여는 동작(복원 규칙은 `nabi_editor::diffapply`).
//!
//! 바로 파일에 쓰지 않는다. **새 문서로 열어 눈으로 본 뒤 저장하게** 한다 — 남의 파일을
//! 덮는 일이라, 무엇이 될지 보지 못한 채 저장되는 길을 만들지 않는다.
//!
//! 제목에 몇 줄이 늘고 주는지를 적는다. 다 읽지 않아도 규모는 알 수 있어야 한다.

use crate::app::NabiApp;
use nabi_editor::diffapply::{counts, restore, Side};
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 그 diff 문서에서 한쪽 글을 복원해 새 탭으로 연다.
    pub(crate) fn restore_diff_side(&mut self, pane: PaneId, side: Side) {
        let Some(doc) = self.editors.get(&pane) else { return };
        let diff = doc.text.clone();
        let body = restore(&diff, side);
        if body.trim().is_empty() {
            self.notify = Some((tr(self.lang, "diff.restore.empty").to_string(), std::time::Instant::now()));
            return;
        }
        let (add, del) = counts(&diff, side);
        let which = match side {
            Side::Left => tr(self.lang, "diff.left"),
            Side::Right => tr(self.lang, "diff.right"),
        };
        // 제목에 규모를 적는다 — 열자마자 "얼마나 달라지나"가 보여야 한다.
        let title = format!("\u{21c4} {which} (+{add}/-{del})");
        self.open_text_as_doc(&title, body);
    }
}
