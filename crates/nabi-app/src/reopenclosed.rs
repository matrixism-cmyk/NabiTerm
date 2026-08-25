//! **최근에 닫은 문서 다시 열기** — 실수로 닫은 것을 되돌린다.
//!
//! 터미널 탭에는 이미 있다(`closed_sessions`). 문서에는 없었다 — 편집기에서 실수로 닫으면
//! 파일 열기 대화상자에서 다시 찾아야 했다. 브라우저가 Ctrl+Shift+T로 하는 그 일이다.
//!
//! ## 무엇을 기억하는가
//!
//! **경로만** 기억한다. 내용은 기억하지 않는다 — 저장하지 않고 닫은 내용은 이미 다른
//! 장치(`padrecover`)가 맡고 있고, 여기서 또 들고 있으면 같은 것을 두 곳에서 관리하게 된다.
//! 경로가 없는 문서(새 문서·비교 결과)는 되살릴 것이 없으므로 기억하지 않는다.

/// 기억할 개수. 너무 길면 옛날에 닫은 것이 목록을 채운다.
pub(crate) const CAP: usize = 10;

/// 닫은 문서를 기억한다. 같은 경로가 있으면 맨 앞으로 끌어올린다.
pub(crate) fn remember(list: &mut Vec<String>, path: &str) {
    let p = path.trim();
    if p.is_empty() {
        return; // 경로 없는 문서는 되살릴 것이 없다.
    }
    list.retain(|x| x != p);
    list.insert(0, p.to_string());
    list.truncate(CAP);
}

/// 다시 열 것을 꺼낸다(가장 최근 것). 목록이 비면 None.
pub(crate) fn take_latest(list: &mut Vec<String>) -> Option<String> {
    (!list.is_empty()).then(|| list.remove(0))
}

impl crate::app::NabiApp {
    /// 최근에 닫은 문서를 다시 연다(가장 최근 것부터).
    ///
    /// 파일이 그새 사라졌으면 그 항목을 버리고 **다음 것을 시도한다** — 없는 파일 하나
    /// 때문에 이 명령이 통째로 죽으면 안 된다.
    pub(crate) fn reopen_closed_doc(&mut self) {
        while let Some(p) = crate::reopenclosed::take_latest(&mut self.closed_docs) {
            let path = std::path::PathBuf::from(&p);
            if path.is_file() {
                self.open_editor_local(path);
                return;
            }
        }
        self.notify = Some((
            nabi_i18n::tr(self.lang, "editor.reopenclosed.none").to_string(),
            std::time::Instant::now(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_document_is_remembered() {
        let mut v = Vec::new();
        remember(&mut v, "C:/a.txt");
        assert_eq!(v, vec!["C:/a.txt".to_string()]);
    }

    /// **경로 없는 문서는 기억하지 않는다** — 되살릴 것이 없다(새 문서·비교 결과).
    #[test]
    fn a_document_without_a_path_is_not_remembered() {
        let mut v = Vec::new();
        remember(&mut v, "");
        remember(&mut v, "   ");
        assert!(v.is_empty());
    }

    /// 같은 파일을 두 번 닫으면 두 줄이 아니라 맨 앞 하나여야 한다.
    #[test]
    fn closing_the_same_file_twice_lifts_it() {
        let mut v = Vec::new();
        remember(&mut v, "a");
        remember(&mut v, "b");
        remember(&mut v, "a");
        assert_eq!(v, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn the_list_stays_bounded() {
        let mut v = Vec::new();
        for i in 0..30 {
            remember(&mut v, &format!("f{i}"));
        }
        assert_eq!(v.len(), CAP);
        assert_eq!(v[0], "f29");
    }

    /// 꺼내면 목록에서 빠진다 — 같은 것을 두 번 열지 않게.
    #[test]
    fn taking_removes_it_from_the_list() {
        let mut v = vec!["a".to_string(), "b".to_string()];
        assert_eq!(take_latest(&mut v).as_deref(), Some("a"));
        assert_eq!(v, vec!["b".to_string()]);
    }

    #[test]
    fn taking_from_empty_is_none() {
        let mut v: Vec<String> = Vec::new();
        assert!(take_latest(&mut v).is_none());
    }
}
