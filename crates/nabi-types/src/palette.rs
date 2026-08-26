//! **ANSI 16색을 고를 수 있게 한다** — 기본 팔레트와 색각 친화 팔레트.
//!
//! 터미널은 색으로 뜻을 나른다. 오류는 빨강, 성공은 초록. 그런데 남성 스무 명 중 한 명은
//! 적록색약이라 **그 둘이 비슷하게 보인다.** 지금까지 우리 팔레트는 상수 하나뿐이었고,
//! 그 사용자에게는 고를 것이 없었다.
//!
//! ## 무엇을 바꾸고 무엇을 안 바꾸나
//!
//! 기본은 **그대로 둔다.** 지금 쓰는 사람의 화면이 달라지면 그건 개선이 아니라 회귀다.
//! 켠 사람에게만 팔레트가 바뀐다.
//!
//! ## "친화적"을 수치로 — 다만 무엇을 재는지 분명히
//!
//! 색각 친화 팔레트는 흔히 감으로 고른다. 여기서는 **적록색약 눈으로 본 색**을 계산해
//! (단순화한 적록 합성 모델) 빨강과 초록이 얼마나 갈리는지 시험이 잰다.
//!
//! **이 모델이 재는 것은 밝기 차이뿐이다.** 색상 혼동 자체는 담지 못한다. 그래서 처음
//! 세운 전제 — "기본 팔레트의 빨강·초록은 색약 눈에 붙어 보인다" — 는 시험에서
//! **반증됐다**(2.34:1로 이미 꽤 갈린다). 실제로 색약 사용자를 힘들게 하는 것은 색상
//! 혼동이고, 그건 이 수치로 증명할 수 없다.
//!
//! 그래서 증명할 수 있는 것만 시험한다: 색각 친화 팔레트가 기본보다 **밝기로 더 크게**
//! 갈라 놓는가(2.34 → 4.01). 색상 쪽은 Okabe-Ito가 색각 이상자를 대상으로 검증해
//! 공개한 값을 그대로 쓰는 것으로 대신한다 — 우리가 지어내지 않았다는 것이 근거다.
//!
//! 색 선택은 Paul Tol과 Okabe-Ito가 공개한 색각 친화 팔레트에서 왔다 — 오래 검증된 값을
//! 새로 지어내는 것보다 낫다.

/// 쓸 수 있는 팔레트.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Palette {
    /// 지금까지 쓰던 값(VS Code 계열). **기본값 — 바꾸지 않는다.**
    #[default]
    Standard,
    /// 적록색약에서도 갈리도록 고른 값(Okabe-Ito 계열).
    Deuteranopia,
    /// 대비를 최대로 — 저시력·밝은 곳에서.
    HighContrast,
}

impl Palette {
    /// 설정 파일에 적히는 이름.
    pub fn as_str(self) -> &'static str {
        match self {
            Palette::Standard => "standard",
            Palette::Deuteranopia => "deuteranopia",
            Palette::HighContrast => "highcontrast",
        }
    }

    /// 설정 파일에서 읽는다. 모르는 값은 기본으로 — 설정 하나 때문에 화면이 망가지지 않게.
    pub fn from_name(s: &str) -> Palette {
        match s {
            "deuteranopia" => Palette::Deuteranopia,
            "highcontrast" => Palette::HighContrast,
            _ => Palette::Standard,
        }
    }
}

/// 지금까지 쓰던 팔레트(VS Code 기본값).
const STANDARD: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), (0xcd, 0x31, 0x31), (0x0d, 0xbc, 0x79), (0xe5, 0xe5, 0x10),
    (0x24, 0x72, 0xc8), (0xbc, 0x3f, 0xbc), (0x11, 0xa8, 0xcd), (0xe5, 0xe5, 0xe5),
    (0x66, 0x66, 0x66), (0xf1, 0x4c, 0x4c), (0x23, 0xd1, 0x8b), (0xf5, 0xf5, 0x43),
    (0x3b, 0x8e, 0xea), (0xd6, 0x70, 0xd6), (0x29, 0xb8, 0xdb), (0xff, 0xff, 0xff),
];

