//! **줄 끝 섞임 감지** — 말없이 온 파일을 바꾸지 않기 위해.
//!
//! `detect_eol`은 CRLF/LF/CR 중 **하나**를 고른다. 그리고 저장하면 `format_on_save`가
//! 온 파일을 그 하나로 통일한다. 원본이 섞여 있었다면 사용자가 건드리지도 않은 줄까지
//! 전부 바뀌고, git diff가 파일 전체로 부푼다. **한 줄 고쳤는데 500줄이 바뀐 것처럼 보인다.**
//!
//! 그래서 먼저 "섞였는가"를 알아야 한다. 섞이지 않은 파일(대부분)은 지금과 똑같이 동작하고,
//! 섞인 파일에서만 사용자에게 알린다.
//!
//! ## 왜 개수까지 세는가
//!
//! "섞였습니다"만으로는 무엇을 해야 할지 알 수 없다. CRLF 3줄 · LF 500줄이면 CRLF 세 줄이
//! 잘못 들어온 것이고, 반반이면 사연이 있는 파일이다. 판단은 사람이 한다 — 우리는 숫자를 준다.

/// 줄 끝 종류별 개수.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EolCounts {
    pub crlf: usize,
    pub lf: usize,
    pub cr: usize,
}

impl EolCounts {
    /// 두 종류 이상 나타나면 섞인 것이다.
    pub fn mixed(&self) -> bool {
        [self.crlf, self.lf, self.cr].iter().filter(|n| **n > 0).count() > 1
    }

    /// 가장 많은 종류(같으면 CRLF → LF → CR 순). 통일할 때 기본값으로 쓴다.
    pub fn dominant(&self) -> &'static str {
        if self.crlf >= self.lf && self.crlf >= self.cr && self.crlf > 0 {
            return "CRLF";
        }
        if self.lf >= self.cr && self.lf > 0 {
            return "LF";
        }
        if self.cr > 0 {
            return "CR";
        }
        "LF" // 줄 끝이 하나도 없으면(한 줄짜리) LF로 본다.
    }

    /// 화면에 낼 짧은 글. 섞이지 않았으면 종류 하나만.
    pub fn label(&self) -> String {
        if !self.mixed() {
            return self.dominant().to_string();
        }
        let mut parts = Vec::new();
        for (n, name) in [(self.crlf, "CRLF"), (self.lf, "LF"), (self.cr, "CR")] {
            if n > 0 {
                parts.push(format!("{name} {n}"));
            }
        }
        parts.join(" · ")
    }
}

/// 줄 끝을 종류별로 센다. **한 번만 훑는다** — 큰 파일에서 세 번 훑을 이유가 없다.
pub fn count_eols(text: &str) -> EolCounts {
    let b = text.as_bytes();
    let mut c = EolCounts::default();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' if i + 1 < b.len() && b[i + 1] == b'\n' => {
                c.crlf += 1;
                i += 2;
                continue;
            }
            b'\r' => c.cr += 1,
            b'\n' => c.lf += 1,
            _ => {}
        }
        i += 1;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_lf_file_is_not_mixed() {
        let c = count_eols("a\nb\nc\n");
        assert_eq!((c.crlf, c.lf, c.cr), (0, 3, 0));
        assert!(!c.mixed());
        assert_eq!(c.label(), "LF");
    }

    #[test]
    fn a_clean_crlf_file_is_not_mixed() {
        let c = count_eols("a\r\nb\r\n");
        assert_eq!((c.crlf, c.lf, c.cr), (2, 0, 0));
        assert!(!c.mixed());
        assert_eq!(c.label(), "CRLF");
    }

    /// **CRLF를 CR+LF 둘로 세면 온 파일이 섞인 것처럼 보인다** — 가장 쉬운 실수다.
    #[test]
    fn crlf_counts_as_one_not_two() {
        let c = count_eols("a\r\nb\r\n");
        assert_eq!(c.cr, 0, "CRLF의 CR을 따로 셌다");
        assert_eq!(c.lf, 0, "CRLF의 LF를 따로 셌다");
    }

    #[test]
    fn a_mixed_file_is_detected_with_counts() {
        let c = count_eols("a\r\nb\nc\nd\n");
        assert_eq!((c.crlf, c.lf), (1, 3));
        assert!(c.mixed());
        assert_eq!(c.label(), "CRLF 1 · LF 3");
    }

    /// 옛 맥 줄 끝(CR 단독)도 센다.
    #[test]
    fn lone_carriage_returns_are_counted() {
        let c = count_eols("a\rb\rc");
        assert_eq!((c.crlf, c.lf, c.cr), (0, 0, 2));
        assert!(!c.mixed());
    }

    #[test]
    fn all_three_can_appear_together() {
        let c = count_eols("a\r\nb\nc\rd");
        assert_eq!((c.crlf, c.lf, c.cr), (1, 1, 1));
        assert!(c.mixed());
    }

    /// 통일 기본값은 **가장 많은 쪽** — 적은 쪽을 고르면 더 많은 줄이 바뀐다.
    #[test]
    fn the_dominant_kind_wins() {
        assert_eq!(count_eols("a\r\nb\nc\nd\n").dominant(), "LF");
        assert_eq!(count_eols("a\r\nb\r\nc\n").dominant(), "CRLF");
        assert_eq!(count_eols("a\rb\rc\n").dominant(), "CR");
    }

    #[test]
    fn a_file_without_line_endings_is_lf() {
        let c = count_eols("one line");
        assert!(!c.mixed());
        assert_eq!(c.dominant(), "LF");
    }

    #[test]
    fn an_empty_file_is_harmless() {
        let c = count_eols("");
        assert_eq!(c, EolCounts::default());
        assert!(!c.mixed());
    }

    /// 파일 끝이 CR로 끝나도 인덱스를 넘겨 터지지 않는다.
    #[test]
    fn a_trailing_cr_does_not_overrun() {
        let c = count_eols("a\r");
        assert_eq!(c.cr, 1);
    }
}
