//! **버퍼 안의 낱말로 자동완성** — LSP가 없는 파일에서도 긴 이름을 다시 치지 않게.
//!
//! LSP 완성은 이미 있다(`lspcomp`). 다만 서버가 붙는 파일에서만 된다 — 설정 파일, 로그,
//! 스크립트, 메모에는 서버가 없다. 그런 파일에도 **긴 낱말은 있다.**
//!
//! 그래서 문서에 이미 있는 낱말을 그대로 쓴다. 무엇이 옳은지 판단하지 않고, 이 문서에서
//! 쓰인 적이 있는지만 본다. 그 편이 오히려 예측 가능하다 — 화면에 보이는 것만 나온다.
//!
//! ## 어떤 순서로 보여 주는가
//!
//! 1. **가까운 것 먼저.** 지금 고치는 자리 근처의 낱말이 대개 지금 필요한 것이다.
//! 2. 같은 거리면 **짧은 것 먼저** — 긴 이름은 짧은 이름을 품는 경우가 많고, 짧은 쪽을
//!    고르고 계속 치는 편이 손이 덜 간다.
//!
//! ## 어디까지만 하는가
//!
//! 아주 큰 문서에서 매번 전체를 훑으면 타이핑이 끊긴다. **커서 둘레만** 본다. 그 바깥의
//! 낱말은 후보에 안 나오는데, 그것은 못 찾은 것이 아니라 **보지 않기로 한 것**이다.

/// 커서 앞뒤로 이만큼 글자만 본다(양쪽 합쳐 대략 20만 자).
const WINDOW: usize = 100_000;
/// 이보다 짧은 낱말은 후보로 두지 않는다 — 치는 것이 더 빠르다.
const MIN_LEN: usize = 4;
/// 보여 줄 개수.
pub const MAX_HITS: usize = 8;

/// 낱말을 이루는 글자인가. 밑줄·숫자는 낱말의 일부로 본다(식별자가 그렇게 생겼다).
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 커서 바로 앞의 **치다 만 낱말**. 없으면 None.
pub fn prefix_at(text: &str, cursor_chars: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().take(cursor_chars).collect();
    let start = chars.iter().rposition(|c| !is_word(*c)).map(|i| i + 1).unwrap_or(0);
    let p: String = chars[start..].iter().collect();
    (p.chars().count() >= 2).then_some(p) // 한두 글자로는 후보가 너무 많다.
}

/// `prefix`로 시작하는 낱말들을 문서에서 찾는다(가까운 것·짧은 것 먼저, 중복 없음).
///
/// 치다 만 낱말 자신은 제외한다 — 자기 자신을 고를 수는 없다.
pub fn candidates(text: &str, cursor_chars: usize, prefix: &str) -> Vec<String> {
    if prefix.is_empty() {
        return Vec::new();
    }
    let total = text.chars().count();
    let from = cursor_chars.saturating_sub(WINDOW);
    let to = (cursor_chars + WINDOW).min(total);
    let mut hits: Vec<(usize, usize, String)> = Vec::new(); // (거리, 길이, 낱말)
    let mut word = String::new();
    let mut start = from;
    for (i, c) in text.chars().enumerate().skip(from).take(to - from) {
        if is_word(c) {
            if word.is_empty() {
                start = i;
            }
            word.push(c);
            continue;
        }
        take(&mut hits, &mut word, start, cursor_chars, prefix);
    }
    take(&mut hits, &mut word, start, cursor_chars, prefix);
    hits.sort_by_key(|h| (h.0, h.1));
    let mut out: Vec<String> = Vec::new();
    for (_, _, w) in hits {
        if !out.contains(&w) {
            out.push(w);
        }
        if out.len() >= MAX_HITS {
            break;
        }
    }
    out
}

/// 모은 낱말 하나를 후보로 담는다(조건에 맞을 때만). `word`는 비워진다.
fn take(hits: &mut Vec<(usize, usize, String)>, word: &mut String, start: usize, cur: usize, prefix: &str) {
    let w = std::mem::take(word);
    let n = w.chars().count();
    if n < MIN_LEN || n <= prefix.chars().count() || !w.starts_with(prefix) {
        return;
    }
    hits.push((start.abs_diff(cur), n, w));
}

#[cfg(test)]
mod tests {
    use super::{candidates, prefix_at};

    #[test]
    fn the_prefix_is_the_word_being_typed() {
        assert_eq!(prefix_at("let conf", 8).as_deref(), Some("conf"));
        assert_eq!(prefix_at("a.conf", 6).as_deref(), Some("conf"), "점에서 끊기지 않았다");
    }

    /// 한두 글자로는 후보가 너무 많아 방해만 된다.
    #[test]
    fn one_letter_is_not_enough_to_ask() {
        assert_eq!(prefix_at("let c", 5), None);
        assert_eq!(prefix_at("", 0), None);
        assert_eq!(prefix_at("let ", 4), None, "공백 뒤에서 낱말을 지어냈다");
    }

    #[test]
    fn a_word_from_the_document_is_offered() {
        let t = "configuration = 1\nlet conf";
        let got = candidates(t, t.chars().count(), "conf");
        assert_eq!(got, vec!["configuration"]);
    }

    /// **치다 만 낱말 자신은 후보가 아니다** — 자기를 고를 수는 없다.
    #[test]
    fn the_prefix_itself_is_not_a_candidate() {
        let t = "conf conf conf";
        assert!(candidates(t, t.chars().count(), "conf").is_empty());
    }

    /// 가까운 것이 먼저 — 지금 고치는 자리 근처의 낱말이 대개 지금 필요한 것이다.
    #[test]
    fn nearer_words_come_first() {
        let far = format!("alphabetical{}alphanumeric x", " ".repeat(500));
        let cur = far.chars().count();
        let got = candidates(&far, cur, "alpha");
        assert_eq!(got.first().map(String::as_str), Some("alphanumeric"), "{got:?}");
    }

    /// 같은 거리면 짧은 것 먼저(짧은 쪽을 고르고 계속 치는 편이 낫다).
    #[test]
    fn shorter_words_win_ties() {
        let t = "value valuewithalongtail v";
        let got = candidates(t, 0, "val");
        assert_eq!(got.first().map(String::as_str), Some("value"), "{got:?}");
    }

    #[test]
    fn the_same_word_is_only_offered_once() {
        let t = "config config config cfg";
        let got = candidates(t, 0, "con");
        assert_eq!(got, vec!["config"]);
    }

    /// 아주 짧은 낱말은 치는 것이 더 빠르다.
    #[test]
    fn very_short_words_are_not_offered() {
        let t = "abc abcd";
        let got = candidates(t, 0, "ab");
        assert_eq!(got, vec!["abcd"], "세 글자짜리까지 권했다");
    }

    /// 한글도 낱말이다(우리말 메모·문서에서도 뜻이 있다).
    #[test]
    fn hangul_words_count_as_words() {
        let t = "설정파일 = 1\n설";
        let got = candidates(t, t.chars().count(), "설정");
        assert_eq!(got, vec!["설정파일"]);
    }

    #[test]
    fn an_empty_prefix_asks_for_nothing() {
        assert!(candidates("anything at all", 5, "").is_empty());
    }
}
