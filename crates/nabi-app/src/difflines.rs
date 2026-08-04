//! 두 파일 라인 비교(diff, VS Code식) — LCS 기반으로 공통/삭제(-)/추가(+) 줄을 만든다.
//! 순수 함수(읽기 전용). 결과는 nabiPad 문서로 표시. 큰 파일은 호출측에서 상한으로 거른다.

use crate::app::NabiApp;
use crate::editor::{file_name, EditorDoc};
use nabi_i18n::tr;
use std::path::PathBuf;
use std::time::Instant;

impl NabiApp {
    /// 로컬 브라우저에서 정확히 2개 선택된 파일의 라인 비교를 nabiPad 문서로 연다(읽기 전용).
    pub(crate) fn compare_selected(&mut self) {
        let mut sel: Vec<String> = self.browser.multi.iter().cloned().collect();
        sel.sort();
        if sel.len() != 2 {
            self.notify = Some((tr(self.lang, "diff.need2").to_string(), Instant::now()));
            return;
        }
        let (Ok(a), Ok(b)) = (
            std::fs::read_to_string(self.browser.path.join(&sel[0])),
            std::fs::read_to_string(self.browser.path.join(&sel[1])),
        ) else {
            self.notify = Some((tr(self.lang, "diff.need2").to_string(), Instant::now()));
            return;
        };
        if a.lines().count() > 2000 || b.lines().count() > 2000 {
            self.notify = Some((tr(self.lang, "diff.toolarge").to_string(), Instant::now()));
            return;
        }
        let body = format!("--- {}\n+++ {}\n{}", sel[0], sel[1], diff_lines(&a, &b));
        let mut doc = EditorDoc::make(format!("\u{21c4} {} / {}", sel[0], sel[1]), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n");
        doc.dirty = true;
        self.add_editor_tab(doc);
    }

    /// 열린 문서(plain)의 현재 내용을 디스크 원본과 비교한 diff를 nabiPad로 연다(저장 전 변경 검토).
    pub(crate) fn diff_editor_against_disk(&mut self, pane: nabi_types::PaneId) {
        let Some(doc) = self.editors.get(&pane) else { return };
        if doc.path.as_os_str().is_empty() || doc.hex.is_some() || doc.edit.is_some() || doc.big.is_some() {
            return; // plain 텍스트 문서만(경로 있음).
        }
        let (cur, name) = (doc.text.clone(), file_name(&doc.path));
        let Ok(disk) = std::fs::read_to_string(&doc.path) else { return };
        if disk.lines().count() > 4000 || cur.lines().count() > 4000 {
            self.notify = Some((tr(self.lang, "diff.toolarge").to_string(), Instant::now()));
            return;
        }
        let d = diff_lines(&disk, &cur);
        let body = if d.lines().all(|l| l.starts_with("  ")) { tr(self.lang, "diff.nochange").to_string() } else { d };
        let mut doc = EditorDoc::make(format!("\u{21c4} {name}"), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n");
        doc.dirty = true;
        self.add_editor_tab(doc);
    }
}

/// a→b 라인 diff를 만든다(" "=공통, "-"=a에만, "+"=b에만). LCS 동적계획법. 순수.
pub(crate) fn diff_lines(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let (n, m) = (al.len(), bl.len());
    // LCS 길이 표(dp[i][j] = al[i..], bl[j..]의 LCS 길이).
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if al[i] == bl[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = String::new();
    while i < n && j < m {
        if al[i] == bl[j] {
            out.push_str(&format!("  {}\n", al[i]));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push_str(&format!("- {}\n", al[i]));
            i += 1;
        } else {
            out.push_str(&format!("+ {}\n", bl[j]));
            j += 1;
        }
    }
    for line in &al[i..] {
        out.push_str(&format!("- {line}\n"));
    }
    for line in &bl[j..] {
        out.push_str(&format!("+ {line}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::diff_lines;

    #[test]
    fn diffs_changed_added_removed() {
        assert_eq!(diff_lines("a\nb\nc", "a\nx\nc"), "  a\n- b\n+ x\n  c\n");
        assert_eq!(diff_lines("a", "a\nb"), "  a\n+ b\n"); // 추가.
        assert_eq!(diff_lines("a\nb", "a"), "  a\n- b\n"); // 삭제.
        assert_eq!(diff_lines("", ""), ""); // 동일(빈).
    }
}
