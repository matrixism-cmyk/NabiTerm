//! 중복 파일 찾기 — 폴더를 재귀로 훑어 내용이 같은 파일을 묶는다(정리용). 읽기 전용·바운디드.
//! 결과는 nabiPad 문서로 표시. 순수 그룹화 코어는 단위테스트.

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// (이름, 크기, 내용해시) 목록을 (크기,해시)로 묶어 2개 이상인 그룹만 정렬해 돌려준다. 순수.
pub(crate) fn group_dups(items: &[(String, u64, u64)]) -> Vec<Vec<String>> {
    let mut by: HashMap<(u64, u64), Vec<String>> = HashMap::new();
    for (name, sz, h) in items {
        by.entry((*sz, *h)).or_default().push(name.clone());
    }
    let mut groups: Vec<Vec<String>> = by.into_values().filter(|g| g.len() >= 2).collect();
    for g in &mut groups {
        g.sort();
    }
    groups.sort();
    groups
}

/// 폴더를 재귀 훑어 (이름,크기,해시)를 모은다. 상한 max_files, 20MB↑·숨김 제외.
fn collect(root: &Path, max_files: usize) -> Vec<(String, u64, u64)> {
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
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(md) = std::fs::metadata(&p) else { continue };
            if md.len() > 20 * 1024 * 1024 {
                continue;
            }
            let Ok(data) = std::fs::read(&p) else { continue };
            let mut h = std::collections::hash_map::DefaultHasher::new();
            data.hash(&mut h);
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
            items.push((rel, md.len(), h.finish()));
        }
    }
    items
}

impl NabiApp {
    /// 로컬 브라우저 폴더에서 중복 파일을 찾아 그룹 보고를 nabiPad 문서로 연다(읽기 전용).
    pub(crate) fn find_duplicates(&mut self) {
        let items = collect(&self.browser.path, 5000);
        let groups = group_dups(&items);
        let mut body = String::new();
        for (i, g) in groups.iter().enumerate() {
            body.push_str(&format!("# {} ({} files)\n", i + 1, g.len()));
            for name in g {
                body.push_str(&format!("  {name}\n"));
            }
            body.push('\n');
        }
        if body.is_empty() {
            body = tr(self.lang, "dup.none").to_string();
        }
        let mut doc = crate::editor::EditorDoc::make(format!("\u{29c9} {}", tr(self.lang, "dup.title")), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n");
        doc.dirty = true;
        self.add_editor_tab(doc);
        self.notify = Some((format!("{}: {}", tr(self.lang, "dup.title"), groups.len()), Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::group_dups;

    #[test]
    fn groups_same_size_and_hash() {
        let items = vec![
            ("a".into(), 10, 111),
            ("b".into(), 10, 111), // a,b 동일.
            ("c".into(), 10, 222), // 크기 같아도 해시 다름 → 별개.
            ("d".into(), 20, 111), // 해시 같아도 크기 다름 → 별개.
        ];
        let g = group_dups(&items);
        assert_eq!(g, vec![vec!["a".to_string(), "b".to_string()]]);
        assert!(group_dups(&[("x".into(), 1, 1)]).is_empty()); // 단독은 그룹 아님.
    }
}
