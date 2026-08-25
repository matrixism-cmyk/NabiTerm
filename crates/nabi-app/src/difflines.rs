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
        let (pa, pb) = (self.browser.path.join(&sel[0]), self.browser.path.join(&sel[1]));
        // 이진 파일이면 줄 비교가 뜻이 없다 — 바이트 자리로 견준다(hexdiff).
        if nabi_editor::edithex::peek_is_binary(&pa) || nabi_editor::edithex::peek_is_binary(&pb) {
            self.compare_binaries(&sel[0], &sel[1], &pa, &pb);
            return;
        }
        let (Ok(a), Ok(b)) = (std::fs::read_to_string(&pa), std::fs::read_to_string(&pb)) else {
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

impl NabiApp {
    /// 두 이진 파일을 견줘 결과를 문서로 연다.
    ///
    /// 통째로 읽되 상한을 둔다 — 몇 GB짜리 둘을 메모리에 올리면 앱이 죽는다. 상한까지만
    /// 읽고 **그 사실을 화면에 적는다**(조용히 자르면 "여기까지 같다"는 거짓말이 된다).
    fn compare_binaries(&mut self, na: &str, nb: &str, pa: &std::path::Path, pb: &std::path::Path) {
        const MAX: u64 = 64 * 1024 * 1024;
        let (Ok(a), Ok(b)) = (read_capped(pa, MAX), read_capped(pb, MAX)) else {
            self.notify = Some((tr(self.lang, "diff.need2").to_string(), Instant::now()));
            return;
        };
        let d = nabi_editor::hexdiff::compare(&a.0, &b.0);
        let mut body = format!("{}\n\n", nabi_editor::hexdiff::summary(&d));
        if a.1 || b.1 {
            body.push_str(&format!("{}\n\n", tr(self.lang, "diff.hexcapped")));
        }
        for x in &d.diffs {
            let f = |v: Option<u8>| v.map(|n| format!("{n:02x}")).unwrap_or_else(|| "--".into());
            body.push_str(&format!("{:08x}  {}  {}\n", x.at, f(x.a), f(x.b)));
        }
        if d.more > 0 {
            body.push_str(&format!("\n\u{2026} +{}\n", d.more));
        }
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

/// 파일을 상한까지 읽는다 — `(바이트, 잘렸는가)`.
fn read_capped(p: &std::path::Path, max: u64) -> std::io::Result<(Vec<u8>, bool)> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut buf = Vec::new();
    let n = f.by_ref().take(max).read_to_end(&mut buf)?;
    let more = n as u64 >= max;
    Ok((buf, more))
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
