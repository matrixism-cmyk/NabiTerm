//! 붙여넣기 유니코드 속임 탐지 — **보이는 것과 실제 바이트가 다른** 문자를 잡는다.
//!
//! 제어문자 위생(`paste::sanitize_paste`)은 ESC·BEL 같은 C0/C1만 지운다. 그런데 붙여넣기
//! 공격의 현대적 형태는 제어문자가 아니라 **서식(Cf) 문자와 남의 문자 세트**를 쓴다:
//! 방향 재정의로 화면 순서를 뒤집거나(Trojan Source), 제로폭 문자를 명령 중간에 숨기거나,
//! 키릴 `а`처럼 라틴 `a`와 똑같이 생긴 글자로 도메인·명령을 위장한다.
//!
//! 지우는 대신 **경고하고 사용자가 고르게** 한다 — 제로폭·방향 문자는 아랍어/히브리어
//! 본문이나 이모지 결합(ZWJ)처럼 정당한 쓰임이 있어서 조용히 지우면 멀쩡한 텍스트가 깨진다.

/// 붙여넣기에서 발견된 속임 유형(사용자에게 보여줄 단위).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Deception {
    /// 방향 재정의·격리(U+202A~E, U+2066~9, U+200E/F) — 화면에 보이는 순서를 뒤집는다.
    Bidi,
    /// 제로폭·폭 없는 이음(U+200B~D, U+2060, U+FEFF) — 눈에 안 보이는 채로 끼어든다.
    ZeroWidth,
    /// 한 낱말 안에 라틴과 키릴/그리스가 섞임 — 호모글리프 위장(예: 키릴 `а`가 섞인 `pаypal`).
    MixedScript,
    /// 보통 공백이 아닌 유니코드 공백(NBSP 등) — 셸의 인자 분리가 눈에 보이는 것과 달라진다.
    OddSpace,
}

impl Deception {
    /// 사용자에게 보여줄 i18n 키.
    pub fn key(self) -> &'static str {
        match self {
            Deception::Bidi => "paste.risk.bidi",
            Deception::ZeroWidth => "paste.risk.zerowidth",
            Deception::MixedScript => "paste.risk.mixedscript",
            Deception::OddSpace => "paste.risk.oddspace",
        }
    }
}

/// 방향 재정의/격리 문자인가.
fn is_bidi(c: char) -> bool {
    matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}')
}

/// 폭이 없어 눈에 보이지 않는 문자인가(BOM 포함).
fn is_zero_width(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200d}' | '\u{2060}' | '\u{feff}')
}

/// 일반 스페이스·탭·개행이 아닌 유니코드 공백인가.
fn is_odd_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// 라틴 문자로 착각하기 쉬운 문자 세트(키릴·그리스)인가.
fn is_confusable_script(c: char) -> bool {
    matches!(c, '\u{0370}'..='\u{03ff}' | '\u{0400}'..='\u{04ff}')
}

/// 한 낱말 안에 ASCII 라틴 글자와 키릴/그리스가 섞였는가.
///
/// 낱말 단위로 보는 게 핵심이다. 한글 문서에 영어 단어가 섞이는 건 지극히 정상이라
/// 텍스트 전체를 보면 오탐만 난다 — 위장은 **한 낱말 안에서** 일어난다.
fn word_mixes_scripts(word: &str) -> bool {
    let mut latin = false;
    let mut confusable = false;
    for c in word.chars() {
        if c.is_ascii_alphabetic() {
            latin = true;
        } else if is_confusable_script(c) {
            confusable = true;
        }
        if latin && confusable {
            return true;
        }
    }
    false
}

/// 붙여넣기 텍스트의 속임 위험을 훑는다(중복 없이 정렬된 목록, 없으면 빈 벡터).
pub fn scan(text: &str) -> Vec<Deception> {
    let mut out = Vec::new();
    let mut push = |d: Deception| {
        if !out.contains(&d) {
            out.push(d);
        }
    };
    for c in text.chars() {
        if is_bidi(c) {
            push(Deception::Bidi);
        } else if is_zero_width(c) {
            push(Deception::ZeroWidth);
        } else if is_odd_space(c) {
            push(Deception::OddSpace);
        }
    }
    if text.split_whitespace().any(word_mixes_scripts) {
        push(Deception::MixedScript);
    }
    out.sort_unstable();
    out
}

/// 자동으로 고칠 수 있는 위험만 제거한다 — 보이지 않는 문자는 지우고, 이상한 공백은
/// 보통 스페이스로 바꾼다. **호모글리프는 건드리지 않는다**(어느 쪽이 진짜인지 알 수 없고,
/// 러시아어·그리스어 본문을 망가뜨린다). 그래서 제거 후에도 경고는 남을 수 있다.
pub fn strip(text: &str) -> String {
    text.chars()
        .filter(|&c| !is_bidi(c) && !is_zero_width(c))
        .map(|c| if is_odd_space(c) { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{scan, strip, Deception};

    #[test]
    fn clean_text_has_no_risk() {
        assert!(scan("git status").is_empty());
        assert!(scan("한글과 English가 섞인 정상 문장").is_empty()); // 낱말이 안 섞이면 정상.
        assert!(scan("日本語とにほんご").is_empty());
    }

    #[test]
    fn detects_bidi_override() {
        // Trojan Source: 방향 재정의로 화면 순서를 뒤집는다.
        assert_eq!(scan("rm -rf \u{202e}txt.exe"), vec![Deception::Bidi]);
    }

    #[test]
    fn detects_zero_width_and_odd_space() {
        assert_eq!(scan("cu\u{200b}rl example.com"), vec![Deception::ZeroWidth]);
        assert_eq!(scan("ls\u{00a0}-la"), vec![Deception::OddSpace]);
    }

    #[test]
    fn detects_homoglyph_only_inside_a_word() {
        // 키릴 а(U+0430)가 라틴 낱말 안에 섞임.
        assert_eq!(scan("p\u{0430}ypal.com"), vec![Deception::MixedScript]);
        // 러시아어 단어가 통째로 키릴이면 정상 텍스트다(오탐 금지).
        assert!(scan("привет мир").is_empty());
    }

    #[test]
    fn strip_removes_invisible_but_keeps_homoglyph() {
        assert_eq!(strip("cu\u{200b}rl"), "curl");
        assert_eq!(strip("a\u{202e}b"), "ab");
        assert_eq!(strip("ls\u{00a0}-la"), "ls -la");
        let h = "p\u{0430}ypal";
        assert_eq!(strip(h), h); // 호모글리프는 그대로 — 자동 판단 불가.
    }

    #[test]
    fn risks_are_deduped_and_sorted() {
        let r = scan("a\u{200b}b\u{200b}c\u{202e}d");
        assert_eq!(r, vec![Deception::Bidi, Deception::ZeroWidth]);
    }
}
