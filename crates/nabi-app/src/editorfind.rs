//! 에디터 찾기/바꾸기 + 줄 이동(Ctrl+F/Ctrl+G). 줄 단위로 다음 일치를 찾아 스크롤한다.
//! 일반 편집기는 버퍼를, 대용량 뷰어는 인덱스 줄을 스캔한다.

use crate::editor::EditorDoc;
use nabi_i18n::{tr, Lang};

/// 찾기/바꾸기/줄이동 상태(에디터 문서별).
#[derive(Default)]
pub(crate) struct FindState {
    pub open: bool,
    pub query: String,
    pub replace: String,
    /// 대소문자 무시.
    pub ci: bool,
    /// 전체 단어만 일치(\b로 감쌈).
    pub whole: bool,
    /// query를 정규식으로 해석.
    pub regex: bool,
    /// 줄 이동 입력.
    pub goto: String,
    /// 적용 대기: 이 줄(0-base)로 스크롤.
    pub scroll_to: Option<usize>,
    /// 적용 대기: 이 char 오프셋으로 커서 이동(파일:줄:열 점프의 열 위치). 1회 소비.
    pub pending_cursor: Option<usize>,
    /// 현재 찾기 위치(줄).
    pub cur: usize,
    /// 일치하는 줄 인덱스 목록(개수 표시·이동 공용). built 키와 함께 캐시.
    pub matches: Vec<usize>,
    /// matches를 만든 검색 키(query+옵션). 바뀌면 재계산.
    pub built: String,
    /// 필터 적용 전 원본(토글 복원용). Some이면 현재 필터된 상태.
    pub filter_backup: Option<String>,
}

/// 문서 내용의 싼 서명(길이·줄수) — 편집되면 일치 캐시를 무효화하기 위함.
fn doc_sig(doc: &EditorDoc) -> usize {
    if let Some(e) = &doc.edit {
        return e.rope.len_chars().wrapping_mul(31).wrapping_add(e.rope.len_lines());
    }
    match &doc.big {
        Some(b) => b.line_count(),
        None => doc.text.len().wrapping_mul(31).wrapping_add(doc.text.split('\n').count()),
    }
}

/// 현재 검색 옵션+문서 서명을 나타내는 캐시 키(바뀌면 일치 목록 재계산).
fn key(doc: &EditorDoc) -> String {
    let f = &doc.find;
    format!("{}\u{1}{}{}{}\u{1}{}", f.query, f.ci as u8, f.whole as u8, f.regex as u8, doc_sig(doc))
}

/// 검색 키가 바뀐 경우에만 일치 줄 목록을 다시 만든다(불변이면 즉시 반환).
fn rebuild(doc: &mut EditorDoc) {
    let k = key(doc);
    if doc.find.built == k {
        return;
    }
    doc.find.built = k;
    doc.find.matches.clear();
    if doc.find.query.is_empty() {
        return;
    }
    let ci = doc.find.ci;
    let re = if needs_regex(&doc.find) { compiled(&doc.find) } else { None };
    if needs_regex(&doc.find) && re.is_none() {
        return; // 잘못된 정규식.
    }
    let needle = if ci { doc.find.query.to_lowercase() } else { doc.find.query.clone() };
    let hit = |line: &str| -> bool {
        if let Some(re) = &re {
            re.is_match(line)
        } else if ci {
            line.to_lowercase().contains(&needle)
        } else {
            line.contains(&needle)
        }
    };
    // 줄을 한 번만 순회(편집기 종류별). line_at 인덱싱은 일반 편집기에서 O(n^2)이라 피한다.
    let mut ms = Vec::new();
    if let Some(e) = &doc.edit {
        for i in 0..e.rope.len_lines() {
            if hit(&e.line_string(i)) {
                ms.push(i);
            }
        }
    } else if let Some(b) = &doc.big {
        for i in 0..b.line_count() {
            if hit(&b.line(i)) {
                ms.push(i);
            }
        }
    } else {
        for (i, line) in doc.text.split('\n').enumerate() {
            if hit(line) {
                ms.push(i);
            }
        }
    }
    doc.find.matches = ms;
}

