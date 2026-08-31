//! **칸 수가 어긋난 줄을 찾는다** — 큰 표에서 깨진 한 줄이 어디인지.
//!
//! ## 무엇을 푸는가
//!
//! 내보낸 자료가 어긋나는 일은 흔하다. 값 안에 구분자가 들어갔는데 따옴표가 빠졌거나,
//! 이어붙이다 한 줄이 잘렸거나, 손으로 고치다 칸 하나를 지웠거나.
//!
//! 그 한 줄을 눈으로 찾으려면 수천 줄을 세어야 한다. 대부분이 여섯 칸인데 한 줄만 일곱
//! 칸이라는 것은 **기계가 세면 즉시** 나온다. Rainbow CSV(VS Code, 500만 다운로드)가
//! 자리를 잡은 이유 가운데 하나가 이것이다(2026-09-01 조사).
//!
//! ## 왜 `parse_csv` 를 다시 쓰지 않는가
//!
//! `editorcsv::parse_csv` 는 칸을 **문자열로 만들어** 돌려준다. 우리는 개수만 세면 되는데
//! 수십만 줄짜리 파일에서 그 문자열을 다 만들면 원본만큼의 메모리를 한 번 더 쓴다.
//! 그리고 줄 번호가 필요하다 — 그쪽은 그것을 돌려주지 않는다(따옴표 안의 개행 때문에
//! 레코드 하나가 여러 줄에 걸칠 수 있어, 나중에 세면 어긋난다).
//!
//! 따옴표 규칙을 두 벌 적는 셈이라 어긋날 위험이 있다. 그래서 **두 파서가 같은 답을
//! 내는지 대조하는 시험**을 붙였다(`the_two_parsers_agree`) — 규칙이 갈라지면 그때 빨개진다.

/// 레코드 하나: 시작한 줄(0기반)과 칸 수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub line: usize,
    pub cols: usize,
}

/// 레코드마다 시작 줄과 칸 수를 센다. 칸 내용은 만들지 않는다.
///
/// 따옴표 안의 구분자와 개행은 세지 않는다(RFC4180). `""` 는 따옴표 하나를 뜻하므로
/// 따옴표 상태가 그대로 유지된다.
pub fn record_cols(text: &str, delim: char) -> Vec<Record> {
    let mut out = Vec::new();
    let (mut line, mut start, mut cols, mut quoted, mut any) = (0usize, 0usize, 1usize, false, false);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        any = true;
        match c {
            '"' if quoted && it.peek() == Some(&'"') => {
                it.next(); // `""` 는 따옴표 한 글자 — 상태를 바꾸지 않는다.
            }
            '"' => quoted = !quoted,
            // 따옴표 안의 개행은 같은 레코드다. 줄 번호만 늘린다.
            '\n' if quoted => line += 1,
            '\n' => {
                out.push(Record { line: start, cols });
                line += 1;
                start = line;
                cols = 1;
                any = false;
            }
            '\r' => {} // CRLF 의 CR 은 무시한다 — 줄 수를 두 번 세면 안 된다.
            c if c == delim && !quoted => cols += 1,
            _ => {}
        }
    }
    if any {
        out.push(Record { line: start, cols }); // 마지막 줄에 개행이 없을 수 있다.
    }
    out
}

