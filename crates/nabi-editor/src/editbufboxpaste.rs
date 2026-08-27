//! 박스(열) 선택에 여러 줄을 붙여넣기(배치 Y E1) — **캐럿마다 한 줄씩.**
//!
//! 지금까지 붙여넣기는 타자와 같은 길로 갔다(`insert_multi`). 타자에는 그것이 맞다 —
//! 한 글자를 모든 줄에 넣는 것이 박스 선택의 뜻이다. 그런데 붙여넣기는 다르다.
//!
//! 세 줄을 박스로 잡고 세 줄짜리 클립보드를 붙여넣으면:
//!
//! | | 지금까지 | VS Code · EmEditor |
//! |---|---|---|
//! | 결과 | 각 캐럿에 세 줄이 통째로 = **아홉 줄** | 캐럿마다 한 줄씩 = **세 줄** |
//!
//! 열 단위로 자료를 옮기는 것(CSV 한 열을 다른 열로)이 박스 선택을 쓰는 가장 큰 이유인데
//! 정작 그때 쓸 수 없었다.
//!
//! ## 줄 수가 안 맞으면 나누지 않는다
//!
//! 캐럿이 셋인데 다섯 줄을 붙여넣으면 어떻게 할지 정답이 없다. 앞의 셋만 넣으면 둘을
//! 말없이 버리는 것이고, 남는 것을 마지막 캐럿에 몰면 그것도 예상 밖이다. **그래서 나누지
//! 않고 예전처럼 통째로 넣는다** — 사용자가 예상할 수 없는 결과를 만드느니, 예상할 수 있는
//! 옛 동작이 낫다. 나눌지 말지는 이 파일의 순수 함수 하나가 정하고, 그 판정만 시험한다.

/// 클립보드를 캐럿 수만큼 나눈다. 나눌 수 없으면 `None`(호출자는 통째로 넣는다).
///
/// * `carets` 가 2 미만이면 나눌 것이 없다 — 박스 선택이 아니다.
/// * 줄 수가 캐럿 수와 다르면 `None`.
/// * 줄 끝의 개행은 세지 않는다. `"a\nb\nc\n"` 은 **네 줄이 아니라 세 줄**이다 —
///   대부분의 편집기가 마지막 줄에 개행을 붙여 복사하기 때문에, 이것을 빈 줄로 세면
///   줄 수가 늘 하나씩 어긋나 나누기가 거의 동작하지 않는다.
/// * CRLF 는 CR 을 떼어 낸다. 떼지 않으면 붙여넣은 줄마다 보이지 않는 CR 이 박힌다.
pub fn split_for_carets(s: &str, carets: usize) -> Option<Vec<&str>> {
    if carets < 2 {
        return None;
    }
    let body = s.strip_suffix('\n').unwrap_or(s);
    let body = body.strip_suffix('\r').unwrap_or(body);
    let lines: Vec<&str> = body
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    (lines.len() == carets).then_some(lines)
}

use crate::editbuf::EditBuf;
use crate::editsel::Range;

