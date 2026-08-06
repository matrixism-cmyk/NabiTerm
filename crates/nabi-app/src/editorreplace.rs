//! 찾기 결과로 **바꾸기·필터**를 적용한다 — 순수 변환은 여기, UI/이동은 editorfind.
//!
//! 바꾸기는 String 문서와 rope(대용량) 문서 둘 다 지원한다. 예전에는 rope 편집기에서
//! 바꾸기를 숨겨서, 큰 로그·덤프를 열어 놓고 찾기는 되는데 고치지는 못했다.

use crate::editor::EditorDoc;
use crate::editorfind::{compiled, needs_regex, FindState};

/// 평문이면 빠른 replace. 정규식 모드의 바꿀 내용은 `$1` 등 캡처 참조를 지원한다.
///
/// 잘못된 정규식이나 빈 query면 **원문 그대로** 돌려준다 — 실수로 문서를 망가뜨리지 않는다.
pub(crate) fn replaced(text: &str, f: &FindState) -> String {
    if f.query.is_empty() {
        return text.to_string();
    }
    if !needs_regex(f) && !f.ci {
        return text.replace(&f.query, &f.replace);
    }
    match compiled(f) {
        Some(re) => re.replace_all(text, f.replace.as_str()).into_owned(),
        None => text.to_string(),
    }
}

/// 전체 바꾸기. String 문서와 rope(대용량) 문서 둘 다 지원한다.
///
/// 예전에는 rope 편집기에서 바꾸기 자체를 숨겼다 — 찾기는 되는데 바꾸기만 안 되는 상태라,
/// 큰 로그·덤프를 열어 놓고 정작 고치지는 못했다.
pub(crate) fn replace_all(doc: &mut EditorDoc) {
    if doc.find.query.is_empty() {
        return;
    }
    if let Some(e) = doc.edit.as_mut() {
        // 선택이 있으면 그 구간만 — 변환 명령과 같은 규칙(되돌리기도 한 단위).
        let f = doc.find.search_only();
        e.apply_transform(|s| replaced(s, &f));
        doc.dirty = e.dirty;
        return;
    }
    doc.text = replaced(&doc.text, &doc.find);
    doc.dirty = true;
}

/// query·옵션에 일치하는 줄만 남긴 텍스트(필터/grep). 잘못된 정규식·빈 query면 원문 유지.
pub(crate) fn filter_lines(text: &str, f: &FindState) -> String {
    if f.query.is_empty() {
        return text.to_string();
    }
    let re = if needs_regex(f) { compiled(f) } else { None };
    if needs_regex(f) && re.is_none() {
        return text.to_string(); // 잘못된 정규식 → 변경 안 함.
    }
    let needle = if f.ci { f.query.to_lowercase() } else { f.query.clone() };
    let hit = |line: &str| -> bool {
        match &re {
            Some(re) => re.is_match(line),
            None if f.ci => line.to_lowercase().contains(&needle),
            None => line.contains(&needle),
        }
    };
    text.split('\n').filter(|l| hit(l)).collect::<Vec<_>>().join("\n")
}


#[cfg(test)]
mod replace_tests {
    use super::{replaced, FindState};

    fn f(q: &str, r: &str) -> FindState {
        FindState { query: q.into(), replace: r.into(), ..Default::default() }
    }

    #[test]
    fn plain_replace() {
        assert_eq!(replaced("a b a", &f("a", "X")), "X b X");
    }

    #[test]
    fn case_insensitive_uses_regex_engine() {
        let s = FindState { ci: true, ..f("ab", "X") };
        assert_eq!(replaced("AB ab Ab", &s), "X X X");
    }

    #[test]
    fn regex_supports_capture_refs() {
        let s = FindState { regex: true, ..f(r"(\w+)=(\w+)", "$2=$1") };
        assert_eq!(replaced("k=v", &s), "v=k");
    }

    /// 전체 단어 모드는 부분 일치를 건드리지 않는다.
    #[test]
    fn whole_word_only() {
        let s = FindState { whole: true, ..f("cat", "dog") };
        assert_eq!(replaced("cat concat cat", &s), "dog concat dog");
    }

    /// 잘못된 정규식·빈 검색어면 원문 그대로 — 실수로 문서를 망가뜨리면 안 된다.
    #[test]
    fn bad_regex_and_empty_query_are_noops() {
        let bad = FindState { regex: true, ..f("(unclosed", "X") };
        assert_eq!(replaced("keep me", &bad), "keep me");
        assert_eq!(replaced("keep me", &f("", "X")), "keep me");
    }
}