/// 다수가 쓰는 칸 수와, 거기서 벗어난 레코드들.
///
/// 기준은 **가장 많이 나온 칸 수**다. 첫 줄(머리글)을 기준으로 삼는 방법도 있지만,
/// 머리글 자체가 깨진 파일에서는 나머지가 전부 "어긋남"이 되어 쓸모가 없어진다.
/// 같은 수가 여럿이면 **먼저 나온 쪽**을 쓴다(대개 머리글이다).
///
/// 빈 글이거나 레코드가 하나뿐이면 벗어난 것도 없다 — 비교할 대상이 없다.
pub fn odd_rows(text: &str, delim: char) -> (usize, Vec<Record>) {
    let recs = record_cols(text, delim);
    if recs.len() < 2 {
        return (recs.first().map(|r| r.cols).unwrap_or(0), Vec::new());
    }
    // 칸 수별로 몇 번 나왔는지. 순서를 지키려고 Vec 으로 센다(같은 수가 여럿일 때 먼저 나온 쪽).
    let mut tally: Vec<(usize, usize)> = Vec::new();
    for r in &recs {
        match tally.iter_mut().find(|(c, _)| *c == r.cols) {
            Some((_, n)) => *n += 1,
            None => tally.push((r.cols, 1)),
        }
    }
    // `max_by_key` 는 동점이면 **마지막**을 준다 — 우리 규칙은 먼저 나온 쪽이므로
    // 직접 접는다(시험 `tabs_work_the_same_way` 가 이 차이를 잡았다).
    let most = tally
        .iter()
        .copied()
        .reduce(|a, b| if b.1 > a.1 { b } else { a })
        .map(|(c, _)| c)
        .unwrap_or(0);
    let odd = recs.into_iter().filter(|r| r.cols != most).collect();
    (most, odd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_table_has_nothing_odd() {
        let (most, odd) = odd_rows("a,b,c\n1,2,3\n4,5,6\n", ',');
        assert_eq!(most, 3);
        assert!(odd.is_empty());
    }

    /// 이것이 이 기능의 전부다 — 한 줄만 다르면 그 줄을 짚는다.
    #[test]
    fn the_one_broken_row_is_reported_with_its_line() {
        let text = "a,b,c\n1,2,3\n4,5,6,7\n8,9,0\n";
        let (most, odd) = odd_rows(text, ',');
        assert_eq!(most, 3);
        assert_eq!(odd, [Record { line: 2, cols: 4 }], "셋째 줄(0기반 2)이 네 칸이다");
    }

    /// 따옴표 안의 구분자는 칸을 나누지 않는다.
    #[test]
    fn a_delimiter_inside_quotes_is_not_a_column_break() {
        let (most, odd) = odd_rows("a,b\n\"x,y\",z\n", ',');
        assert_eq!(most, 2);
        assert!(odd.is_empty(), "따옴표 안의 쉼표를 칸으로 셌다");
    }

    /// 따옴표 안의 개행은 레코드를 끝내지 않는다 — 다음 레코드의 **줄 번호**는 밀린다.
    #[test]
    fn a_newline_inside_quotes_stays_in_the_same_record() {
        let text = "a,b\n\"두\n줄\",z\n1,2\n";
        let recs = record_cols(text, ',');
        assert_eq!(recs.len(), 3, "레코드는 셋이다");
        assert_eq!(recs[2].line, 3, "마지막 레코드는 넷째 줄에서 시작한다");
        assert!(odd_rows(text, ',').1.is_empty());
    }

    /// `""` 는 따옴표 한 글자다 — 상태를 뒤집으면 그 뒤가 통째로 어긋난다.
    #[test]
    fn a_doubled_quote_is_one_character_not_a_toggle() {
        let (most, odd) = odd_rows("a,b\n\"he said \"\"hi\"\",x\",y\n", ',');
        assert_eq!(most, 2);
        assert!(odd.is_empty());
    }

    /// 마지막 줄에 개행이 없어도 센다.
    #[test]
    fn a_file_without_a_trailing_newline_still_counts_its_last_row() {
        let recs = record_cols("a,b\n1,2", ',');
        assert_eq!(recs.len(), 2);
    }

    /// CRLF 를 줄 두 개로 세면 줄 번호가 전부 밀린다.
    #[test]
    fn crlf_counts_as_one_line() {
        let recs = record_cols("a,b\r\n1,2\r\n3,4\r\n", ',');
        assert_eq!(recs.iter().map(|r| r.line).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn one_row_alone_has_nothing_to_compare_with() {
        assert!(odd_rows("a,b,c\n", ',').1.is_empty());
        assert!(odd_rows("", ',').1.is_empty());
    }

    /// 탭으로 나뉜 표도 같은 규칙이다.
    #[test]
    fn tabs_work_the_same_way() {
        let (most, odd) = odd_rows("a\tb\n1\t2\t3\n", '\t');
        assert_eq!(most, 2);
        assert_eq!(odd.len(), 1);
    }

    /// **두 파서가 같은 답을 내는가.** 따옴표 규칙을 두 벌 적었으므로, 갈라지면 여기서 잡는다.
    #[test]
    fn the_two_parsers_agree() {
        for text in [
            "a,b,c\n1,2,3\n",
            "\"x,y\",z\n1,2\n",
            "\"he said \"\"hi\"\"\",b\n",
            "a,b\n1,2",
            "a,,c\n,,\n",
        ] {
            let mine: Vec<usize> = record_cols(text, ',').iter().map(|r| r.cols).collect();
            let theirs: Vec<usize> =
                crate::editorcsv::parse_csv(text, ',').iter().map(|r| r.len()).collect();
            assert_eq!(mine, theirs, "두 파서가 다른 답을 냈다: {text:?}");
        }
    }
}
