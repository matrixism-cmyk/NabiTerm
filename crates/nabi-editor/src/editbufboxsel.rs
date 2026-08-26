//! 컬럼(박스) 선택 + 멀티범위 편집(T6-3/A3) — Alt+드래그로 세로 띠를 선택한다.
//!
//! Selection이 처음부터 "정렬·비겹침 범위 집합"이라(editsel), 박스 선택은 **줄당 범위
//! 하나**로 표현된다. 짧은 줄은 줄 끝 캐럿이 되어 타자 시 모든 줄에 입력된다(VS Code식).
//! 편집(삽입/삭제)은 아래에서 위로 적용해 앞 범위의 오프셋이 흔들리지 않게 한다.
//! undo는 기존 스냅샷 방식이 그대로 안전하다(begin이 rope 전체를 저장).

use crate::editbuf::EditBuf;
use crate::editsel::{Range, Selection};

impl EditBuf {
    /// (줄, 표시열) 두 점 사이의 박스 선택을 만든다. head_side가 오른쪽이면 각 범위의
    /// head를 오른쪽 끝에 둔다(Shift+화살표 확장 방향 보존).
    pub fn box_select(&mut self, a: (usize, usize), b: (usize, usize)) {
        let (l0, l1) = (a.0.min(b.0), a.0.max(b.0));
        let (c0, c1) = (a.1.min(b.1), a.1.max(b.1));
        let last = self.rope.len_lines().saturating_sub(1);
        let mut ranges = Vec::new();
        for line in l0..=l1.min(last) {
            let ls = self.rope.line_to_char(line);
            let d = self.disp_line(line);
            let n = self.line_len(line);
            let sa = d.to_src(c0).min(n);
            let sb = d.to_src(c1).min(n);
            ranges.push(Range { anchor: ls + sa, head: ls + sb });
        }
        if ranges.is_empty() {
            return;
        }
        // 커서(주 범위)는 드래그 중인 줄(b쪽)에 둔다.
        let primary_line = b.0.clamp(l0, l1.min(last)) - l0;
        let mut sel = Selection::caret(0);
        // caret(0) 자리표시를 첫 범위로 대체하고 나머지를 push.
        for (i, r) in ranges.into_iter().enumerate() {
            if i == 0 {
                sel = Selection::single(r.anchor, r.head);
            } else {
                sel.push(r);
            }
        }
        // push가 마지막을 primary로 만들므로, 필요한 줄로 재지정한다.
        let want = primary_line.min(sel.len().saturating_sub(1));
        for _ in 0..sel.len() {
            if sel_primary_index(&sel) == want {
                break;
            }
            rotate_primary(&mut sel);
        }
        self.sel = sel;
        self.ensure_visible = true;
    }

    /// 멀티범위 삽입 — 각 범위를 s로 대체하고 캐럿을 삽입 끝에 둔다(멀티캐럿 유지 = 계속 타자).
    pub fn insert_multi(&mut self, s: &str) {
        let ranges: Vec<Range> = self.sel.ranges().to_vec();
        if ranges.len() <= 1 {
            self.insert(s);
            return;
        }
        self.mark_hl(ranges.first().map(|r| r.start()).unwrap_or(0));
        self.push_snapshot();
        let ins = s.chars().count();
        let mut carets = Vec::with_capacity(ranges.len());
        for r in ranges.iter().rev() {
            let (a, b) = (r.start(), r.end());
            if b > a {
                self.rope.remove(a..b);
            }
            self.rope.insert(a, s);
        }
        // 앞에서부터 캐럿 위치 재계산(각 범위가 s로 대체된 뒤의 오프셋).
        let mut shift: isize = 0;
        for r in &ranges {
            let (a, b) = (r.start(), r.end());
            let at = (a as isize + shift) as usize + ins;
            carets.push(Range::caret(at));
            shift += ins as isize - (b - a) as isize;
        }
        self.replace_selection(carets);
    }

    /// 멀티범위 삭제 — 범위는 지우고, 캐럿은 back(백스페이스)/앞(delete) 한 글자.
    pub fn delete_multi(&mut self, back: bool) {
        let ranges: Vec<Range> = self.sel.ranges().to_vec();
        if ranges.len() <= 1 {
            if back { self.backspace() } else { self.delete() }
            return;
        }
        self.mark_hl(ranges.first().map(|r| r.start()).unwrap_or(0));
        self.push_snapshot();
        let len = self.rope.len_chars();
        // 실제 지울 구간으로 확장(캐럿이면 한 글자).
        let cuts: Vec<(usize, usize)> = ranges
            .iter()
            .map(|r| {
                let (a, b) = (r.start(), r.end());
                if b > a {
                    (a, b)
                } else if back {
                    (a.saturating_sub(1), a)
                } else {
                    (a, (a + 1).min(len))
                }
            })
            .collect();
        for &(a, b) in cuts.iter().rev() {
            if b > a {
                self.rope.remove(a..b);
            }
        }
        let mut shift = 0usize;
        let mut carets = Vec::with_capacity(cuts.len());
        for &(a, b) in &cuts {
            carets.push(Range::caret(a - shift));
            shift += b - a;
        }
        self.replace_selection(carets);
    }

