//! nabiPad 정렬 변형 — 자연(숫자 인식)·대소문자 무시. 전체 문서 줄 단위.
//! 순수 `fn(&str)->String`라 Edit 메뉴 xform 체인에 그대로 얹히고 단위 테스트가 쉽다.

use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

/// 자연 정렬: 연속 숫자 런을 값으로 비교(file2 < file10).
pub(crate) fn sort_natural(text: &str) -> String {
    sort_by(text, natural_cmp)
}

/// 대소문자 무시 사전식 정렬.
pub(crate) fn sort_ci(text: &str) -> String {
    sort_by(text, |a, b| a.to_lowercase().cmp(&b.to_lowercase()))
}

/// 내림차순(Z→A) 사전식 정렬.
pub(crate) fn sort_desc(text: &str) -> String {
    sort_by(text, |a, b| b.cmp(a))
}

/// 줄 길이(문자 수) 오름차순 정렬(동률은 사전식).
pub(crate) fn sort_by_length(text: &str) -> String {
    sort_by(text, |a, b| a.chars().count().cmp(&b.chars().count()).then_with(|| a.cmp(b)))
}

/// 줄 선두의 정수 값으로 정렬(숫자 없으면 0). 로그·번호 목록을 수치 순서로.
pub(crate) fn sort_numeric(text: &str) -> String {
    fn key(l: &str) -> i64 {
        let s = l.trim_start();
        let neg = s.starts_with('-');
        let digits: String = s.trim_start_matches('-').chars().take_while(char::is_ascii_digit).collect();
        let v = digits.parse::<i64>().unwrap_or(0);
        if neg {
            -v
        } else {
            v
        }
    }
    let mut v: Vec<&str> = text.split('\n').collect();
    v.sort_by_key(|l| key(l));
    v.join("\n")
}

/// 정렬 + 중복 제거(고유 정렬). 정렬 후 인접 중복을 없앤다.
pub(crate) fn sort_unique(text: &str) -> String {
    let mut v: Vec<&str> = text.split('\n').collect();
    v.sort_unstable();
    v.dedup();
    v.join("\n")
}

fn sort_by(text: &str, cmp: impl Fn(&str, &str) -> Ordering) -> String {
    let mut v: Vec<&str> = text.split('\n').collect();
    v.sort_by(|a, b| cmp(a, b)); // 안정 정렬(동순위 입력 순서 유지).
    v.join("\n")
}

/// 숫자 런은 값으로, 그 외 문자는 코드포인트로 비교한다.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, nb) = (take_digits(&mut ai), take_digits(&mut bi));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                // 선행 0 제거 후 자릿수 → 사전식으로 수치 대소 결정.
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                ai.next();
                bi.next();
                if x != y {
                    return x.cmp(&y);
                }
            }
        }
    }
}

fn take_digits(it: &mut Peekable<Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&c) = it.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        s.push(c);
        it.next();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_orders_numbers_by_value() {
        assert_eq!(sort_natural("file10\nfile2\nfile1"), "file1\nfile2\nfile10");
        assert_eq!(sort_natural("img9\nimg10\nimg100"), "img9\nimg10\nimg100");
    }

    #[test]
    fn natural_handles_leading_zeros_and_text() {
        assert_eq!(sort_natural("a\nb2\nb10\nb1"), "a\nb1\nb2\nb10");
        assert_eq!(sort_natural("v1.9\nv1.10\nv1.2"), "v1.2\nv1.9\nv1.10");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(sort_ci("Banana\napple\nCherry"), "apple\nBanana\nCherry");
    }

    #[test]
    fn descending_and_by_length() {
        assert_eq!(sort_desc("a\nc\nb"), "c\nb\na");
        assert_eq!(sort_by_length("ccc\na\nbb"), "a\nbb\nccc");
    }

    #[test]
    fn unique_sort() {
        assert_eq!(sort_unique("b\na\nb\nc\na"), "a\nb\nc");
    }

    #[test]
    fn numeric_by_leading_value() {
        assert_eq!(sort_numeric("10\n2\n1"), "1\n2\n10"); // 값 기준(사전식 아님).
        assert_eq!(sort_numeric("5 a\n10 b\n2 c"), "2 c\n5 a\n10 b"); // 선두 숫자 키.
        assert_eq!(sort_numeric("-3\n5\n-10"), "-10\n-3\n5"); // 음수.
    }

    #[test]
    fn numeric_sort() {
        assert_eq!(sort_numeric("10 a\n2 b\n1 c"), "1 c\n2 b\n10 a");
    }
}