impl EditBuf {
    /// 붙여넣기 — 박스 선택이고 줄 수가 맞으면 **캐럿마다 한 줄씩**, 아니면 예전대로 통째로.
    ///
    /// `insert_multi` 를 고치지 않고 옆에 둔 이유: 타자 경로가 그것에 의존한다.
    /// 한 글자를 모든 줄에 넣는 것은 박스 선택의 뜻이므로 그쪽은 그대로 두어야 한다.
    pub fn paste_multi(&mut self, s: &str) {
        let ranges: Vec<Range> = self.sel.ranges().to_vec();
        let Some(parts) = split_for_carets(s, ranges.len()) else {
            self.insert_multi(s);
            return;
        };
        self.mark_hl(ranges.first().map(|r| r.start()).unwrap_or(0));
        self.push_snapshot();
        // 뒤에서부터 고친다 — 앞 범위의 오프셋이 흔들리지 않게(insert_multi 와 같은 규칙).
        for (r, part) in ranges.iter().zip(parts.iter()).rev() {
            let (a, b) = (r.start(), r.end());
            if b > a {
                self.rope.remove(a..b);
            }
            self.rope.insert(a, part);
        }
        // 캐럿은 앞에서부터 다시 잰는다. 줄마다 넣은 길이가 다르므로 누적 이동량도 다르다.
        let mut carets = Vec::with_capacity(ranges.len());
        let mut shift: isize = 0;
        for (r, part) in ranges.iter().zip(parts.iter()) {
            let (a, b) = (r.start(), r.end());
            let ins = part.chars().count();
            let at = (a as isize + shift) as usize + ins;
            carets.push(Range::caret(at));
            shift += ins as isize - (b - a) as isize;
        }
        self.replace_selection(carets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_when_line_count_matches() {
        assert_eq!(split_for_carets("a\nb\nc", 3), Some(vec!["a", "b", "c"]));
    }

    #[test]
    fn trailing_newline_does_not_add_a_line() {
        // 대부분의 편집기가 마지막 줄에 개행을 붙여 복사한다. 이것을 빈 줄로 세면
        // 줄 수가 늘 하나씩 어긋나 나누기가 거의 동작하지 않는다.
        assert_eq!(split_for_carets("a\nb\nc\n", 3), Some(vec!["a", "b", "c"]));
    }

    #[test]
    fn crlf_is_stripped_from_every_line() {
        // 떼지 않으면 붙여넣은 줄마다 보이지 않는 CR이 박힌다.
        assert_eq!(split_for_carets("a\r\nb\r\nc\r\n", 3), Some(vec!["a", "b", "c"]));
    }

    #[test]
    fn mismatched_count_does_not_split() {
        // 셋인데 다섯 줄 — 어떻게 나눌지 정답이 없으므로 나누지 않는다.
        assert_eq!(split_for_carets("a\nb\nc\nd\ne", 3), None);
        assert_eq!(split_for_carets("a\nb", 3), None);
    }

    #[test]
    fn single_caret_is_not_a_box_selection() {
        assert_eq!(split_for_carets("a\nb", 1), None);
        assert_eq!(split_for_carets("a", 0), None);
    }

    #[test]
    fn empty_lines_in_the_middle_are_kept() {
        // 빈 줄도 한 줄이다 — 그 캐럿 자리는 비워 두는 것이 옳다.
        assert_eq!(split_for_carets("a\n\nc", 3), Some(vec!["a", "", "c"]));
    }

    #[test]
    fn single_line_clipboard_never_splits() {
        // 한 줄을 모든 캐럿에 넣는 것은 기존 동작이 맞다(열 전체를 같은 값으로 채우기).
        assert_eq!(split_for_carets("x", 3), None);
    }

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "LF")
    }

    #[test]
    fn paste_distributes_one_line_per_caret() {
        // 이것이 이 변경의 전부다 — 예전에는 세 캐럿 모두에 세 줄이 통째로 들어갔다.
        let mut eb = buf("aa
bb
cc
");
        eb.box_select((0, 1), (2, 1)); // 각 줄 1열 캐럿 박스.
        eb.paste_multi("1
22
333");
        assert_eq!(eb.rope.to_string(), "a1a
b22b
c333c
");
        assert_eq!(eb.sel.len(), 3, "붙여넣기 뒤에도 멀티캐럿 유지");
    }

    #[test]
    fn paste_undoes_in_one_step() {
        let mut eb = buf("aa
bb
cc
");
        eb.box_select((0, 1), (2, 1));
        eb.paste_multi("1
2
3");
        eb.undo();
        assert_eq!(eb.rope.to_string(), "aa
bb
cc
", "한 번의 undo 로 전부 되돌린다");
    }

    #[test]
    fn paste_replaces_the_selected_box() {
        // 범위를 잡은 박스면 그 글자를 지우고 대신 넣는다.
        let mut eb = buf("aXa
bYb
cZc
");
        eb.box_select((0, 1), (2, 2));
        eb.paste_multi("1
2
3");
        assert_eq!(eb.rope.to_string(), "a1a
b2b
c3c
");
    }

    #[test]
    fn mismatched_paste_keeps_the_old_behaviour() {
        // 줄 수가 안 맞으면 예전대로 통째로 — 예상할 수 없는 결과를 만들지 않는다.
        let mut eb = buf("aa
bb
cc
");
        eb.box_select((0, 1), (2, 1));
        eb.paste_multi("X");
        assert_eq!(eb.rope.to_string(), "aXa
bXb
cXc
");
    }
}
