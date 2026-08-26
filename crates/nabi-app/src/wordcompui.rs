//! **낱말 완성**의 동작 — 후보 고르기는 `nabi_editor::wordcomp`가 한다.
//!
//! ## 목록을 띄우는 대신 돌아가며 넣는다
//!
//! 팝업으로 목록을 보여 주려면 커서의 **화면 좌표**가 필요한데, 이 편집기(rope)는 그것을
//! 갖고 있지 않다. 좌표를 만들려면 그리는 코드를 손봐야 하고, 그 자리는 과거에 두 번
//! 사고가 난 곳이다(밑줄 어긋남 v0.1.33, 렌더 스레드 즉사 v0.1.41).
//!
//! 그래서 다른 길을 쓴다 — **누를 때마다 다음 후보로 바꾼다.** Vim의 `Ctrl-N`,
//! Notepad++의 낱말 완성이 오래 써 온 방식이고, 목록 없이도 손에 익는다.
//!
//! 목록 팝업은 그리는 코드를 손볼 여유가 있는 배치에서 따로 다룬다. **없는 것을 있는 척
//! 하지 않는다** — 지금은 이 방식이 전부다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

/// 돌아가는 중인 완성 한 건.
#[derive(Clone, Default)]
pub(crate) struct WordCycle {
    /// 원래 치던 낱말(후보를 다시 뽑는 기준).
    pub prefix: String,
    /// 그 낱말이 시작하는 자리.
    pub start: usize,
    /// 지금 넣어 둔 후보의 차례.
    pub idx: usize,
    /// 지금 넣어 둔 글자 수(다음 후보로 바꿀 때 이만큼 지운다).
    pub len: usize,
}

impl NabiApp {
    /// 커서 앞 낱말을 문서 안의 낱말로 완성한다. 되풀이하면 다음 후보로 돈다.
    pub(crate) fn complete_word(&mut self, pane: PaneId) {
        let cycle = self.word_cycle.clone();
        let Some(doc) = self.editors.get_mut(&pane) else { return };
        if doc.readonly {
            return;
        }
        let Some(eb) = doc.edit.as_mut() else {
            // rope 편집기가 아닌 문서(평문·HEX·대용량)는 대상이 아니다.
            self.notify = Some((tr(self.lang, "editor.wordcomp.none").to_string(), std::time::Instant::now()));
            return;
        };
        let text = eb.rope.to_string();
        let cur = eb.cursor();
        // 이어서 누른 것인가 — 방금 넣은 후보의 끝에 커서가 그대로 있어야 한다.
        let cont = cycle.as_ref().filter(|c| c.start + c.len == cur && !c.prefix.is_empty());
        let (prefix, start, next_idx) = match cont {
            Some(c) => (c.prefix.clone(), c.start, c.idx + 1),
            None => {
                let Some(p) = nabi_editor::wordcomp::prefix_at(&text, cur) else {
                    self.notify = Some((tr(self.lang, "editor.wordcomp.none").to_string(), std::time::Instant::now()));
                    return;
                };
                let start = cur - p.chars().count();
                (p, start, 0)
            }
        };
        let hits = nabi_editor::wordcomp::candidates(&text, start, &prefix);
        if hits.is_empty() {
            self.word_cycle = None;
            self.notify = Some((tr(self.lang, "editor.wordcomp.none").to_string(), std::time::Instant::now()));
            return;
        }
        // 끝까지 돌면 **치던 글자로 되돌아온다** — 갇히지 않고 빠져나갈 길이 있어야 한다.
        let idx = next_idx % (hits.len() + 1);
        let word = hits.get(idx).cloned().unwrap_or_else(|| prefix.clone());
        let old_len = cont.map(|c| c.len).unwrap_or_else(|| prefix.chars().count());
        eb.replace_chars(start, start + old_len, &word);
        let len = word.chars().count();
        eb.set_cursor(start + len);
        eb.ensure_visible = true;
        self.word_cycle = Some(WordCycle { prefix, start, idx, len });
    }
}
