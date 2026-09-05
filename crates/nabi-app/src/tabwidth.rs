//! 탭이 늘어나면 **탭 폭을 줄인다** — 크롬이 하는 것과 같다.
//!
//! ## 왜
//!
//! 탭 줄이 꽉 차면 새 탭은 화면 밖에 생겼다. 끝까지 굴려서 보이게는 했지만, 그러면
//! 앞쪽 탭이 대신 밀려 나간다. 굴리는 것으로는 "한 번에 다 보이지 않는다"는 사실 자체가
//! 바뀌지 않는다(사용자 지적 2026-09-05).
//!
//! 브라우저는 이 문제를 폭으로 푼다. 탭이 늘면 탭이 좁아지고, 좁아진 만큼 이름을 줄인다.
//!
//! ## 어떻게 정하나
//!
//! 탭 폭은 egui_dock 이 **제목 길이로** 정한다. 그래서 우리가 만질 수 있는 것은 제목이다.
//! 탭 줄에 쓸 수 있는 폭을 탭 수로 나눠 한 탭 몫을 구하고, 그 몫을 글자 수로 바꾼다.
//!
//! 글자 폭을 정확히 재지 않는다 — 한글은 두 칸, 영문은 한 칸이라 글꼴마다 다르고, 매
//! 프레임 재면 경계에서 이름이 떨린다. 대신 넉넉히 잡은 어림값 하나를 쓴다.

/// 한 글자에 잡아 두는 폭(점). 한글과 영문 사이 어딘가로 넉넉히 잡는다.
const CHAR_W: f32 = 9.0;

/// 이름 말고 탭이 늘 쓰는 폭 — 닫기 단추·안쪽 여백·배지 자리.
const CHROME_W: f32 = 46.0;

/// 아무리 좁아도 이만큼은 보여 준다. 이보다 짧으면 어느 탭인지 알 수 없다.
const MIN_CHARS: usize = 6;

/// 아무리 넓어도 이보다 길게는 안 쓴다. 탭 하나가 창을 가로지르면 그것도 못 쓴다.
const MAX_CHARS: usize = 28;

/// 탭 줄 폭과 탭 수로 **탭 이름에 쓸 글자 수**를 정한다.
///
/// 탭이 적으면 `MAX_CHARS`, 많아지면 줄어들다 `MIN_CHARS` 에서 멈춘다. 거기서 더 늘면
/// 이제 굴려야 하는데, 그때는 상태바의 탭 목록으로 고르면 된다(`tabreveal`).
pub(crate) fn name_budget(bar_width: f32, tabs: usize) -> usize {
    if tabs == 0 {
        return MAX_CHARS;
    }
    let per_tab = bar_width / tabs as f32 - CHROME_W;
    let chars = (per_tab / CHAR_W).floor();
    if !chars.is_finite() || chars < MIN_CHARS as f32 {
        return MIN_CHARS;
    }
    (chars as usize).min(MAX_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 탭이_적으면_넉넉하다() {
        assert_eq!(name_budget(1200.0, 2), MAX_CHARS);
        assert_eq!(name_budget(1200.0, 3), MAX_CHARS);
    }

    #[test]
    fn 탭이_늘면_줄어든다() {
        let few = name_budget(1200.0, 6);
        let many = name_budget(1200.0, 12);
        assert!(few > many, "탭이 늘면 이름이 짧아져야 한다: {few} vs {many}");
        assert!(many >= MIN_CHARS);
    }

    #[test]
    fn 아무리_좁아도_바닥은_있다() {
        assert_eq!(name_budget(200.0, 40), MIN_CHARS);
        assert_eq!(name_budget(0.0, 5), MIN_CHARS);
    }

    #[test]
    fn 탭이_없으면_최대로() {
        assert_eq!(name_budget(1200.0, 0), MAX_CHARS);
    }

    /// 창을 넓히면 같은 탭 수에서 이름이 길어져야 한다.
    #[test]
    fn 넓은_창이_더_길게_보여_준다() {
        assert!(name_budget(1600.0, 10) > name_budget(900.0, 10));
    }
}
