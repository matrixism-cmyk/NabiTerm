//! **줄바꿈 폭** — 창 폭에 맞출 것인가, 정해진 열에서 접을 것인가.
//!
//! 지금까지 줄바꿈은 켜기/끄기뿐이었다. 그런데 코드를 볼 때 필요한 것은 대개 "창 폭"이
//! 아니라 **규약 폭**이다 — 80·100·120열. 창을 넓혀 놓고도 그 폭에서 접어 보면 어느 줄이
//! 규약을 넘는지 눈으로 보인다.
//!
//! 계산만 여기 둔다. 픽셀은 글꼴이 정하고, 그건 화면 쪽 일이다.

/// 줄바꿈에 쓸 폭(픽셀). `f32::INFINITY`면 접지 않는다(가로 스크롤).
///
/// * `wrap`이 꺼져 있으면 접지 않는다.
/// * `col`이 0이면 창 폭에 맞춘다(기존 동작).
/// * `col`이 0보다 크면 그 열에서 접는다.
pub fn wrap_width(wrap: bool, col: usize, char_w: f32, viewport_w: f32) -> f32 {
    if !wrap {
        return f32::INFINITY;
    }
    if col == 0 {
        return viewport_w;
    }
    // 글자 폭을 못 재면(글꼴이 아직 없음) 창 폭으로 물러선다 — 0을 곱해 한 글자도 못 쓰는
    // 폭이 되면 화면이 세로 한 줄이 된다.
    if char_w.is_nan() || char_w <= 0.0 {
        return viewport_w;
    }
    char_w * col as f32
}

/// 고를 수 있는 폭들. 0은 "창 폭에 맞춤".
pub const CHOICES: &[usize] = &[0, 72, 80, 100, 120];

/// 화면에 적을 이름.
pub fn label(col: usize) -> String {
    match col {
        0 => "\u{2194}".to_string(), // 창 폭에 맞춤.
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_off_means_no_wrapping() {
        assert_eq!(wrap_width(false, 80, 8.0, 500.0), f32::INFINITY);
        assert_eq!(wrap_width(false, 0, 8.0, 500.0), f32::INFINITY);
    }

    #[test]
    fn zero_column_follows_the_window() {
        assert_eq!(wrap_width(true, 0, 8.0, 500.0), 500.0);
    }

    #[test]
    fn a_column_becomes_that_many_characters_wide() {
        assert_eq!(wrap_width(true, 80, 8.0, 500.0), 640.0);
        assert_eq!(wrap_width(true, 120, 7.5, 500.0), 900.0);
    }

    /// **글자 폭을 못 재면 창 폭으로 물러선다** — 0을 곱하면 화면이 세로 한 줄이 된다.
    #[test]
    fn an_unmeasurable_font_falls_back_to_the_window() {
        assert_eq!(wrap_width(true, 80, 0.0, 500.0), 500.0);
        assert_eq!(wrap_width(true, 80, f32::NAN, 500.0), 500.0);
        assert_eq!(wrap_width(true, 80, -1.0, 500.0), 500.0);
    }

    /// 고를 수 있는 값에 "창 폭에 맞춤"이 들어 있어야 예전 동작으로 돌아갈 수 있다.
    #[test]
    fn the_choices_include_following_the_window() {
        assert!(CHOICES.contains(&0));
        assert!(CHOICES.len() >= 3);
    }

    #[test]
    fn labels_are_readable() {
        assert_eq!(label(80), "80");
        assert_ne!(label(0), "0", "0을 그대로 보이면 '0열에서 접기'로 읽힌다");
    }
}
