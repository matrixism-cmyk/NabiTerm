//! 파일 내용 검색(Find in Files, VS Code식) — 디렉터리를 재귀로 훑어 패턴이 포함된 줄을 찾는다.
//! 결과는 "상대경로:줄: 내용" 텍스트로 모아 nabiPad 문서로 연다(별도 결과 패널 없이 기존 인프라 재사용).
//! 안전장치: 파일 수·히트 수 상한, 큰 파일·이진 파일 건너뜀.

use crate::app::NabiApp;
use crate::editor::EditorDoc;
use nabi_i18n::tr;
use std::path::{Path, PathBuf};
use std::time::Instant;

impl NabiApp {
    /// 파일 내용 검색(Find in Files) — 로컬 브라우저 폴더를 재귀 검색해 결과를 nabiPad 문서로 연다.
    pub(crate) fn content_search(&mut self, pat: String) {
        let pat = pat.trim().to_string();
        if pat.is_empty() {
            return;
        }
        let ci = !pat.chars().any(|c| c.is_uppercase()); // 스마트케이스.
        let (results, hits) = search_dir(&self.browser.path, &pat, ci, 3000, 500);
        let body = if results.is_empty() { tr(self.lang, "search.nomatch").to_string() } else { results };
        let mut doc = EditorDoc::make(format!("\u{1f50d} {pat}"), PathBuf::new(), None, body, true, self.font_size, "UTF-8".into(), "\n");
        doc.dirty = true;
        self.add_editor_tab(doc);
        self.notify = Some((format!("{} {hits}", tr(self.lang, "browser.contentsearch")), Instant::now()));
    }
}

/// 한 텍스트에서 패턴(부분 문자열)이 든 줄을 (줄번호1-base, 줄내용 trim) 으로. ci=대소문자 무시. 순수.
pub(crate) fn grep_lines(content: &str, pat: &str, ci: bool) -> Vec<(usize, String)> {
    let needle = if ci { pat.to_lowercase() } else { pat.to_string() };
    content
        .lines()
        .enumerate()
        .filter(|(_, l)| if ci { l.to_lowercase().contains(&needle) } else { l.contains(&needle) })
        .map(|(i, l)| (i + 1, l.trim().chars().take(200).collect()))
        .collect()
}

/// root를 재귀 검색해 결과 텍스트를 만든다. 상한: 파일 max_files개, 히트 max_hits개, 파일당 512KB·이진 제외.
pub(crate) fn search_dir(root: &Path, pat: &str, ci: bool, max_files: usize, max_hits: usize) -> (String, usize) {
    let mut out = String::new();
    let (mut files, mut hits) = (0usize, 0usize);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            if files >= max_files || hits >= max_hits {
                return (out, hits);
            }
            let p = ent.path();
            let name = ent.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue; // 숨김/.git 등 제외.
            }
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let big = std::fs::metadata(&p).map(|m| m.len() > 512 * 1024).unwrap_or(true);
            if big || crate::edithex::peek_is_binary(&p) {
                continue;
            }
            files += 1;
            let Ok(content) = std::fs::read_to_string(&p) else { continue };
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
            for (ln, text) in grep_lines(&content, pat, ci) {
                out.push_str(&format!("{rel}:{ln}: {text}\n"));
                hits += 1;
                if hits >= max_hits {
                    break;
                }
            }
        }
    }
    (out, hits)
}

/// 한 텍스트의 치환 횟수와 치환 결과를 돌려준다(대소문자 구분). 순수.
pub(crate) fn replace_count(content: &str, find: &str, to: &str) -> (usize, String) {
    if find.is_empty() {
        return (0, content.to_string());
    }
    (content.matches(find).count(), content.replace(find, to))
}

/// 디렉터리 재귀 찾아 바꾸기(대소문자 구분). apply=false=계산만, true=기록. (변경 파일수, 총 치환수).
pub(crate) fn replace_in_dir(root: &Path, find: &str, to: &str, apply: bool, max_files: usize) -> (usize, usize) {
    if find.is_empty() {
        return (0, 0);
    }
    let (mut changed, mut total, mut files) = (0usize, 0usize, 0usize);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            if files >= max_files {
                return (changed, total);
            }
            let p = ent.path();
            if ent.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let big = std::fs::metadata(&p).map(|m| m.len() > 512 * 1024).unwrap_or(true);
            if big || crate::edithex::peek_is_binary(&p) {
                continue;
            }
            files += 1;
            let Ok(content) = std::fs::read_to_string(&p) else { continue };
            let (n, out) = replace_count(&content, find, to);
            if n == 0 {
                continue;
            }
            total += n;
            changed += 1;
            if apply {
                let _ = std::fs::write(&p, out);
            }
        }
    }
    (changed, total)
}

#[cfg(test)]
mod tests {
    use super::{grep_lines, replace_count};

    #[test]
    fn replaces_and_counts() {
        assert_eq!(replace_count("a x a y a", "a", "Z"), (3, "Z x Z y Z".to_string()));
        assert_eq!(replace_count("no match", "zzz", "q"), (0, "no match".to_string()));
        assert_eq!(replace_count("anything", "", "q").0, 0); // 빈 find=무동작.
    }

    #[test]
    fn greps_lines_with_case() {
        let c = "alpha\nBeta error\ngamma ERROR\ndelta";
        // 대소문자 무시: 두 줄 모두.
        let ci = grep_lines(c, "error", true);
        assert_eq!(ci.len(), 2);
        assert_eq!(ci[0], (2, "Beta error".to_string()));
        // 대소문자 구분: 소문자 한 줄만.
        let cs = grep_lines(c, "error", false);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].0, 2);
        assert!(grep_lines(c, "zeta", true).is_empty());
    }
}