    /// 스냅샷을 undo에 쌓는다(멀티 편집은 항상 새 묶음 — 예측 가능한 한 번의 취소).
    fn push_snapshot(&mut self) {
        self.undo.push((self.rope.clone(), self.cursor()));
        self.redo.clear();
        self.undo_open = false;
        self.dirty = true;
    }

    fn replace_selection(&mut self, carets: Vec<Range>) {
        let mut sel = Selection::caret(carets.first().map(|r| r.head).unwrap_or(0));
        for r in carets.into_iter().skip(1) {
            sel.push(r);
        }
        self.sel = sel;
        self.ensure_visible = true;
        self.sync_dirty();
    }
}

/// Selection 내부 primary 인덱스 접근(회전용 최소 헬퍼).
fn sel_primary_index(s: &Selection) -> usize {
    let p = s.primary();
    s.ranges().iter().position(|r| *r == p).unwrap_or(0)
}

/// primary를 다음 범위로 옮긴다(원하는 줄에 맞출 때까지 회전).
fn rotate_primary(s: &mut Selection) {
    let i = sel_primary_index(s);
    let next = (i + 1) % s.ranges().len();
    let r = s.ranges()[next];
    let mut ns = Selection::single(r.anchor, r.head);
    for (j, rr) in s.ranges().iter().enumerate() {
        if j != next {
            ns.push(*rr);
        }
    }
    // push가 마지막을 primary로 만드는 것을 피해, 원하는 범위를 마지막에 넣지 않고
    // set_primary로 지정한다.
    ns.set_primary(r);
    *s = ns;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "LF")
    }

    /// **복사한 덩어리의 줄이 밀리면 안 된다.**
    ///
    /// 짧은 줄에서는 사각 구간이 비는데, `selected_text`가 빈 범위를 걸러 내고 있었다.
    /// 그러면 3번 줄이 비어 있었다는 사실이 사라지고 4번 줄이 3번 자리로 올라온다 —
    /// 들쭉날쭉한 로그에서 열을 떠 갈 때 바로 드러나는데, 붙여넣기 전에는 모른다.
    #[test]
    fn copying_a_ragged_box_keeps_every_row() {
        let mut eb = buf("aaaXXbbb\ncc\ndddYYeee\n");
        eb.box_select((0, 3), (2, 5));
        let got = eb.selected_text();
        let rows: Vec<&str> = got.split("\n").collect();
        assert_eq!(rows.len(), 3, "줄이 밀렸다: {got:?}");
        assert_eq!(rows[0], "XX");
        assert_eq!(rows[1], "", "짧은 줄이 빈 줄로 남아야 한다");
        assert_eq!(rows[2], "YY");
    }

    /// 멀티커서만 세워 둔 상태(전부 캐럿)에서는 복사할 것이 없다.
    #[test]
    fn carets_alone_copy_nothing() {
        let mut eb = buf("abc\ndef\n");
        eb.box_select((0, 1), (1, 1));
        assert!(eb.sel.ranges().iter().all(|r| r.is_caret()));
        assert_eq!(eb.selected_text(), "");
    }

    #[test]
    fn box_select_makes_one_range_per_line() {
        let mut eb = buf("abcdef\nabc\nabcdef\n");
        eb.box_select((0, 2), (2, 4));
        assert_eq!(eb.sel.len(), 3, "3줄 박스 = 범위 3개");
        // 짧은 줄(abc)은 줄 끝으로 클램프.
        let mid = eb.sel.ranges()[1];
        assert_eq!((mid.start(), mid.end()), (9, 10), "abc 줄은 c(2..3)만");
    }

    #[test]
    fn multi_insert_types_on_every_line() {
        let mut eb = buf("aa\nbb\ncc\n");
        eb.box_select((0, 1), (2, 1)); // 각 줄 1열 캐럿 박스.
        eb.insert_multi("X");
        assert_eq!(eb.rope.to_string(), "aXa\nbXb\ncXc\n");
        assert_eq!(eb.sel.len(), 3, "타자 후에도 멀티캐럿 유지");
        // 한 번의 undo로 전부 되돌아간다.
        eb.undo();
        assert_eq!(eb.rope.to_string(), "aa\nbb\ncc\n");
    }

    #[test]
    fn multi_delete_and_backspace() {
        let mut eb = buf("abcd\nabcd\n");
        eb.box_select((0, 1), (1, 3)); // 각 줄 bc 선택.
        eb.delete_multi(false);
        assert_eq!(eb.rope.to_string(), "ad\nad\n");
        // 캐럿 상태에서 백스페이스 = 각 줄 앞 글자 삭제.
        eb.delete_multi(true);
        assert_eq!(eb.rope.to_string(), "d\nd\n");
    }

    #[test]
    fn multi_copy_joins_lines() {
        let mut eb = buf("abcd\nefgh\n");
        eb.box_select((0, 1), (1, 3));
        assert_eq!(eb.selected_text(), "bc\nfg", "컬럼 복사는 줄바꿈으로 연결");
    }
}