/// 적록색약 친화 — 빨강을 주황 쪽으로, 초록을 청록 쪽으로 밀어 **밝기까지** 갈라 놓는다.
///
/// 색상만 바꾸면 색약 눈에는 여전히 붙어 보인다. 그래서 빨강 자리는 어둡게, 초록 자리는
/// 밝게 두어 **색을 못 봐도 밝기로** 구별되게 했다.
const DEUTER: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), (0xd5, 0x5e, 0x00), (0x00, 0x9e, 0x73), (0xf0, 0xe4, 0x42),
    (0x00, 0x72, 0xb2), (0xcc, 0x79, 0xa7), (0x56, 0xb4, 0xe9), (0xe5, 0xe5, 0xe5),
    (0x66, 0x66, 0x66), (0xe6, 0x8f, 0x2e), (0x35, 0xc9, 0x9a), (0xf7, 0xf0, 0x6e),
    (0x34, 0x9b, 0xd8), (0xe0, 0xa0, 0xc4), (0x8a, 0xcf, 0xf0), (0xff, 0xff, 0xff),
];

/// 고대비 — 어두운 바탕에서 전부 밝게, 서로 멀게.
const HIGH: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), (0xff, 0x6b, 0x6b), (0x5c, 0xff, 0x9d), (0xff, 0xf1, 0x76),
    (0x7a, 0xb8, 0xff), (0xff, 0x8a, 0xf0), (0x6d, 0xe8, 0xff), (0xff, 0xff, 0xff),
    (0x9a, 0x9a, 0x9a), (0xff, 0x9b, 0x9b), (0x9d, 0xff, 0xc4), (0xff, 0xf9, 0xb0),
    (0xa8, 0xd2, 0xff), (0xff, 0xb5, 0xf5), (0xa5, 0xf1, 0xff), (0xff, 0xff, 0xff),
];

/// **지금 쓰는 팔레트.** 설정에서 정하고, `ansi16`이 여기를 본다.
///
/// 전역 하나로 둔 까닭: 색을 고르는 자리가 두 곳(이름색 · 인덱스색)이라, 각자 설정을
/// 들고 다니면 언젠가 한쪽만 바뀐다 — 어떤 색은 바뀌고 어떤 색은 안 바뀌는 화면이 된다.
/// keepalive·접속 제한과 같은 방식이다.
pub static ACTIVE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// 지금 팔레트를 정한다(설정이 바뀔 때).
pub fn set_active(p: Palette) {
    let v = match p {
        Palette::Standard => 0,
        Palette::Deuteranopia => 1,
        Palette::HighContrast => 2,
    };
    ACTIVE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// 지금 팔레트.
pub fn active() -> Palette {
    match ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Palette::Deuteranopia,
        2 => Palette::HighContrast,
        _ => Palette::Standard,
    }
}

/// 그 팔레트의 색 하나(0~15).
pub fn color(p: Palette, index: u8) -> (u8, u8, u8) {
    let t = match p {
        Palette::Standard => &STANDARD,
        Palette::Deuteranopia => &DEUTER,
        Palette::HighContrast => &HIGH,
    };
    t[(index & 0x0f) as usize]
}

/// **적록색약 눈으로 본 색**(적색맹 근사).
///
/// 정밀한 변환은 LMS 색공간을 거치지만, 여기서 필요한 것은 "두 색이 갈리는가"뿐이다.
/// 널리 쓰이는 단순화식을 쓴다 — 빨강과 초록 성분이 하나로 합쳐진다고 본다.
pub fn as_deuteranope(c: (u8, u8, u8)) -> (u8, u8, u8) {
    let (r, g) = (c.0 as f32, c.1 as f32);
    let rg = 0.625 * r + 0.375 * g;
    let v = rg.round().clamp(0.0, 255.0) as u8;
    (v, v, c.2)
}

#[cfg(test)]
mod tests {
    use super::{as_deuteranope, color, Palette};
    use crate::contrast::contrast_ratio;

