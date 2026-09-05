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

/// 지금 보고 있는 탭에는 이만큼은 준다 — 나머지가 아무리 좁아져도.
///
/// 지금 무엇을 보고 있는지가 가장 자주 읽히는 정보다. 그것까지 여섯 글자로 줄면
/// 탭 줄 전체가 무슨 말인지 알 수 없게 된다(사용자 지적 2026-09-05).
const ACTIVE_MIN_CHARS: usize = 16;

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

/// (지금 보는 탭, 나머지 탭)이 쓸 글자 수.
///
/// ## 왜 나눠 주는가
///
/// 모두에게 같은 몫을 주면, 탭이 많아졌을 때 **지금 보고 있는 탭까지** 여섯 글자가 된다.
/// 그런데 탭 줄에서 가장 자주 읽는 것이 그 탭이다.
///
/// 그래서 보고 있는 탭에게 먼저 넉넉히 주고, **남은 자리를 나머지가 나눈다.** 브라우저가
/// 하는 것과 같다 — 다른 탭은 좁아도 지금 것은 읽을 수 있다.
pub(crate) fn name_budgets(bar_width: f32, tabs: usize) -> (usize, usize) {
    let even = name_budget(bar_width, tabs);
    // 넉넉하면 나눌 이유가 없다.
    if tabs <= 1 || even >= MAX_CHARS {
        return (even, even);
    }
    let active = even.clamp(ACTIVE_MIN_CHARS, MAX_CHARS);
    // 보고 있는 탭이 더 가져간 만큼을 나머지에서 뺀다.
    let extra = (active - even) as f32 * CHAR_W;
    let left = (bar_width - extra).max(0.0);
    let other = name_budget(left, tabs);
    (active, other)
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

    /// **보고 있는 탭은 나머지보다 넓다.** 탭이 많아질수록 그 차이가 뜻이 있다.
    #[test]
    fn 보고_있는_탭에_더_준다() {
        let (a, o) = name_budgets(1200.0, 14);
        assert!(a > o, "보고 있는 탭이 더 넓어야 한다: {a} vs {o}");
        assert!(a >= ACTIVE_MIN_CHARS, "{a}");
        assert!(o >= MIN_CHARS, "{o}");
    }

    /// 자리가 넉넉하면 둘이 같다 — 굳이 나눌 이유가 없다.
    #[test]
    fn 넉넉하면_똑같이_준다() {
        let (a, o) = name_budgets(2000.0, 2);
        assert_eq!((a, o), (MAX_CHARS, MAX_CHARS));
    }

    /// 나머지에서 뺀 만큼이 실제로 좁아져야 한다 — 안 그러면 나눈 시늉만 한 것이다.
    #[test]
    fn 나머지는_실제로_좁아진다() {
        let even = name_budget(1000.0, 12);
        let (_a, o) = name_budgets(1000.0, 12);
        assert!(o <= even, "나머지가 늘어나면 안 된다: {even} → {o}");
    }
}