/// 찾기 옵션에 맞는 정규식 패턴 문자열. 정규식 모드면 query 그대로, 아니면 escape.
/// 전체 단어면 \b로 감싼다(literal·regex 공통).
fn pattern(f: &FindState) -> String {
    let base = if f.regex { f.query.clone() } else { regex::escape(&f.query) };
    if f.whole {
        format!(r"\b(?:{base})\b")
    } else {
        base
    }
}

/// 정규식/전체단어/대소문자무시가 필요한 경우의 컴파일된 정규식(plain 검색은 None으로 빠른 경로).
pub(crate) fn compiled(f: &FindState) -> Option<regex::Regex> {
    regex::RegexBuilder::new(&pattern(f)).case_insensitive(f.ci).build().ok()
}

impl FindState {
    /// 검색 조건만 담은 사본. 통째로 복제하지 않는 이유는 `filter_backup`이 **문서 전체**를
    /// 들고 있을 수 있어서다 — 바꾸기 한 번에 파일을 통째로 한 벌 더 복사하게 된다.
    pub(crate) fn search_only(&self) -> FindState {
        FindState {
            query: self.query.clone(),
            replace: self.replace.clone(),
            ci: self.ci,
            whole: self.whole,
            regex: self.regex,
            ..Default::default()
        }
    }
}

/// 이 찾기가 정규식 엔진을 써야 하는지(정규식·전체단어 모드).
pub(crate) fn needs_regex(f: &FindState) -> bool {
    f.regex || f.whole
}

/// 찾기 바를 그린다(에디터 본문 위). 일치 줄로 scroll_to를 설정한다.
pub(crate) fn find_bar(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) {
    // 바꾸기는 편집 가능한 문서에서만 — 읽기 전용 뷰어(big)는 제외하고 rope는 포함한다.
    let viewer = doc.big.is_some();
    // 필터(일치 줄만 보기)는 아직 String 경로 전용이다(문서를 통째로 갈아 끼우는 방식이라).
    let plain_only = doc.big.is_some() || doc.edit.is_some();
    ui.horizontal_wrapped(|ui| {
        ui.label("\u{1f50d}");
        let q = ui.add(egui::TextEdit::singleline(&mut doc.find.query).desired_width(160.0).hint_text(tr(lang, "find.placeholder")));
        let enter = q.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("\u{25bc}").on_hover_text(tr(lang, "find.next")).clicked() || enter {
            find_line(doc, true);
        }
        if ui.button("\u{25b2}").on_hover_text(tr(lang, "find.prev")).clicked() {
            find_line(doc, false);
        }
        // 일치 개수: 작은 파일은 입력 즉시 갱신, 큰 파일은 검색(▼/▲) 후 표시.
        if line_count(doc) <= 50_000 {
            rebuild(doc);
        }
        if doc.find.built == key(doc) && !doc.find.query.is_empty() {
            let n = doc.find.matches.len();
            let txt = match doc.find.matches.iter().position(|&m| m == doc.find.cur) {
                _ if n == 0 => "0".to_string(),
                Some(p) => format!("{}/{}", p + 1, n),
                None => format!("{n}"),
            };
            ui.label(txt);
        }
        ui.toggle_value(&mut doc.find.ci, "Aa").on_hover_text(tr(lang, "find.ci"));
        ui.toggle_value(&mut doc.find.whole, "\u{2502}W\u{2502}").on_hover_text(tr(lang, "find.whole"));
        ui.toggle_value(&mut doc.find.regex, ".*").on_hover_text(tr(lang, "find.regex"));
        if needs_regex(&doc.find) && !doc.find.query.is_empty() && compiled(&doc.find).is_none() {
            ui.colored_label(egui::Color32::from_rgb(240, 120, 120), "\u{26a0}").on_hover_text(tr(lang, "find.badregex"));
        }
        if !viewer {
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut doc.find.replace).desired_width(140.0).hint_text(tr(lang, "find.replace")));
            if ui.button(tr(lang, "find.replaceall")).clicked() {
                crate::editorreplace::replace_all(doc);
            }
        }
        if !plain_only {
            // 필터 토글: 켜면 일치 줄만, 다시 누르면 원본 복원(filter_backup 보관).
            if ui.selectable_label(doc.find.filter_backup.is_some(), tr(lang, "find.filter")).on_hover_text(tr(lang, "find.filterhint")).clicked() {
                if let Some(orig) = doc.find.filter_backup.take() {
                    doc.text = orig;
                } else {
                    doc.find.filter_backup = Some(doc.text.clone());
                    doc.text = crate::editorreplace::filter_lines(&doc.text, &doc.find);
                }
                doc.dirty = true;
            }
        }
        ui.separator();
        let g = ui.add(egui::TextEdit::singleline(&mut doc.find.goto).desired_width(60.0).hint_text(tr(lang, "find.goto")));
        if g.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some((line0, col)) = crate::editorloc::parse_goto(&doc.find.goto) {
                doc.jump_to_line(line0); // 열 지정 시 그 열로 커서(VS Code식 줄:열).
                if let Some(c) = col { doc.find.pending_cursor = Some(crate::termlink::line_col_to_offset(&doc.text, line0, c)); }
            }
        }
    });
}

