//! 폴더 분석 도구 — 트리 구조 텍스트화 + 확장자별 통계(읽기 전용·바운디드). 결과는 nabiPad 문서로.

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 폴더를 재귀로 훑어 트리 텍스트를 만든다(`├──`/`└──`). 상한: 항목 max개·숨김 제외.
pub(crate) fn dir_tree(root: &Path, max: usize) -> String {
    let mut out = String::new();
    let mut n = 0usize;
    fn walk(dir: &Path, prefix: &str, out: &mut String, n: &mut usize, max: usize) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut items: Vec<_> = rd.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')).collect();
        items.sort_by_key(|e| (e.path().is_file(), e.file_name()));
        let last = items.len().saturating_sub(1);
        for (i, e) in items.iter().enumerate() {
            if *n >= max {
                return;
            }
            *n += 1;
            let (branch, ext) = if i == last { ("\u{2514}\u{2500} ", "   ") } else { ("\u{251c}\u{2500} ", "\u{2502}  ") };
            let dir_mark = if e.path().is_dir() { "/" } else { "" };
            out.push_str(&format!("{prefix}{branch}{}{dir_mark}\n", e.file_name().to_string_lossy()));
            if e.path().is_dir() {
                walk(&e.path(), &format!("{prefix}{ext}"), out, n, max);
            }
        }
    }
    walk(root, "", &mut out, &mut n, max);
    out
}

/// (확장자→(개수,바이트)) 통계를 개수 내림차순 표로 포맷한다. 순수.
pub(crate) fn fmt_ext_stats(stats: &BTreeMap<String, (u64, u64)>) -> String {
    let mut rows: Vec<(&String, &(u64, u64))> = stats.iter().collect();
    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
    rows.iter().map(|(ext, (cnt, sz))| format!("{cnt:>6}  {:>10}  {ext}", crate::browserfs::human(*sz))).collect::<Vec<_>>().join("\n")
}

/// 폴더를 재귀로 훑어 확장자별 (개수,바이트)를 모은다. 상한 max_files.
fn collect_ext(root: &Path, max_files: usize) -> BTreeMap<String, (u64, u64)> {
    let mut map: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    let mut files = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            if files >= max_files {
                return map;
            }
            let p = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(md) = std::fs::metadata(&p) {
                let ext = p.extension().and_then(|x| x.to_str()).map(|x| format!(".{x}")).unwrap_or_else(|| "(없음)".into());
                let ent = map.entry(ext).or_default();
                ent.0 += 1;
                ent.1 += md.len();
                files += 1;
            }
        }
    }
    map
}

impl NabiApp {
    /// 로컬 브라우저 폴더의 트리 구조를 nabiPad 문서로 연다.
    pub(crate) fn open_dir_tree(&mut self) {
        let body = dir_tree(&self.browser.path, 5000);
        let title = format!("\u{1f333} {}", self.browser.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        self.add_editor_tab(crate::editor::EditorDoc::make(title, PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n"));
    }

    /// 로컬 브라우저 폴더의 확장자별 통계를 nabiPad 문서로 연다.
    pub(crate) fn open_dir_stats(&mut self) {
        let body = fmt_ext_stats(&collect_ext(&self.browser.path, 20_000));
        let body = if body.is_empty() { tr(self.lang, "dup.none").to_string() } else { body };
        self.add_editor_tab(crate::editor::EditorDoc::make(format!("\u{1f4ca} {}", tr(self.lang, "dir.stats")), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n"));
        self.notify = Some((tr(self.lang, "dir.stats").to_string(), Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_ext_stats;
    use std::collections::BTreeMap;

    #[test]
    fn formats_ext_stats_desc() {
        let mut m = BTreeMap::new();
        m.insert(".rs".to_string(), (10u64, 1024u64));
        m.insert(".md".to_string(), (3u64, 512u64));
        let out = fmt_ext_stats(&m);
        // 개수 내림차순 → .rs 먼저.
        assert!(out.starts_with("    10"));
        assert!(out.contains(".rs") && out.contains(".md"));
        assert!(out.find(".rs").unwrap() < out.find(".md").unwrap());
    }
}
