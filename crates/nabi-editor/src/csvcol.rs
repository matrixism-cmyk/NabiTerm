//! 초대용량 파일에서 **지금 몇 번째 칸에 있는지** 알려 준다(배치 AC).
//!
//! 서버 로그와 내보낸 자료는 대개 구분자로 나뉜 표이고, 그런 파일이 수 GB로 자란다.
//! 칸이 서른 개쯤 되면 화면에서 세로줄이 안 보여 **지금 보고 있는 값이 몇 번째 칸인지**
//! 알 수 없다. EmEditor가 CSV 편집으로 자리를 잡은 이유가 이것이다.
//!
//! ## 왜 칸을 맞춰 그리지 않는가
//!
//! 칸 너비를 맞추려면 **문서 전체**의 그 칸을 다 봐야 한다. 그런데 이 편집기가 존재하는
//! 이유가 "문서 전체를 훑지 않는다"이다(`textview` 헤더). 보이는 줄만 보고 너비를 정하면
//! 스크롤할 때마다 칸이 들썩여서, 없는 것보다 나쁘다.
//!
//! 그래서 **현재 줄 하나만** 본다. 그것만으로 "몇 번째 칸인가"에는 답할 수 있고, 그 답이
//! 수 GB 파일에서도 공짜다.
//!
//! ## 머리글은 왜 첫 줄만 보는가
//!
//! 칸 이름을 보여 주려면 머리글이 필요한데, 그것도 **첫 줄 하나**면 된다. 첫 줄이 머리글이
//! 아닌 파일도 많지만, 그때는 그 줄의 값이 이름 자리에 나올 뿐 해가 없다.

/// 흔한 구분자 — 쉼표·탭·세미콜론·파이프.
///
/// 공백은 넣지 않는다. 로그 문장에는 공백이 널려 있어서, 공백을 구분자로 보면 아무 줄이나
/// 표로 읽힌다 — 맞을 때보다 틀릴 때가 훨씬 많다.
const DELIMS: [char; 4] = [',', '\t', ';', '|'];

/// 줄에서 가장 그럴듯한 구분자를 고른다. 후보가 없으면 `None`(표가 아니다).
///
/// 가장 많이 나온 것을 고르되 **최소 두 번**은 나와야 한다. 한 번뿐이면 우연일 수 있고,
/// 우연을 표로 읽으면 엉뚱한 칸 번호를 자신 있게 말하게 된다.
pub fn guess_delim(line: &str) -> Option<char> {
    let mut best: Option<(char, usize)> = None;
    for d in DELIMS {
        let n = line.matches(d).count();
        if n >= 2 && best.is_none_or(|(_, b)| n > b) {
            best = Some((d, n));
        }
    }
    best.map(|(d, _)| d)
}

/// 따옴표를 아는 칸 나누기 — `"a,b",c` 는 두 칸이다.
///
/// 따옴표를 무시하면 값 안의 쉼표에서 칸이 갈려, 뒤쪽 칸 번호가 통째로 밀린다.
/// 그러면 "3번째 칸"이라는 안내가 오히려 사람을 헷갈리게 만든다.
pub fn split_fields(line: &str, delim: char) -> Vec<&str> {
    let (mut out, mut start, mut quoted) = (Vec::new(), 0usize, false);
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c == delim && !quoted => {
                out.push(&line[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

/// 글자 위치 `col`(0부터)이 몇 번째 칸인가 — **1부터** 센다(사람이 세는 방식).
pub fn field_at(line: &str, delim: char, col: usize) -> usize {
    let mut quoted = false;
    let mut n = 1usize;
    for (ci, (_, c)) in line.char_indices().enumerate() {
        if ci >= col {
            break;
        }
        match c {
            '"' => quoted = !quoted,
            c if c == delim && !quoted => n += 1,
            _ => {}
        }
    }
    n
}

/// 상태바에 붙일 짧은 안내 — `3/12 user_id`. 표가 아니면 `None`.
///
/// `header` 는 첫 줄(있으면). 칸 수가 머리글보다 많으면 이름 없이 번호만 보여 준다 —
/// 없는 이름을 지어내면 사용자가 그것을 믿는다.
pub fn hint(line: &str, header: Option<&str>, col: usize) -> Option<String> {
    let delim = guess_delim(line)?;
    let total = split_fields(line, delim).len();
    let at = field_at(line, delim, col).min(total);
    let name = header
        .and_then(|h| split_fields(h, delim).get(at - 1).map(|s| s.trim().trim_matches('"').to_string()))
        .filter(|s| !s.is_empty());
    Some(match name {
        Some(n) => format!("{at}/{total} {n}"),
        None => format!("{at}/{total}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_sentence_is_not_a_table() {
        // 로그 문장을 표로 읽으면 아무 줄에나 칸 번호가 붙는다.
        assert_eq!(guess_delim("2026-08-28 서버가 응답하지 않습니다"), None);
        assert_eq!(guess_delim("한 번만, 쉼표"), None, "한 번뿐이면 우연일 수 있다");
    }

    #[test]
    fn the_most_common_delimiter_wins() {
        assert_eq!(guess_delim("a,b,c,d"), Some(','));
        assert_eq!(guess_delim("a\tb\tc\td"), Some('\t'));
        assert_eq!(guess_delim("a;b;c"), Some(';'));
        // 쉼표가 더 많으면 쉼표.
        assert_eq!(guess_delim("a,b,c;d"), Some(','));
    }

    #[test]
    fn quotes_keep_a_value_together() {
        let f = split_fields(r#""a,b",c,d"#, ',');
        assert_eq!(f, vec![r#""a,b""#, "c", "d"], "값 안의 쉼표에서 갈리면 안 된다");
    }

    #[test]
    fn field_numbers_start_at_one() {
        let line = "id,name,age";
        assert_eq!(field_at(line, ',', 0), 1);
        assert_eq!(field_at(line, ',', 3), 2, "쉼표를 지나면 다음 칸");
        assert_eq!(field_at(line, ',', 8), 3);
    }

    #[test]
    fn a_comma_inside_quotes_does_not_advance_the_field() {
        let line = r#""a,b",c"#;
        assert_eq!(field_at(line, ',', 3), 1, "따옴표 안이라 아직 첫 칸");
        assert_eq!(field_at(line, ',', 6), 2);
    }

    #[test]
    fn the_hint_names_the_column_from_the_header() {
        let header = "id,user_id,age";
        let line = "1,1234,30";
        assert_eq!(hint(line, Some(header), 3).as_deref(), Some("2/3 user_id"));
    }

    #[test]
    fn no_header_means_numbers_only() {
        assert_eq!(hint("1,2,3", None, 0).as_deref(), Some("1/3"));
    }

    #[test]
    fn a_missing_header_name_is_not_invented() {
        // 칸이 머리글보다 많으면 이름 없이 번호만 — 없는 이름을 지어내면 사용자가 믿는다.
        let header = "id,name";
        assert_eq!(hint("1,2,3", Some(header), 5).as_deref(), Some("3/3"));
    }

    #[test]
    fn a_non_table_line_has_no_hint() {
        assert_eq!(hint("그냥 문장입니다", Some("id,name"), 2), None);
    }

    #[test]
    fn hangul_columns_count_by_character_not_byte() {
        // 글자 위치로 세야 한다 — 바이트로 세면 한글 파일에서 칸이 어긋난다.
        let line = "이름,나이,도시";
        assert_eq!(field_at(line, ',', 3), 2, "'이름,' 다음은 둘째 칸");
        assert_eq!(hint(line, None, 3).as_deref(), Some("2/3"));
    }
}
