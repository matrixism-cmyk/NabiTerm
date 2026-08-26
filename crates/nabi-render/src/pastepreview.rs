//! **붙여넣기 미리보기** — 무엇을 붙이는지 실제로 보여 준다.
//!
//! 확인창은 "여러 줄입니다(12 lines)"라고 말하고 **첫 줄 60자**만 보여 줬다. 그런데 이
//! 창이 뜨는 이유는 붙여넣을 것이 위험할 수 있어서다 — 위험은 대개 **두 번째 줄부터**
//! 온다(첫 줄은 그럴듯한 주석이고 그 아래에 명령이 붙는 것이 고전적인 수법이다).
//!
//! 첫 줄만 보여 주는 확인창은 "확인했다"는 느낌만 주고 실제로는 아무것도 확인시키지
//! 못한다. 그건 없느니만 못하다.
//!
//! ## 무엇을 보여 주는가
//!
//! * 앞뒤 몇 줄씩. 가운데를 접고 **몇 줄을 접었는지 말한다**.
//! * 줄 번호를 붙인다 — 몇 번째 줄이 문제인지 말할 수 있어야 한다.
//! * 보이지 않는 문자는 **보이게 바꿔** 놓는다. 안 그러면 미리보기를 봐도 못 본다.

/// 앞뒤로 보여 줄 줄 수.
pub const HEAD: usize = 12;
pub const TAIL: usize = 4;
/// 한 줄에서 보여 줄 최대 글자 수.
pub const WIDTH: usize = 200;

/// 미리보기 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// (줄 번호 1부터, 글).
    Line(usize, String),
    /// 접은 줄 수.
    Elided(usize),
}

/// 붙여넣을 글을 미리보기 줄들로 만든다.
pub fn preview(text: &str) -> Vec<Row> {
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    if n <= HEAD + TAIL + 1 {
        return lines.iter().enumerate().map(|(i, l)| Row::Line(i + 1, visible(l))).collect();
    }
    let mut out: Vec<Row> = lines[..HEAD].iter().enumerate().map(|(i, l)| Row::Line(i + 1, visible(l))).collect();
    out.push(Row::Elided(n - HEAD - TAIL));
    out.extend(lines[n - TAIL..].iter().enumerate().map(|(i, l)| Row::Line(n - TAIL + i + 1, visible(l))));
    out
}

/// 보이지 않는 글자를 보이게 바꾼다. 미리보기에서까지 숨어 있으면 보여 주는 뜻이 없다.
///
/// 탭은 화살표로, 그 밖의 제어문자·양방향 표식·폭 없는 문자는 `·` 자리에 코드로 적는다.
/// 원문은 건드리지 않는다 — 여기서 만든 것은 **보여 주기용**이고, 붙여넣는 것은 원문이다.
pub fn visible(line: &str) -> String {
    let mut out = String::new();
    for (i, c) in line.chars().enumerate() {
        if i >= WIDTH {
            out.push('\u{2026}');
            break;
        }
        match c {
            '\t' => out.push('\u{2192}'),
            '\r' => out.push('\u{240d}'),
            c if is_hidden(c) => out.push_str(&format!("\u{2039}U+{:04X}\u{203a}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 눈에 보이지 않거나 글의 방향을 바꾸는 글자인가.
fn is_hidden(c: char) -> bool {
    let u = c as u32;
    (u < 0x20 && c != '\t')            // 제어문자
        || u == 0x7f                    // DEL
        || (0x200b..=0x200f).contains(&u) // 폭 없음·방향 표식
        || (0x202a..=0x202e).contains(&u) // 방향 뒤집기
        || (0x2066..=0x2069).contains(&u) // 방향 격리
        || u == 0xfeff // BOM
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn a_short_paste_is_shown_whole() {
        let rows = preview(&text(5));
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], Row::Line(1, "line 1".into()));
        assert!(!rows.iter().any(|r| matches!(r, Row::Elided(_))));
    }

    /// **가운데를 접되 몇 줄인지 말한다.** 조용히 자르면 본 것이 전부라고 믿게 된다.
    #[test]
    fn a_long_paste_is_elided_in_the_middle_and_says_how_much() {
        let rows = preview(&text(100));
        let elided: Vec<usize> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Elided(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(elided, vec![100 - HEAD - TAIL]);
        assert_eq!(rows.first(), Some(&Row::Line(1, "line 1".into())));
        assert_eq!(rows.last(), Some(&Row::Line(100, "line 100".into())));
    }

    /// **줄 번호는 원문 기준이어야 한다** — 접힌 뒤의 줄도 제 번호를 달고 나와야
    /// "97번째 줄이 이상하다"고 말할 수 있다.
    #[test]
    fn line_numbers_survive_the_elision() {
        let rows = preview(&text(50));
        let last_nums: Vec<usize> = rows
            .iter()
            .rev()
            .take(TAIL)
            .filter_map(|r| match r {
                Row::Line(n, _) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(last_nums, vec![50, 49, 48, 47]);
    }

    /// 숨은 글자는 **보이게** 바꾼다 — 미리보기에서도 숨어 있으면 보여 주는 뜻이 없다.
    #[test]
    fn invisible_characters_are_made_visible() {
        let s = visible("a\u{200b}b");
        assert!(s.contains("U+200B"), "{s}");
        assert!(!s.contains('\u{200b}'), "숨은 글자가 그대로 남았다");
    }

    /// 방향을 뒤집는 글자도 드러낸다(보이는 것과 실행되는 것이 달라지는 수법).
    #[test]
    fn direction_overrides_are_revealed() {
        assert!(visible("x\u{202e}y").contains("U+202E"));
    }

    #[test]
    fn a_tab_becomes_an_arrow_and_stays_one_character() {
        assert_eq!(visible("a\tb"), "a\u{2192}b");
    }

    /// 아주 긴 한 줄은 잘라 보여 주되 잘렸다는 표시를 남긴다.
    #[test]
    fn a_very_long_line_is_clipped_with_a_mark() {
        let s = visible(&"x".repeat(WIDTH + 50));
        assert!(s.chars().count() <= WIDTH + 1);
        assert!(s.ends_with('\u{2026}'));
    }

    #[test]
    fn an_empty_paste_gives_one_empty_row() {
        assert_eq!(preview(""), vec![Row::Line(1, String::new())]);
    }
}
