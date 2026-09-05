//! 상태 표시줄의 **크기 칩**(`172×34`)을 눌러 창을 그 크기에 맞춘다.
//!
//! ## 왜 창을 옮기는가
//!
//! 터미널 격자는 그려진 자리에서 나온다 — 매 프레임 pane 의 폭·높이를 글자 크기로 나눠
//! `Resize` 를 보낸다(`tabsterm.rs`). 그래서 격자만 따로 바꿔 봐야 **다음 프레임에 다시
//! 덮인다.** 손으로 정한 값이 한 프레임 만에 사라지면 눌러도 아무 일 없는 것처럼 보인다.
//!
//! 그래서 격자가 아니라 **창을 바꾼다.** 지금 격자와 지금 pane 크기를 알면 글자 한 칸의
//! 크기를 알 수 있고, 원하는 격자에 필요한 pane 크기가 나온다. 그 차이만큼 창을 늘리거나
//! 줄이면 격자가 따라온다.
//!
//! 창을 채우는 것은 pane 만이 아니라서(메뉴·탭 줄·상태바·사이드바) 차이만 더한다.
//! 절대값으로 계산하면 그 장식들의 높이를 우리가 다 알아야 하는데, 그것은 설정에 따라 변한다.

use nabi_types::GridSize;

/// 자주 쓰는 크기 — 눌러서 바로 맞춘다.
pub(crate) const PRESETS: [(u16, u16); 4] = [(80, 24), (100, 30), (120, 40), (160, 48)];

/// 지금 창 크기에서 목표 격자에 맞는 창 크기를 구한다.
///
/// * `win` — 지금 창 크기(논리 점)
/// * `pane` — 지금 pane 이 차지한 크기
/// * `grid` — 지금 격자(열×행)
/// * `want` — 원하는 격자
///
/// 격자나 pane 이 0이면 나눌 수 없으니 그대로 둔다.
pub(crate) fn window_for(
    win: (f32, f32),
    pane: (f32, f32),
    grid: GridSize,
    want: (u16, u16),
) -> Option<(f32, f32)> {
    let (c, r) = (grid.cols() as f32, grid.rows() as f32);
    if c <= 0.0 || r <= 0.0 || pane.0 <= 0.0 || pane.1 <= 0.0 {
        return None;
    }
    let (cw, ch) = (pane.0 / c, pane.1 / r);
    let dw = cw * want.0 as f32 - pane.0;
    let dh = ch * want.1 as f32 - pane.1;
    // 너무 작게는 못 만든다 — 창이 쓸 수 없게 작아지면 되돌릴 방법이 없다.
    Some(((win.0 + dw).max(480.0), (win.1 + dh).max(320.0)))
}

impl crate::app::NabiApp {
    /// 고른 격자에 맞게 창 크기를 바꾼다.
    ///
    /// 격자를 직접 바꾸지 않는 까닭은 위 모듈 설명에 적었다 — 다음 프레임에 덮인다.
    pub(crate) fn resize_window_for_grid(&mut self, ctx: &egui::Context, want: (u16, u16)) {
        let Some(pane) = self.focused_pane() else { return };
        let Some(grid) = self.last_grid.get(&pane).copied() else { return };
        let Some(rect) = self.pane_rects.get(&pane).copied() else { return };
        let Some((w, h)) = window_for(self.last_win, (rect.width(), rect.height()), grid, want)
        else {
            return;
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
        self.notify = Some((
            format!("{}\u{00d7}{}", want.0, want.1),
            std::time::Instant::now(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 원하는_격자만큼_창을_늘린다() {
        // 100열 30행이 800×600 을 쓰고 있다 = 한 칸 8×20.
        let g = GridSize::new(100, 30);
        let got = window_for((1000.0, 700.0), (800.0, 600.0), g, (120, 40)).unwrap();
        // 20열 더 = 160점 더, 10행 더 = 200점 더.
        assert_eq!(got, (1160.0, 900.0));
    }

    #[test]
    fn 줄일_때도_같은_셈이다() {
        let g = GridSize::new(100, 30);
        let got = window_for((1000.0, 700.0), (800.0, 600.0), g, (80, 24)).unwrap();
        assert_eq!(got, (840.0, 580.0));
    }

    #[test]
    fn 너무_작아지지는_않는다() {
        let g = GridSize::new(200, 100);
        let got = window_for((600.0, 400.0), (500.0, 300.0), g, (1, 1)).unwrap();
        assert_eq!(got, (480.0, 320.0));
    }

    #[test]
    fn 셀_수_없으면_그대로_둔다() {
        assert!(window_for((900.0, 600.0), (0.0, 0.0), GridSize::new(80, 24), (100, 30)).is_none());
        assert!(window_for((900.0, 600.0), (800.0, 600.0), GridSize::new(0, 24), (100, 30)).is_none());
    }
}
