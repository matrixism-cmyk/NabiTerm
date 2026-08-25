//! **들여쓰기 안내선** — 중첩 깊이를 눈으로 본다.
//!
//! 세로 눈금(`rulers`)은 **고정된 열**에 긋는다. 들여쓰기 안내선은 **줄마다 다른 자리**에
//! 긋는다 — 그 줄이 몇 단계 안쪽인지 보여 준다. 둘은 같은 "보조선"이지만 하는 일이 다르다.
//!
//! ## 빈 줄에서 선이 끊기면 안 된다
//!
//! 블록 사이의 빈 줄에서 안내선이 사라지면 세로줄이 토막 나 보인다. 그래서 빈 줄은
//! **이웃한 줄의 깊이를 물려받는다** — 위아래 중 더 깊은 쪽이 아니라 **얕은 쪽**을 쓴다
//! (더 깊게 그리면 없는 블록을 있는 것처럼 보이게 한다).

/// 한 줄의 들여쓰기 깊이(탭 폭 기준 칸 수). 빈 줄은 None.
pub fn depth_of(line: &str, tab: usize) -> Option<usize> {
    if line.trim().is_empty() {
        return None;
    }
    let mut col = 0usize;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += tab - (col % tab),
            _ => break,
        }
    }
    Some(col / tab.max(1))
}

/// 각 줄에 그릴 안내선 개수. 빈 줄은 이웃에서 물려받는다.
pub fn depths(lines: &[&str], tab: usize) -> Vec<usize> {
    let raw: Vec<Option<usize>> = lines.iter().map(|l| depth_of(l, tab)).collect();
    let mut out = vec![0usize; raw.len()];
    for i in 0..raw.len() {
        if let Some(d) = raw[i] {
            out[i] = d;
            continue;
        }
        // 위아래에서 가장 가까운 글 있는 줄을 찾아 **얕은 쪽**을 쓴다.
        let up = raw[..i].iter().rev().flatten().next().copied();
        let down = raw[i + 1..].iter().flatten().next().copied();
        out[i] = match (up, down) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => 0,
        };
    }
    out
}

/// 그 줄에 그을 x 좌표들. 0단계(맨 왼쪽)에는 긋지 않는다 — 글자와 겹친다.
pub fn offsets(depth: usize, tab: usize, char_w: f32) -> Vec<f32> {
    if char_w.is_nan() || char_w <= 0.0 || depth == 0 {
        return Vec::new();
    }
    (1..=depth).map(|d| char_w * (d * tab) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_and_tabs_both_count() {
        assert_eq!(depth_of("    x", 4), Some(1));
        assert_eq!(depth_of("\tx", 4), Some(1));
        assert_eq!(depth_of("        x", 4), Some(2));
        assert_eq!(depth_of("x", 4), Some(0));
    }

    #[test]
    fn a_blank_line_has_no_depth_of_its_own() {
        assert_eq!(depth_of("", 4), None);
        assert_eq!(depth_of("   ", 4), None);
    }

    /// **빈 줄에서 선이 끊기면 세로줄이 토막 나 보인다** — 이웃에서 물려받는다.
    #[test]
    fn a_blank_line_inherits_from_its_neighbours() {
        let lines = ["    a", "", "    b"];
        assert_eq!(depths(&lines, 4), vec![1, 1, 1]);
    }

    /// **얕은 쪽을 쓴다** — 깊게 그리면 없는 블록을 있는 것처럼 보이게 한다.
    #[test]
    fn a_blank_line_between_depths_takes_the_shallower() {
        let lines = ["        a", "", "    b"];
        assert_eq!(depths(&lines, 4), vec![2, 1, 1]);
    }

    #[test]
    fn leading_and_trailing_blanks_are_handled() {
        assert_eq!(depths(&["", "    a", ""], 4), vec![1, 1, 1]);
        assert_eq!(depths(&["", ""], 4), vec![0, 0]);
    }

    /// 맨 왼쪽에는 긋지 않는다 — 글자와 겹친다.
    #[test]
    fn depth_zero_draws_nothing() {
        assert!(offsets(0, 4, 8.0).is_empty());
    }

    #[test]
    fn each_level_gets_one_line() {
        assert_eq!(offsets(2, 4, 8.0), vec![32.0, 64.0]);
    }

    /// 글자 폭을 못 재면 아무것도 그리지 않는다(눈금과 같은 규칙).
    #[test]
    fn an_unmeasurable_font_draws_nothing() {
        assert!(offsets(3, 4, 0.0).is_empty());
        assert!(offsets(3, 4, f32::NAN).is_empty());
    }
}
