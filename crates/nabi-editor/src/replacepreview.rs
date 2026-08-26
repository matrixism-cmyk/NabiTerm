//! **바꾸기 미리보기** — 무엇이 어떻게 바뀌는지 먼저 보여 준다.
//!
//! "모두 바꾸기"는 되돌릴 수 있지만, 되돌리기와 **먼저 보는 것**은 다른 일이다. 되돌리기는
//! 잘못된 뒤에 쓰는 것이고 그때는 이미 무엇이 잘못됐는지 알아야 한다 — 300곳이 바뀐 뒤
//! 그중 어디가 틀렸는지 찾는 것은 바꾸기 전에 보는 것보다 훨씬 어렵다.
//!
//! 특히 정규식에서 그렇다. `.*`를 하나 잘못 넣으면 한 줄이 통째로 먹히는데, 그 사실은
//! 바꾸고 나서야 보인다.
//!
//! ## 몇 개만 보여 준다
//!
//! 300곳이 바뀐다면 300개를 다 보여 줄 필요가 없다. 앞의 몇 개를 보면 **패턴이 맞는지**
//! 알 수 있고, 그것이 미리보기의 목적이다. 전체 개수는 따로 말한다.

/// 미리보기에 담을 최대 건수.
pub const MAX: usize = 20;

/// 바뀔 자리 하나 — 줄 번호(1부터)와 바뀌기 전/후의 그 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub line: usize,
    pub before: String,
    pub after: String,
}

/// 미리보기 결과.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    /// 보여 줄 몇 건.
    pub shown: Vec<Change>,
    /// 바뀔 자리 전체 개수.
    pub total: usize,
}

/// 각 일치 자리(문자 오프셋)를 바꿨을 때 그 줄이 어떻게 되는지 만든다.
///
/// `text`는 문서 전체, `at`은 일치 시작 오프셋(문자), `len`은 일치 길이(문자).
/// 같은 줄에 여러 곳이 걸리면 **그 줄을 한 번만** 보여 준다 — 같은 줄이 열 번 나오면
/// 훑어보는 데 방해만 된다. 다만 전체 개수에는 다 센다.
pub fn build(text: &str, hits: &[(usize, usize)], to: &str) -> Preview {
    let chars: Vec<char> = text.chars().collect();
    let starts = line_starts(&chars);
    let mut shown: Vec<Change> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for (at, len) in hits {
        if shown.len() >= MAX {
            break;
        }
        let li = line_of(&starts, *at);
        if seen.contains(&li) {
            continue;
        }
        seen.push(li);
        let (ls, le) = (starts[li], line_end(&chars, &starts, li));
        let before: String = chars[ls..le].iter().collect();
        // 그 줄 안에서만 바꿔 본다 — 문서 전체를 복사하지 않는다(큰 파일에서 중요하다).
        let (rs, re) = (at.saturating_sub(ls), (at + len).saturating_sub(ls).min(le - ls));
        let mut after = String::new();
        after.extend(chars[ls..ls + rs].iter());
        after.push_str(to);
        after.extend(chars[ls + re..le].iter());
        shown.push(Change { line: li + 1, before, after });
    }
    Preview { shown, total: hits.len() }
}

fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            v.push(i + 1);
        }
    }
    v
}

fn line_of(starts: &[usize], at: usize) -> usize {
    match starts.binary_search(&at) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    }
}

fn line_end(chars: &[char], starts: &[usize], li: usize) -> usize {
    let next = starts.get(li + 1).copied().unwrap_or(chars.len());
    let mut e = next;
    // 줄 끝 문자는 보여 주지 않는다 — 미리보기 줄에 개행이 섞이면 표가 깨진다.
    if e > starts[li] && chars.get(e - 1) == Some(&'\n') {
        e -= 1;
    }
    if e > starts[li] && chars.get(e - 1) == Some(&'\r') {
        e -= 1;
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_shows_the_line_before_and_after() {
        let t = "let a = 1;\nlet b = 2;\n";
        // "let"이 두 곳(0, 11).
        let p = build(t, &[(0, 3), (11, 3)], "var");
        assert_eq!(p.total, 2);
        assert_eq!(p.shown[0].line, 1);
        assert_eq!(p.shown[0].before, "let a = 1;");
        assert_eq!(p.shown[0].after, "var a = 1;");
        assert_eq!(p.shown[1].after, "var b = 2;");
    }

    /// **같은 줄은 한 번만** 보여 준다 — 열 번 나오면 훑어보는 데 방해만 된다.
    #[test]
    fn one_line_appears_once_however_many_hits_it_has() {
        let t = "aaa aaa aaa\nbbb\n";
        let p = build(t, &[(0, 3), (4, 3), (8, 3)], "x");
        assert_eq!(p.shown.len(), 1, "같은 줄이 여러 번 나왔다");
        assert_eq!(p.total, 3, "개수는 다 세야 한다");
    }

    /// 줄 끝 문자는 미리보기에 들어가지 않는다(표가 깨진다).
    #[test]
    fn the_line_ending_is_not_shown() {
        let t = "abc\n";
        let p = build(t, &[(0, 1)], "X");
        assert_eq!(p.shown[0].before, "abc");
        assert_eq!(p.shown[0].after, "Xbc");
    }

    #[test]
    fn crlf_is_not_shown_either() {
        let t = "abc\r\ndef\r\n";
        let p = build(t, &[(5, 1)], "X");
        assert_eq!(p.shown[0].before, "def");
        assert_eq!(p.shown[0].after, "Xef");
    }

    /// **몇 개만 보여 주되 전체 개수는 말한다.**
    #[test]
    fn it_caps_what_it_shows_but_counts_everything() {
        let t: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let hits: Vec<(usize, usize)> = (0..100).map(|i| (i * 7 + i.to_string().len().saturating_sub(1), 4)).collect();
        let p = build(&t, &hits, "LINE");
        assert!(p.shown.len() <= MAX);
        assert_eq!(p.total, 100);
    }

    /// 빈 문자열로 바꾸는 것(=지우기)도 보여 줘야 한다 — 오히려 이게 더 위험하다.
    #[test]
    fn replacing_with_nothing_is_previewed_too() {
        let t = "keep DELETE keep\n";
        let p = build(t, &[(5, 6)], "");
        assert_eq!(p.shown[0].after, "keep  keep");
    }

    /// 여러 줄을 삼키는 일치(정규식 `.*\n.*`)에서도 줄이 깨지지 않아야 한다.
    #[test]
    fn a_match_spanning_lines_does_not_break_the_row() {
        let t = "aaa\nbbb\nccc\n";
        let p = build(t, &[(0, 7)], "X");
        assert_eq!(p.shown[0].line, 1);
        assert!(!p.shown[0].after.contains('\n'), "미리보기 줄에 개행이 들어갔다");
    }

    #[test]
    fn no_hits_gives_an_empty_preview() {
        assert_eq!(build("abc", &[], "x"), Preview::default());
    }

    /// 한글처럼 여러 바이트인 글자에서도 자리가 어긋나면 안 된다(문자 기준이어야 한다).
    #[test]
    fn multibyte_text_keeps_its_positions() {
        let t = "가나다 라마바\n";
        let p = build(t, &[(4, 3)], "X");
        assert_eq!(p.shown[0].before, "가나다 라마바");
        assert_eq!(p.shown[0].after, "가나다 X");
    }
}
