//! **diff를 되돌려 한쪽 글로 만든다** — 견주기만 되고 고치는 길이 없던 것.
//!
//! `difflines::diff_lines`가 만드는 글은 이런 꼴이다:
//!
//! ```text
//!   같은 줄
//! - 왼쪽에만 있는 줄
//! + 오른쪽에만 있는 줄
//! ```
//!
//! 지금까지 이 글은 **읽기 전용**이었다. 두 파일이 어떻게 다른지는 알려 주는데, 한쪽을
//! 다른 쪽에 맞추려면 사람이 손으로 옮겨 적어야 했다.
//!
//! 여기서는 그 글을 되짚어 **왼쪽 글**과 **오른쪽 글**을 복원한다. 그러면 "왼쪽을 오른쪽에
//! 맞추기"가 곧 "오른쪽 글로 저장하기"가 된다.
//!
//! ## 왜 이 방향이 안전한가
//!
//! 덩이(hunk)를 하나씩 골라 적용하는 편집기도 많지만, 그러려면 diff 글이 원본과 정확히
//! 짝이 맞는지를 매번 확인해야 한다. 사용자가 diff 문서를 손으로 고쳤을 수도 있다.
//!
//! 대신 **전부 한쪽으로**만 만든다. 그러면 diff 글 자체가 유일한 근거이고, 결과가
//! 무엇인지도 눈으로 볼 수 있다(미리 보고 저장한다). 좁지만 틀릴 수 없는 길이다.

/// 어느 쪽 글을 만들 것인가.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// `-`와 공통 줄 = 왼쪽(원래) 글.
    Left,
    /// `+`와 공통 줄 = 오른쪽(새) 글.
    Right,
}

/// diff 글에서 한쪽 글을 복원한다.
///
/// 표시가 없는 줄(사용자가 덧붙인 메모, `--- a/파일` 같은 머리글)은 **버린다** — 그것을
/// 글에 남기면 저장했을 때 파일이 더럽혀진다.
pub fn restore(diff: &str, side: Side) -> String {
    let mut out = String::with_capacity(diff.len());
    for line in diff.lines() {
        let Some(kept) = keep(line, side) else { continue };
        out.push_str(kept);
        out.push('\n');
    }
    out
}

/// 이 줄이 그쪽 글에 남는가 — 남으면 표시를 뗀 알맹이.
fn keep(line: &str, side: Side) -> Option<&str> {
    // 표시는 두 글자다(`  `, `- `, `+ `). 빈 줄은 양쪽 모두의 빈 줄이다.
    if line.is_empty() {
        return Some("");
    }
    // **세 번째 글자의 자리**에서 자른다 — 두 번째에서 자르면 표시가 한 글자만 떨어져
    // 나가고 알맹이 앞에 공백이 남는다(한글 줄에서는 글자 폭 때문에 더 티가 안 난다).
    let (mark, rest) = line.split_at(line.char_indices().nth(2).map_or(line.len(), |(i, _)| i));
    match (mark, side) {
        ("  ", _) => Some(rest),
        ("- ", Side::Left) => Some(rest),
        ("+ ", Side::Right) => Some(rest),
        _ => None,
    }
}

/// 그쪽 글로 바꾸면 몇 줄이 늘고 몇 줄이 주는가 — 미리보기 요약용.
///
/// `(더할 줄, 뺄 줄)`. 왼쪽으로 되돌리면 `+`가 빠지고 `-`가 돌아온다.
pub fn counts(diff: &str, side: Side) -> (usize, usize) {
    let (mut add, mut del) = (0, 0);
    for line in diff.lines() {
        match (line.get(..2), side) {
            (Some("+ "), Side::Right) | (Some("- "), Side::Left) => add += 1,
            (Some("+ "), Side::Left) | (Some("- "), Side::Right) => del += 1,
            _ => {}
        }
    }
    (add, del)
}

#[cfg(test)]
mod tests {
    use super::{counts, restore, Side};

    const D: &str = "  a\n- b\n+ x\n  c\n";

    #[test]
    fn the_left_side_comes_back_whole() {
        assert_eq!(restore(D, Side::Left), "a\nb\nc\n");
    }

    #[test]
    fn the_right_side_comes_back_whole() {
        assert_eq!(restore(D, Side::Right), "a\nx\nc\n");
    }

    /// **표시 없는 줄은 버린다** — 머리글이나 메모가 파일에 섞이면 안 된다.
    #[test]
    fn unmarked_lines_are_dropped() {
        let d = "--- a.txt\n+++ b.txt\n  a\n+ b\n";
        assert_eq!(restore(d, Side::Right), "a\nb\n");
        assert_eq!(restore(d, Side::Left), "a\n");
    }

    /// 빈 줄은 양쪽 모두의 빈 줄이다(글의 문단이 무너지면 안 된다).
    #[test]
    fn blank_lines_survive_on_both_sides() {
        let d = "  a\n\n  b\n";
        assert_eq!(restore(d, Side::Left), "a\n\nb\n");
        assert_eq!(restore(d, Side::Right), "a\n\nb\n");
    }

    /// 같은 줄만 있으면 양쪽이 같다.
    #[test]
    fn an_identical_diff_gives_the_same_text_both_ways() {
        let d = "  a\n  b\n";
        assert_eq!(restore(d, Side::Left), restore(d, Side::Right));
    }

    #[test]
    fn an_empty_diff_gives_empty_text() {
        assert_eq!(restore("", Side::Left), "");
    }

    /// 줄 안에 표시처럼 생긴 글자가 있어도 **앞의 두 글자만** 본다.
    #[test]
    fn a_minus_inside_the_line_is_not_a_marker() {
        let d = "  a - b\n+ c + d\n";
        assert_eq!(restore(d, Side::Right), "a - b\nc + d\n");
    }

    #[test]
    fn the_counts_tell_what_will_change() {
        assert_eq!(counts(D, Side::Right), (1, 1));
        assert_eq!(counts(D, Side::Left), (1, 1));
        assert_eq!(counts("  a\n+ b\n+ c\n", Side::Right), (2, 0));
        assert_eq!(counts("  a\n+ b\n+ c\n", Side::Left), (0, 2));
    }

    /// 한글 줄에서도 표시를 정확히 뗀다(글자 폭이 아니라 글자 수로 자른다).
    #[test]
    fn hangul_lines_keep_their_first_character() {
        assert_eq!(restore("+ 안녕하세요\n", Side::Right), "안녕하세요\n");
        assert_eq!(restore("  설정\n", Side::Left), "설정\n");
    }
}
