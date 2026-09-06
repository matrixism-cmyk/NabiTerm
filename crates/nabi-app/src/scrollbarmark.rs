//! 스크롤바 위의 **표식** — 막대만 봐도 어디에 무엇이 있었는지 보인다.
//!
//! 긴 로그를 되짚을 때 사람이 찾는 것은 대개 셋 중 하나다. 명령이 시작된 자리, **실패한**
//! 명령, 그리고 자기가 남겨 둔 표식. 지금까지 그 자리들은 스크롤해 봐야만 보였다.
//!
//! Ghostty 1.3 이 네이티브 스크롤바를 넣으면서 같은 길로 갔고, 편집기 쪽에서는 오래된
//! 관습이다(VS Code 의 오른쪽 눈금). 스크롤바는 이미 "전체 중 어디"를 나타내는 자리이므로,
//! 거기에 눈금을 얹는 것이 새 화면을 만드는 것보다 낫다.
//!
//! 계산만 여기 둔다 — 그리는 일은 [`crate::scrollbar`] 가 한다. 숫자를 떼어 두면 눈으로
//! 확인할 수 없는 부분(범위 밖 표식, 겹침, 0으로 나누기)을 시험으로 잡을 수 있다.

/// 절대 줄 번호가 트랙에서 차지하는 위치(0.0=맨 위, 1.0=맨 아래). 범위 밖이면 None.
///
/// `total` 은 스크롤백 + 현재 화면 줄 수다. 썸(thumb) 이 쓰는 것과 **같은 자**여야 한다 —
/// 자가 다르면 표식이 가리키는 자리와 실제로 스크롤되는 자리가 어긋난다.
pub(crate) fn mark_frac(abs: usize, total: usize) -> Option<f32> {
    match total {
        0 => None,
        t if abs >= t => None,
        t => Some(abs as f32 / t as f32),
    }
}

/// 표식들을 트랙의 **픽셀 행**으로 바꾼다(오름차순, 중복 제거).
///
/// 스크롤백이 수만 줄이면 표식 수십 개가 한 픽셀에 겹친다. 겹쳐 그려 봐야 보이지도 않고
/// 프레임만 먹으므로, 같은 행은 한 번만 그린다.
pub(crate) fn marker_rows(lines: &[usize], total: usize, track_h: f32) -> Vec<u32> {
    if track_h < 1.0 {
        return Vec::new();
    }
    let mut out: Vec<u32> = lines
        .iter()
        .filter_map(|&l| mark_frac(l, total))
        .map(|f| (f * track_h) as u32)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 맨 위는 0, 마지막 줄은 1보다 작다 — 트랙 밖으로 나가면 안 된다.
    #[test]
    fn 자리는_0에서_1_사이다() {
        assert_eq!(mark_frac(0, 100), Some(0.0));
        let last = mark_frac(99, 100).unwrap();
        assert!((0.98..1.0).contains(&last), "마지막 줄 {last}");
    }

    /// 범위를 벗어난 표식은 그리지 않는다. 스크롤백이 잘려 나가면 실제로 생긴다.
    #[test]
    fn 범위_밖은_그리지_않는다() {
        assert_eq!(mark_frac(100, 100), None);
        assert_eq!(mark_frac(1, 0), None, "0으로 나누면 안 된다");
        assert_eq!(mark_frac(0, 0), None);
    }

    /// 같은 픽셀 행에 겹친 표식은 한 번만 그린다.
    #[test]
    fn 겹친_표식은_한_번만() {
        // 10000줄을 100px 트랙에 — 100줄이 1px 이다.
        let lines: Vec<usize> = (0..50).collect(); // 전부 첫 픽셀 행.
        assert_eq!(marker_rows(&lines, 10_000, 100.0), vec![0]);
    }

    /// 떨어진 표식은 각각 남는다.
    #[test]
    fn 떨어진_표식은_각각_남는다() {
        let rows = marker_rows(&[0, 5_000, 9_999], 10_000, 100.0);
        assert_eq!(rows, vec![0, 50, 99]);
    }

    /// 트랙이 없다시피 하면 아무것도 그리지 않는다(0 나누기·헛수고 방지).
    #[test]
    fn 트랙이_없으면_아무것도_안_그린다() {
        assert!(marker_rows(&[1, 2, 3], 10, 0.0).is_empty());
        assert!(marker_rows(&[1, 2, 3], 10, 0.9).is_empty());
    }

    /// 범위 밖 표식이 섞여 있어도 나머지는 살아남는다(부분 실패 ≠ 전체 손실).
    #[test]
    fn 섞여_있어도_나머지는_남는다() {
        let rows = marker_rows(&[999_999, 0, 5_000], 10_000, 100.0);
        assert_eq!(rows, vec![0, 50]);
    }
}
