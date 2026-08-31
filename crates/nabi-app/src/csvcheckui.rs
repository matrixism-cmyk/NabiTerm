//! 칸 수가 어긋난 줄 찾기의 화면 쪽 — 세는 일은 `nabi_editor::csvcheck` 가 한다.
//!
//! 비밀 찾기(`secretui`)와 같은 모양이다: **아무것도 바꾸지 않고** 알림 한 줄로 답한다.
//! 다만 하나 더 한다 — 찾았으면 **첫 줄로 데려간다.** 줄 번호만 알려 주면 사용자가
//! 다시 그 줄을 찾아가야 하는데, 큰 파일에서는 그게 다시 일이다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 이 문서를 표로 보고, 칸 수가 다수와 다른 줄을 찾아 알린다.
    pub(crate) fn find_odd_rows_in_doc(&mut self, pane: PaneId) {
        let Some(doc) = self.editors.get_mut(&pane) else { return };
        // 대용량 편집기(rope)는 문자열로 펼치지 않는다 — 그 자체가 그 편집기의 존재 이유다.
        // 여기서는 세려면 전체가 필요하므로, rope 문서는 그때만 한 번 펼친다.
        let text = match doc.edit.as_ref() {
            Some(eb) => eb.rope.to_string(),
            None => doc.text.clone(),
        };
        // 구분자는 확장자로 고른다 — `.tsv` 는 탭이다. 그 밖에는 쉼표로 본다.
        let delim = if doc.lang_ext().eq_ignore_ascii_case("tsv") { '\t' } else { ',' };
        let (most, odd) = nabi_editor::csvcheck::odd_rows(&text, delim);
        if odd.is_empty() {
            let msg = format!("{} ({most})", tr(self.lang, "editor.oddrows.none"));
            self.notify = Some((msg, std::time::Instant::now()));
            return;
        }
        // 줄 번호를 몇 개만 보여 준다 — 전부 늘어놓으면 알림이 화면을 덮는다.
        let head: Vec<String> = odd.iter().take(5).map(|r| (r.line + 1).to_string()).collect();
        let more = odd.len().saturating_sub(head.len());
        let tail = if more > 0 { format!(" +{more}") } else { String::new() };
        let msg = format!(
            "{} {} \u{b7} {}{tail} ({most})",
            tr(self.lang, "editor.oddrows.found"),
            odd.len(),
            head.join(", ")
        );
        self.notify = Some((msg, std::time::Instant::now()));
        doc.jump_to_line(odd[0].line); // 첫 줄로 데려간다.
    }
}
