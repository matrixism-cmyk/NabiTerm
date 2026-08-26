//! **색이 읽히는지 수치로 판단한다** — 상대 휘도와 대비비(WCAG 2.1).
//!
//! 터미널은 색으로 정보를 나른다. 그런데 전경색과 배경색을 따로 고를 수 있게 해 두면
//! 사용자는 **읽을 수 없는 조합**을 쉽게 만든다(짙은 회색 바탕에 짙은 파랑 글씨 같은).
//! 그때 "안 보인다"고 말해 주려면 먼저 **얼마나 안 보이는지 셀 줄** 알아야 한다.
//!
//! ## 왜 밝기를 그냥 평균 내지 않는가
//!
//! 사람 눈은 초록에 가장 민감하고 파랑에 가장 둔하다. RGB를 단순 평균하면 노랑과 파랑이
//! 비슷한 밝기로 나오는데, 실제로는 노랑이 훨씬 밝게 보인다. WCAG의 상대 휘도는 그
//! 민감도 차이를 가중치로 담고, 화면의 감마도 되돌린다(sRGB 역감마).
//!
//! ## 기준선
//!
//! 본문 글자는 대비비 **4.5:1** 이상이 권고다(WCAG AA). 큰 글자는 3:1. 터미널 본문은
//! 작은 글자에 가까우므로 4.5를 기준으로 본다. 이 값은 규격에서 온 것이지 우리가 정한
//! 것이 아니다.

/// sRGB 한 성분(0~255)을 선형 값으로 되돌린다.
fn linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    match s <= 0.04045 {
        true => s / 12.92,
        false => ((s + 0.055) / 1.055).powf(2.4),
    }
}

/// 상대 휘도(0.0=검정, 1.0=흰색). 가중치는 사람 눈의 색별 민감도다.
pub fn luminance(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// 두 색의 대비비(1.0~21.0). 순서는 상관없다.
pub fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (la, lb) = (luminance(a.0, a.1, a.2), luminance(b.0, b.1, b.2));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// 본문으로 읽을 만한가(WCAG AA, 4.5:1).
pub fn readable(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> bool {
    contrast_ratio(fg, bg) >= 4.5
}

#[cfg(test)]
mod tests {
    use super::{contrast_ratio, luminance, readable};

    const BLACK: (u8, u8, u8) = (0, 0, 0);
    const WHITE: (u8, u8, u8) = (255, 255, 255);

    /// 규격이 정한 양 끝 — 검정과 흰색은 21:1이다.
    #[test]
    fn black_on_white_is_the_maximum() {
        let r = contrast_ratio(BLACK, WHITE);
        assert!((r - 21.0).abs() < 0.01, "{r}");
    }

    /// 같은 색끼리는 1:1(전혀 안 보인다).
    #[test]
    fn a_colour_against_itself_is_the_minimum() {
        assert!((contrast_ratio(BLACK, BLACK) - 1.0).abs() < 0.001);
        let grey = (0x80, 0x80, 0x80);
        assert!((contrast_ratio(grey, grey) - 1.0).abs() < 0.001);
    }

    /// 순서가 답을 바꾸면 안 된다.
    #[test]
    fn the_order_does_not_matter() {
        let a = (0x12, 0x34, 0x56);
        assert!((contrast_ratio(a, WHITE) - contrast_ratio(WHITE, a)).abs() < 0.0001);
    }

    /// **초록이 파랑보다 밝다.** 단순 평균이었다면 이 시험이 깨진다.
    #[test]
    fn green_reads_brighter_than_blue() {
        assert!(luminance(0, 255, 0) > luminance(0, 0, 255));
        // 사람 눈의 민감도 차이는 크다 — 열 배가 넘는다.
        assert!(luminance(0, 255, 0) > luminance(0, 0, 255) * 8.0);
    }

    /// 우리 기본 테마(흰 글씨·검정 바탕)는 읽을 만해야 한다.
    #[test]
    fn our_default_theme_is_readable() {
        assert!(readable((0xe5, 0xe5, 0xe5), (0x1e, 0x1e, 0x1e)));
    }

    /// 흔한 실수 — 짙은 바탕에 짙은 파랑은 **읽을 수 없다**.
    #[test]
    fn dark_blue_on_dark_grey_is_not_readable() {
        assert!(!readable((0x24, 0x72, 0xc8), (0x1e, 0x1e, 0x1e)));
    }

    /// 기준선 언저리에서 흔들리지 않는다(규격 값 4.5).
    #[test]
    fn the_threshold_is_the_standard_one() {
        // 회색 계단을 올려 가며 4.5를 넘는 지점이 있는지 본다.
        let bg = (0x00, 0x00, 0x00);
        let below = (0x74, 0x74, 0x74); // 약 4.3
        let above = (0x80, 0x80, 0x80); // 약 5.3
        assert!(!readable(below, bg), "{}", contrast_ratio(below, bg));
        assert!(readable(above, bg), "{}", contrast_ratio(above, bg));
    }
}
