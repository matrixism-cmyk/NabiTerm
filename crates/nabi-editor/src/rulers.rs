//! **세로 눈금** — 규약 폭을 넘는 줄이 접지 않고도 보인다.
//!
//! 지난 배치에 "화면에서 접을 폭"을 냈는데, 접고 싶지 않은 사람에게는 답이 안 된다.
//! 눈금은 접지 않고 **선만 긋는다** — VS Code의 `editor.rulers`가 하는 일이다.
//!
//! 계산만 여기 둔다(어디에 선을 그을지). 색과 그리기는 화면 쪽 일이다.

/// 눈금을 그을 열들을 판다. `"80,100"` 같은 글에서 숫자만 뽑는다.
///
/// 잘못된 조각은 버린다 — 하나가 틀렸다고 나머지를 잃으면 안 된다.
pub fn parse_columns(spec: &str) -> Vec<usize> {
    let mut out: Vec<usize> = spec
        .split([',', ' '])
        .filter_map(|t| t.trim().parse::<usize>().ok())
        .filter(|n| *n > 0 && *n <= 500)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// 각 눈금의 x 좌표(글자 폭 기준). 글자 폭을 못 재면 아무것도 그리지 않는다.
pub fn offsets(cols: &[usize], char_w: f32) -> Vec<f32> {
    if char_w.is_nan() || char_w <= 0.0 {
        return Vec::new();
    }
    cols.iter().map(|c| char_w * *c as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_parsed_sorted_and_deduped() {
        assert_eq!(parse_columns("100,80,80"), vec![80, 100]);
        assert_eq!(parse_columns("80 100 120"), vec![80, 100, 120]);
    }

    /// 하나가 틀렸다고 나머지를 잃으면 안 된다.
    #[test]
    fn a_bad_piece_does_not_lose_the_others() {
        assert_eq!(parse_columns("80,쓰레기,120"), vec![80, 120]);
    }

    /// 0열이나 터무니없는 값은 버린다 — 화면 왼쪽 끝에 선이 그어지면 방해만 된다.
    #[test]
    fn nonsense_columns_are_dropped() {
        assert!(parse_columns("0").is_empty());
        assert!(parse_columns("9999").is_empty());
        assert!(parse_columns("").is_empty());
        assert!(parse_columns("   ").is_empty());
    }

    #[test]
    fn offsets_follow_the_character_width() {
        assert_eq!(offsets(&[80, 100], 8.0), vec![640.0, 800.0]);
    }

    /// **글자 폭을 못 재면 아무것도 그리지 않는다** — 0을 곱하면 전부 왼쪽 끝에 겹친다.
    #[test]
    fn an_unmeasurable_font_draws_nothing() {
        assert!(offsets(&[80], 0.0).is_empty());
        assert!(offsets(&[80], f32::NAN).is_empty());
    }

    #[test]
    fn no_columns_means_no_lines() {
        assert!(offsets(&[], 8.0).is_empty());
    }
}
