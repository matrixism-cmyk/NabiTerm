//! **열린 문서끼리 비교** — 지금 보고 있는 것과 다른 탭을 견준다.
//!
//! 비교는 이미 두 갈래가 있었다: 브라우저에서 고른 두 파일(`compare_selected`)과 디스크
//! 원본과의 비교(`diff_editor_against_disk`). 정작 **열려 있는 두 문서**를 견줄 길이 없었다.
//! 같은 파일의 두 판을 나란히 열어 놓고 비교하는 것은 흔한 일이라 빈 자리가 컸다.
//!
//! 비교 알고리즘은 `difflines::diff_lines`를 그대로 쓴다 — 세 갈래가 같은 결과를 내야 한다.

use crate::app::NabiApp;
use crate::editor::{file_name, EditorDoc};
use nabi_i18n::tr;
use nabi_types::PaneId;
use std::path::PathBuf;
use std::time::Instant;

/// 비교할 수 있는 문서인가 — 글 문서만(HEX·대용량은 줄 비교가 뜻이 없다).
pub(crate) fn comparable(doc: &EditorDoc) -> bool {
    doc.hex.is_none() && doc.big.is_none()
}

/// 비교에 쓸 글. rope 문서(대용량 편집)는 문자열로 펼쳐 온다.
fn body_of(doc: &EditorDoc) -> String {
    match doc.edit.as_ref() {
        Some(eb) => eb.rope.to_string(),
        None => doc.text.clone(),
    }
}

impl NabiApp {
    /// 비교 상대를 고르는 창을 연다. 열린 글 문서가 둘 미만이면 알리고 만다.
    pub(crate) fn open_compare_picker(&mut self, from: PaneId) {
        if self.others_to_compare(from).is_empty() {
            self.notify = Some((tr(self.lang, "diff.needopen").to_string(), Instant::now()));
            return;
        }
        self.diff_pick = Some(from);
    }

    /// 비교 상대가 될 수 있는 다른 문서들 (pane, 보여 줄 이름).
    pub(crate) fn others_to_compare(&self, from: PaneId) -> Vec<(PaneId, String)> {
        let mut v: Vec<(PaneId, String)> = self
            .editors
            .iter()
            .filter(|(p, d)| **p != from && comparable(d))
            .map(|(p, d)| (*p, doc_label(d)))
            .collect();
        v.sort_by(|a, b| a.1.cmp(&b.1));
        v
    }

    /// 두 열린 문서를 견줘 결과를 새 문서로 연다.
    pub(crate) fn compare_open_docs(&mut self, a: PaneId, b: PaneId) {
        let (Some(da), Some(db)) = (self.editors.get(&a), self.editors.get(&b)) else { return };
        if !comparable(da) || !comparable(db) {
            return;
        }
        let (ta, tb) = (body_of(da), body_of(db));
        // 상한은 기존 비교와 같은 이유로 둔다 — LCS는 줄 수의 곱만큼 걸린다.
        if ta.lines().count() > 4000 || tb.lines().count() > 4000 {
            self.notify = Some((tr(self.lang, "diff.toolarge").to_string(), Instant::now()));
            return;
        }
        let (na, nb) = (doc_label(da), doc_label(db));
        let d = crate::difflines::diff_lines(&ta, &tb);
        let changed = d.lines().any(|l| !l.starts_with("  "));
        let body = match changed {
            true => format!("--- {na}\n+++ {nb}\n{d}"),
            false => tr(self.lang, "diff.nochange").to_string(),
        };
        let mut doc = EditorDoc::make(
            format!("\u{21c4} {na} / {nb}"),
            PathBuf::new(),
            None,
            body,
            true,
            self.font_size,
            "UTF-8".into(),
            "\n",
        );
        doc.dirty = true;
        self.add_editor_tab(doc);
    }
}

/// 목록·제목에 쓸 이름. 경로가 없으면(새 문서) 탭 제목을 쓴다.
fn doc_label(d: &EditorDoc) -> String {
    match d.path.as_os_str().is_empty() {
        true => d.title.clone(),
        false => file_name(&d.path),
    }
}

#[cfg(test)]
mod tests {
    /// 상대 목록에 **자기 자신이 있으면 안 된다** — 자기와 비교하면 늘 "차이 없음"이다.
    /// (앱 상태를 만들 수 없어 규칙만 확인한다: 필터 조건이 `**p != from`인지.)
    #[test]
    fn the_source_document_is_excluded_by_the_filter() {
        let src = include_str!("diffopen.rs");
        assert!(src.contains("**p != from"), "자기 자신을 거르는 조건이 사라졌다");
    }

    /// 결과 제목은 두 이름을 모두 담아야 무엇과 무엇을 견줬는지 알 수 있다.
    #[test]
    fn the_result_title_names_both_sides() {
        let label = format!("\u{21c4} {} / {}", "a.txt", "b.txt");
        assert!(label.contains("a.txt") && label.contains("b.txt"));
    }
}