/// 현재 위치 기준 다음/이전 일치 줄로 이동(캐시된 matches 사용, 끝에서 순환).
fn find_line(doc: &mut EditorDoc, forward: bool) {
    rebuild(doc);
    let ms = &doc.find.matches;
    if ms.is_empty() {
        return;
    }
    let cur = doc.find.cur;
    let pos = if forward {
        ms.iter().position(|&m| m > cur).unwrap_or(0)
    } else {
        ms.iter().rposition(|&m| m < cur).unwrap_or(ms.len() - 1)
    };
    doc.jump_to_line(ms[pos]); // 일치 줄로 이동(스크롤+커서) — goto·북마크와 동일.
}

/// 찾기 설정대로 바꾼 결과 텍스트(순수). 정규식·전체단어·대소문자무시면 정규식 엔진,
fn line_count(doc: &EditorDoc) -> usize {
    if let Some(e) = &doc.edit {
        return e.rope.len_lines();
    }
    match &doc.big {
        Some(b) => b.line_count(),
        None => doc.text.split('\n').count().max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::{compiled, FindState};

    fn fs(query: &str, regex: bool, whole: bool, ci: bool) -> FindState {
        FindState { query: query.into(), regex, whole, ci, ..Default::default() }
    }

    #[test]
    fn whole_word_respects_boundaries() {
        let re = compiled(&fs("cat", false, true, false)).unwrap();
        assert!(re.is_match("a cat sat"));
        assert!(!re.is_match("category")); // 부분 단어 제외
        assert!(!re.is_match("scat"));
    }

    #[test]
    fn nonregex_query_is_escaped() {
        let re = compiled(&fs("a.b", false, false, false)).unwrap();
        assert!(re.is_match("a.b"));
        assert!(!re.is_match("axb")); // '.'은 리터럴(이스케이프)
    }

    #[test]
    fn regex_mode_is_wildcard() {
        let re = compiled(&fs("a.b", true, false, false)).unwrap();
        assert!(re.is_match("axb")); // '.'은 와일드카드
    }

    #[test]
    fn ci_flag_ignores_case() {
        let re = compiled(&fs("Cat", false, false, true)).unwrap();
        assert!(re.is_match("CAT"));
    }

    #[test]
    fn invalid_regex_yields_none() {
        assert!(compiled(&fs("a(", true, false, false)).is_none());
    }

    #[test]
    fn filter_keeps_matching_lines() {
        use crate::editorreplace::filter_lines;
        let t = "error: boom\ninfo: ok\nERROR: bad\nwarn: hmm";
        assert_eq!(filter_lines(t, &fs("error", false, false, false)), "error: boom");
        assert_eq!(filter_lines(t, &fs("error", false, false, true)), "error: boom\nERROR: bad"); // 대소문자 무시.
        assert_eq!(filter_lines(t, &fs("err", true, false, false)), "error: boom"); // 정규식.
        assert_eq!(filter_lines(t, &fs("err", false, true, false)), ""); // 전체단어: 'err'은 'error' 부분→불일치.
        assert_eq!(filter_lines(t, &fs("", false, false, false)), t); // 빈 query → 원문.
    }
}
