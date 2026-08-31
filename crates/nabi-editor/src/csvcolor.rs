//! **표의 칸마다 다른 색** — 어느 값이 몇 번째 칸인지 눈으로 바로 알게 한다.
//!
//! ## 무엇을 푸는가
//!
//! 칸을 맞춰 그리려면 그 칸의 가장 긴 값을 알아야 하고, 그러려면 문서 전체를 훑어야 한다.
//! 게다가 빈칸을 채워 넣으면 화면에 담기는 자료가 줄어든다.
//!
//! 색으로 나누면 **아무것도 채워 넣지 않고** 같은 일을 한다. 줄마다 칸이 어긋나 있으면
//! 색이 어긋나 보여서, 깨진 줄이 눈에 먼저 들어온다. Rainbow CSV(VS Code, 500만 다운로드)가
//! 이 방식으로 자리를 잡았다(2026-09-01 조사).
//!
//! ## 왜 이 색들인가 — Okabe-Ito
//!
//! 아무 색이나 여덟 개 고르면 색각 이상이 있는 사람에게는 몇 개가 같은 색이 된다.
//! Okabe-Ito 팔레트는 적록·청황 색각 이상 **모두에서 구분되도록** 실험으로 고른 여덟 색이고,
//! R 4.0 의 기본값이자 과학 도판의 사실상 표준이다(2026-09-01 조사).
//!
//! 여덟 중 **검정은 뺐다** — 어두운 바탕에서 글자가 사라진다. 그래서 일곱 색이 돌아간다.
//! 노랑(`#F0E442`)은 밝은 바탕에서 가장 약하다. 그래도 임의로 바꾸지 않았다 — 팔레트를
//! 손보는 순간 "색각 이상에서 구분된다"는 근거가 사라지기 때문이다.
//!
//! ## 따옴표
//!
//! 따옴표 안의 구분자는 칸을 나누지 않고, 따옴표 안의 개행도 같은 칸이다. 그래서 줄 단위가
//! 아니라 **글 전체를 한 번** 훑는다 — 줄 단위로 하면 여러 줄짜리 값에서 색이 어긋난다.

/// 칸 색(Okabe-Ito, 검정 제외). 칸 번호를 이 길이로 나눈 나머지가 색이다.
pub const COLORS: [egui::Color32; 7] = [
    egui::Color32::from_rgb(0xE6, 0x9F, 0x00), // 주황
    egui::Color32::from_rgb(0x56, 0xB4, 0xE9), // 하늘
    egui::Color32::from_rgb(0x00, 0x9E, 0x73), // 청록
    egui::Color32::from_rgb(0xF0, 0xE4, 0x42), // 노랑
    egui::Color32::from_rgb(0x00, 0x72, 0xB2), // 파랑
    egui::Color32::from_rgb(0xD5, 0x5E, 0x00), // 주홍
    egui::Color32::from_rgb(0xCC, 0x79, 0xA7), // 자주
];

/// 칸 번호에 붙일 색.
pub fn color_of(col: usize) -> egui::Color32 {
    COLORS[col % COLORS.len()]
}

/// 글을 칸 단위 조각으로 나눈다 — `(시작 바이트, 끝 바이트, 칸 번호)`.
///
/// 조각은 빈틈없이 이어지고 글 전체를 덮는다(구분자·개행도 앞 칸에 붙는다). 그래야 받는
/// 쪽이 그대로 이어 붙이기만 하면 원본이 된다 — 한 글자라도 새면 화면에서 글이 사라진다.
pub fn spans(text: &str, delim: char) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let (mut start, mut col, mut quoted) = (0usize, 0usize, false);
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        match c {
            '"' if quoted && it.peek().map(|(_, c)| *c) == Some('"') => {
                it.next(); // `""` 는 따옴표 한 글자 — 상태를 바꾸지 않는다.
            }
            '"' => quoted = !quoted,
            '\n' if !quoted => {
                // 개행까지 이 칸에 넣고, 다음 줄은 첫 칸부터.
                out.push((start, i + c.len_utf8(), col));
                start = i + c.len_utf8();
                col = 0;
            }
            c if c == delim && !quoted => {
                out.push((start, i + c.len_utf8(), col));
                start = i + c.len_utf8();
                col += 1;
            }
            _ => {}
        }
    }
    if start < text.len() {
        out.push((start, text.len(), col)); // 마지막 조각(개행 없이 끝난 경우).
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 조각을 이어 붙이면 원본이 되어야 한다 — 한 글자라도 새면 화면에서 글이 사라진다.
    fn rebuilt(text: &str, delim: char) -> String {
        spans(text, delim).iter().map(|(a, b, _)| &text[*a..*b]).collect()
    }

    #[test]
    fn the_pieces_add_back_up_to_the_original() {
        for t in ["a,b,c\n1,2,3\n", "a,b", "", "\n\n", "\"x,y\",z\n", "한글,값\n두,줄\n"] {
            assert_eq!(rebuilt(t, ','), t, "조각이 원본과 다르다: {t:?}");
        }
    }

    #[test]
    fn columns_are_numbered_from_zero_on_every_line() {
        let t = "a,b,c\n1,2,3\n";
        let cols: Vec<usize> = spans(t, ',').iter().map(|(_, _, c)| *c).collect();
        assert_eq!(cols, [0, 1, 2, 0, 1, 2], "줄이 바뀌면 다시 0부터다");
    }

    /// 따옴표 안의 구분자는 칸을 나누지 않는다.
    #[test]
    fn a_delimiter_inside_quotes_does_not_split() {
        let t = "\"x,y\",z\n";
        let cols: Vec<usize> = spans(t, ',').iter().map(|(_, _, c)| *c).collect();
        assert_eq!(cols, [0, 1], "따옴표 안의 쉼표를 칸으로 셌다");
    }

    /// 따옴표 안의 개행도 같은 칸이다 — 줄 단위로 세면 여기서 어긋난다.
    #[test]
    fn a_newline_inside_quotes_stays_in_the_same_column() {
        let t = "\"두\n줄\",z\n";
        let cols: Vec<usize> = spans(t, ',').iter().map(|(_, _, c)| *c).collect();
        assert_eq!(cols, [0, 1]);
        assert_eq!(rebuilt(t, ','), t);
    }

    /// 여러 바이트 글자 경계에서 잘리면 안 된다(한글은 세 바이트다).
    #[test]
    fn multibyte_text_is_not_cut_in_the_middle() {
        let t = "이름,값\n가나다,라마바\n";
        assert_eq!(rebuilt(t, ','), t);
        let first = spans(t, ',')[0];
        assert_eq!(&t[first.0..first.1], "이름,");
    }

    /// 색은 일곱 개가 돌아간다 — 여덟 번째 칸은 첫 색으로 돌아온다.
    #[test]
    fn colors_wrap_around_after_seven() {
        assert_eq!(color_of(0), color_of(7));
        assert_ne!(color_of(0), color_of(1));
        assert_eq!(COLORS.len(), 7, "검정을 뺀 Okabe-Ito 일곱 색");
    }

    /// 탭으로 나뉜 표도 같은 규칙이다.
    #[test]
    fn tabs_split_the_same_way() {
        let cols: Vec<usize> = spans("a\tb\n", '\t').iter().map(|(_, _, c)| *c).collect();
        assert_eq!(cols, [0, 1]);
    }
}
