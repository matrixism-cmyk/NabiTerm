//! rope 버퍼에 순수 텍스트 변환을 적용한다(정렬·대소문자·인코딩 등 40여 개 명령 공용).
//!
//! 이 명령들은 전부 `fn(&str) -> String`이라 어느 버퍼에나 쓸 수 있는데, 지금까지는
//! **작은 문서(String 경로)에서만** 메뉴에 나왔다. 2MB를 넘어 rope 편집기로 열리는 순간
//! 40여 개가 통째로 사라졌다 — 정작 정렬·중복 제거가 절실한 건 큰 파일 쪽이다.
//!
//! 선택이 있으면 선택 구간만, 없으면 문서 전체에 적용한다(VS Code·EmEditor와 같다).

use crate::editbuf::{EditBuf, EditKind};

/// 전체 문서에 적용할 수 있는 최대 문자 수. 변환은 전부 O(n) 문자열 연산이라
/// 아주 큰 문서에서는 UI 스레드가 눈에 띄게 멈춘다. 선택 구간은 이 제한을 받지 않는다
/// (사용자가 범위를 스스로 좁힌 것이므로).
pub const WHOLE_DOC_LIMIT: usize = 8 * 1024 * 1024;

/// 변환을 적용할 구간을 정한다 — 선택이 있으면 그 범위, 없으면 문서 전체.
///
/// 전체가 한도를 넘으면 `None`(메뉴를 막고 이유를 알린다).
pub fn target_range(sel: Option<(usize, usize)>, len: usize) -> Option<(usize, usize)> {
    match sel {
        Some((a, b)) if a < b => Some((a, b)),
        _ if len <= WHOLE_DOC_LIMIT => Some((0, len)),
        _ => None,
    }
}

impl EditBuf {
    /// 순수 변환을 적용한다. 적용했으면 true.
    ///
    /// 되돌리기는 한 단위로 묶인다 — 정렬 한 번을 Ctrl+Z 한 번으로 되돌릴 수 있어야 한다.
    pub fn apply_transform(&mut self, f: impl Fn(&str) -> String) -> bool {
        let Some((a, b)) = target_range(self.selection(), self.rope.len_chars()) else {
            return false;
        };
        let src: String = self.rope.slice(a..b).to_string();
        let out = f(&src);
        if out == src {
            return false; // 바뀐 게 없으면 undo 기록도 남기지 않는다.
        }
        self.begin_transform();
        self.rope.remove(a..b);
        self.rope.insert(a, &out);
        // 변환한 구간을 그대로 선택해 둔다 — 연달아 다른 변환을 걸기 쉽다.
        let end = a + out.chars().count();
        self.set_cursor(a);
        self.move_head(end);
        self.finish_transform();
        true
    }

    /// 변환 전용 undo 경계 — 앞뒤 타자와 절대 묶이지 않게 한다.
    fn begin_transform(&mut self) {
        self.undo.push((self.rope.clone(), self.cursor()));
        self.redo.clear();
        self.undo_open = false;
        self.last_kind = Some(EditKind::Delete);
    }

    fn finish_transform(&mut self) {
        self.undo_open = false; // 다음 편집은 새 묶음.
        self.last_time = None;
        self.ensure_visible = true;
        self.sync_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editbuf::EditBuf;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "
")
    }

    #[test]
    fn applies_to_whole_document_without_selection() {
        let mut b = buf("b\na\nc");
        assert!(b.apply_transform(|s| s.to_uppercase()));
        assert_eq!(b.rope.to_string(), "B\nA\nC");
    }

    #[test]
    fn applies_only_to_selection() {
        let mut b = buf("abcdef");
        b.set_cursor(1);
        b.move_head(3); // "bc"만 선택.
        assert!(b.apply_transform(|s| s.to_uppercase()));
        assert_eq!(b.rope.to_string(), "aBCdef");
    }

    /// 변환 한 번은 되돌리기 한 번 — 글자별로 쪼개지면 쓸 수 없다.
    #[test]
    fn one_transform_is_one_undo() {
        let mut b = buf("hello");
        b.apply_transform(|s| s.to_uppercase());
        b.undo();
        assert_eq!(b.rope.to_string(), "hello");
    }

    /// 결과가 같으면 아무 일도 하지 않는다(빈 undo 단위가 쌓이지 않게).
    #[test]
    fn no_op_transform_changes_nothing() {
        let mut b = buf("ABC");
        assert!(!b.apply_transform(|s| s.to_uppercase()));
        assert!(b.undo.is_empty(), "바뀐 게 없으면 undo도 없어야 한다");
    }

    /// 변환 뒤에는 결과 구간이 선택돼 있어야 한다(연속 변환).
    #[test]
    fn selects_result_after_transform() {
        let mut b = buf("ab");
        b.apply_transform(|_| "xyz".to_string());
        assert_eq!(b.selection(), Some((0, 3)));
    }

    /// 선택이 없을 때만 크기 한도를 본다 — 선택은 사용자가 이미 범위를 좁힌 것이다.
    #[test]
    fn size_limit_applies_only_to_whole_document() {
        let big = WHOLE_DOC_LIMIT + 1;
        assert_eq!(target_range(None, big), None, "너무 크면 전체 적용은 막는다");
        assert_eq!(target_range(Some((0, 5)), big), Some((0, 5)), "선택은 크기와 무관");
        assert_eq!(target_range(None, 10), Some((0, 10)));
        assert_eq!(target_range(Some((3, 3)), 10), Some((0, 10)), "빈 선택은 선택 아님");
    }
}

#[cfg(test)]
mod replace_on_rope {
    use crate::editbuf::EditBuf;
    use crate::editorfind::FindState;
    use crate::editorreplace::replaced;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "\n")
    }

    /// 대용량(rope) 문서에서도 전체 바꾸기가 되고, 되돌리기 한 번으로 원래대로 온다.
    #[test]
    fn replace_all_on_rope_is_one_undo() {
        let mut b = buf("cat\ncat\ndog");
        let f = FindState { query: "cat".into(), replace: "fox".into(), ..Default::default() };
        assert!(b.apply_transform(|s| replaced(s, &f)));
        assert_eq!(b.rope.to_string(), "fox\nfox\ndog");
        b.undo();
        assert_eq!(b.rope.to_string(), "cat\ncat\ndog", "한 번의 취소로 되돌아와야 한다");
    }

    /// 선택이 있으면 그 구간만 바뀐다(문서 전체를 건드리지 않는다).
    #[test]
    fn replace_respects_selection() {
        let mut b = buf("cat cat cat");
        b.set_cursor(0);
        b.move_head(7); // 앞의 두 개만.
        let f = FindState { query: "cat".into(), replace: "fox".into(), ..Default::default() };
        b.apply_transform(|s| replaced(s, &f));
        assert_eq!(b.rope.to_string(), "fox fox cat");
    }
}
