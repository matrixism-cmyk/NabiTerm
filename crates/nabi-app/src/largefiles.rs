//! 큰 파일 찾기 — 폴더를 재귀로 훑어 용량 상위 파일을 보고한다(디스크 정리). 읽기 전용·바운디드.
//! 결과는 nabiPad 문서로. 순수 top-N 코어는 단위테스트.

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::path::{Path, PathBuf};

/// (이름,크기) 목록에서 용량 내림차순 상위 n개. 순수.
pub(crate) fn top_n(mut items: Vec<(String, u64)>, n: usize) -> Vec<(String, u64)> {
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(n);
    items
}

/// 폴더를 재귀 훑어 (상대경로,크기)를 모은다. 상한 max_files, 숨김 제외.
fn collect(root: &Path, max_files: usize) -> Vec<(String, u64)> {
    let mut items = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            if items.len() >= max_files {
                return items;
            }
            let p = ent.path();
            if ent.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if nabi_fs::walk::is_real_dir(&p) {
                stack.push(p);
            } else if let Ok(md) = std::fs::metadata(&p) {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
                items.push((rel, md.len()));
            }
        }
    }
    items
}

impl NabiApp {
    /// 로컬 브라우저 폴더의 용량 상위 50개 파일을 nabiPad 문서로 연다(읽기 전용).
    pub(crate) fn find_large_files(&mut self) {
        let top = top_n(collect(&self.browser.path, 20_000), 50);
        let mut body = String::new();
        for (name, sz) in &top {
            body.push_str(&format!("{:>10}  {name}\n", crate::browserfs::human(*sz)));
        }
        if body.is_empty() {
            body = tr(self.lang, "dup.none").to_string();
        }
        let mut doc = crate::editor::EditorDoc::make(format!("\u{1f4be} {}", tr(self.lang, "large.title")), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n");
        doc.dirty = true;
        self.add_editor_tab(doc);
    }
}

#[cfg(test)]
mod tests {
    use super::top_n;

    #[test]
    fn sorts_desc_and_truncates() {
        let items = vec![("a".into(), 10), ("b".into(), 30), ("c".into(), 20)];
        assert_eq!(top_n(items.clone(), 2), vec![("b".to_string(), 30), ("c".to_string(), 20)]);
        assert_eq!(top_n(items, 10).len(), 3); // n>len이면 전부.
    }
}