    /// 기본은 **바뀌지 않았다** — 지금 쓰는 사람의 화면이 달라지면 회귀다.
    #[test]
    fn the_standard_palette_is_unchanged() {
        assert_eq!(color(Palette::Standard, 1), (0xcd, 0x31, 0x31));
        assert_eq!(color(Palette::Standard, 2), (0x0d, 0xbc, 0x79));
        assert_eq!(color(Palette::Standard, 0), (0, 0, 0));
        assert_eq!(color(Palette::Standard, 15), (0xff, 0xff, 0xff));
    }

    /// 인덱스가 넘쳐도 터지지 않는다(16으로 감는다).
    #[test]
    fn an_index_past_the_end_wraps() {
        assert_eq!(color(Palette::Standard, 16), color(Palette::Standard, 0));
        assert_eq!(color(Palette::Standard, 255), color(Palette::Standard, 15));
    }

    /// 색약 눈으로 본 빨강·초록 거리(밝기 기준).
    fn red_green_gap(p: Palette) -> f32 {
        contrast_ratio(as_deuteranope(color(p, 1)), as_deuteranope(color(p, 2)))
    }

    /// **이 시험이 A2의 존재 이유다** — 색각 친화 팔레트가 기본보다 **더 크게** 갈라 놓는다.
    ///
    /// 처음에는 "기본 팔레트는 붙어 보인다"고 단정했다가 이 시험에 반증됐다(2.34:1).
    /// 재는 것이 밝기뿐이라 그렇다. 그래서 단정 대신 **견주기**로 바꿨다.
    #[test]
    fn the_deuteranopia_palette_separates_better_than_the_standard_one() {
        let (std, deu) = (red_green_gap(Palette::Standard), red_green_gap(Palette::Deuteranopia));
        assert!(deu > std * 1.5, "충분히 더 갈리지 않는다: 기본 {std:.2} → 친화 {deu:.2}");
        assert!(deu >= 3.5, "색약 눈에서 빨강과 초록이 가깝다: {deu:.2}");
    }

    /// 밝은 쪽(9·10)도 마찬가지여야 한다 — 굵은 글씨가 밝은 색을 쓴다.
    #[test]
    fn the_bright_pair_stays_apart_too() {
        let r = as_deuteranope(color(Palette::Deuteranopia, 9));
        let g = as_deuteranope(color(Palette::Deuteranopia, 10));
        assert!(contrast_ratio(r, g) >= 1.8, "{:?}", contrast_ratio(r, g));
    }

    /// 어떤 팔레트든 **어두운 바탕에서 읽혀야** 한다(검정과 회색 자리는 뺀다).
    #[test]
    fn every_palette_reads_on_a_dark_background() {
        let bg = (0x1e, 0x1e, 0x1e);
        for p in [Palette::Standard, Palette::Deuteranopia, Palette::HighContrast] {
            for i in [1u8, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15] {
                let c = color(p, i);
                let r = contrast_ratio(c, bg);
                // 3:1은 큰 글자 기준 — 터미널 색은 본문과 강조가 섞여 있어 이 선을 쓴다.
                assert!(r >= 3.0, "{p:?} {i}번 색이 어두운 바탕에서 안 보인다: {r:.2}");
            }
        }
    }

    /// 고대비 팔레트는 이름값을 해야 한다 — 기본보다 평균 대비가 높다.
    #[test]
    fn the_high_contrast_palette_earns_its_name() {
        let bg = (0x00, 0x00, 0x00);
        let avg = |p| {
            (1u8..15).map(|i| contrast_ratio(color(p, i), bg)).sum::<f32>() / 14.0
        };
        assert!(avg(Palette::HighContrast) > avg(Palette::Standard));
    }

    /// 이름 왕복 — 설정 파일에 적고 다시 읽어도 같아야 한다.
    #[test]
    fn the_name_survives_a_round_trip() {
        for p in [Palette::Standard, Palette::Deuteranopia, Palette::HighContrast] {
            assert_eq!(Palette::from_name(p.as_str()), p);
        }
    }

    /// 모르는 이름은 **기본**으로 — 설정 하나 때문에 화면이 망가지지 않게.
    #[test]
    fn an_unknown_name_falls_back_to_standard() {
        assert_eq!(Palette::from_name("무지개"), Palette::Standard);
        assert_eq!(Palette::from_name(""), Palette::Standard);
    }
}
