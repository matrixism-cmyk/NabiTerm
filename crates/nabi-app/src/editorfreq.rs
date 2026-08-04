//! nabiPad 단어 빈도 분석(EmEditor "단어 수"/CyberChef 벤치마킹) — 텍스트를 빈도표로 치환.
//! 단어=유니코드 영숫자 연속(소문자 정규화). 빈도 내림차순, 동률은 사전식. 순수 함수.

use std::collections::HashMap;

/// 텍스트의 단어 빈도표를 "횟수<TAB>단어" 줄들로 만든다(빈도↓, 동률은 사전순).
pub(crate) fn word_frequency(t: &str) -> String {
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, counts: &mut HashMap<String, u32>| {
        if !cur.is_empty() {
            *counts.entry(std::mem::take(cur)).or_insert(0) += 1;
        }
    };
    for c in t.chars() {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        } else {
            flush(&mut cur, &mut counts);
        }
    }
    flush(&mut cur, &mut counts);

    let mut v: Vec<(String, u32)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.iter().map(|(w, n)| format!("{n}\t{w}")).collect::<Vec<_>>().join("\n")
}

/// 고유 단어 목록(소문자 정규화, 사전순, 한 줄에 하나) — 어휘 추출용.
pub(crate) fn word_list(t: &str) -> String {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut cur = String::new();
    for c in t.chars() {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        } else if !cur.is_empty() {
            set.insert(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        set.insert(cur);
    }
    set.into_iter().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_list_unique_sorted() {
        assert_eq!(word_list("The fox the Fox cat"), "cat\nfox\nthe");
        assert_eq!(word_list(""), "");
    }

    #[test]
    fn counts_and_orders() {
        // the=3, fox=2, 나머지 1 — 빈도↓ 동률 사전순.
        let out = word_frequency("The fox the Fox THE cat dog");
        assert_eq!(out, "3\tthe\n2\tfox\n1\tcat\n1\tdog");
        assert_eq!(word_frequency(""), "");
        assert_eq!(word_frequency("a-b a"), "2\ta\n1\tb"); // 구분자로 분리.
    }
}
