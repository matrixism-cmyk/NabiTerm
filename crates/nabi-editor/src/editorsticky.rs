//! **지금 어디 안에 있는지** 화면 맨 위에 붙여 둔다(고정 스크롤).
//!
//! ## 무엇을 푸는가
//!
//! 긴 파일에서 아래로 내려가면 함수 이름이 화면 밖으로 나간다. 그러면 지금 보는 코드가
//! 어느 함수의 것인지 알려고 위로 올라갔다가 다시 내려와야 한다. 스크롤을 두 번 하는 동안
//! 하던 생각이 끊긴다.
//!
//! VS Code·Visual Studio·JetBrains 가 이것을 "sticky scroll" 로 부른다(2026 조사).
//! 감싸고 있는 이름들을 맨 위 몇 줄에 고정해 두는 것이다.
//!
//! ## 왜 새로 파싱하지 않는가
//!
//! 아웃라인(`editoroutline`)이 이미 줄마다 `(줄 번호, 이름, 깊이)` 를 만든다. 고정 줄은
//! 그중 **화면 위쪽에 있는 것들 가운데 깊이가 계속 얕아지는 사슬**이다. 같은 자료를 두 번
//! 파싱하면 두 곳이 서로 어긋나고, 어긋나면 고정된 이름이 거짓말을 한다.
//!
//! ## 어디까지 맞는가 (한계를 적어 둔다)
//!
//! 우리가 가진 것은 줄 번호와 깊이뿐이고 **범위의 끝은 모른다.** 마크다운 제목은 다음
//! 같은/상위 제목까지가 범위라 이 방식이 정확하다. 코드는 들여쓰기 깊이로 어림하므로,
//! 함수가 끝나고 같은 깊이의 다른 문장이 이어지면 **끝난 함수의 이름이 잠깐 남을 수 있다.**
//! 정확히 하려면 구문 범위가 필요한데(트리시터), 그건 아웃라인부터 함께 바꿔야 한다.

use crate::editoroutline::OutlineItem;

/// 맨 위에 붙일 수 있는 줄 수. 넘치면 화면이 좁아져 본문이 밀린다.
///
/// VS Code 기본값은 5인데, 우리 아웃라인은 깊이가 얕아(마크다운 6단계·코드 들여쓰기)
/// 셋이면 거의 다 담긴다. 넘는 경우에는 **안쪽 셋**을 남긴다 — 지금 있는 자리에 가까운
/// 것이 더 알고 싶은 것이다.
pub const MAX_STICKY: usize = 3;

/// 화면 맨 위 줄(`first_line`)을 감싸고 있는 항목들 — **바깥부터 안쪽 순**.
///
/// 화면에 이미 보이는 줄은 넣지 않는다(`line < first_line`). 넣으면 같은 줄이 위아래로
/// 두 번 보인다.
pub fn chain(items: &[OutlineItem], first_line: usize) -> Vec<OutlineItem> {
    let mut out: Vec<OutlineItem> = Vec::new();
    // 위쪽 항목을 뒤에서부터 훑으며 **깊이가 계속 얕아지는 것만** 모은다.
    // 그것이 곧 "나를 감싸고 있는 것들"이다.
    let mut need = u8::MAX;
    for it in items.iter().rev().filter(|i| i.line < first_line) {
        if it.depth < need {
            need = it.depth;
            out.push(it.clone());
            if it.depth == 0 {
                break; // 최상위까지 왔다 — 더 바깥은 없다.
            }
        }
    }
    out.reverse(); // 바깥부터 보이게.
    if out.len() > MAX_STICKY {
        out.drain(..out.len() - MAX_STICKY); // 넘치면 안쪽을 남긴다.
    }
    out
}

/// 스크롤 위치(픽셀)와 줄 높이로 화면 맨 위 줄 번호를 구한다.
///
/// 줄 높이가 0이거나 이상하면 0을 돌려준다 — 0으로 나누면 무한대가 되고, 그것을 줄 번호로
/// 쓰면 색인이 터진다(2026-08 `painter.rs` 인덱스 패닉과 같은 결).
pub fn first_visible_line(scroll_y: f32, row_h: f32) -> usize {
    if !row_h.is_finite() || row_h <= 0.0 || !scroll_y.is_finite() || scroll_y <= 0.0 {
        return 0;
    }
    (scroll_y / row_h).floor().max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(line: usize, depth: u8, label: &str) -> OutlineItem {
        OutlineItem { line, depth, label: label.into() }
    }

    /// 마크다운처럼 깔끔히 중첩된 경우 — 바깥부터 안쪽 순으로 나와야 한다.
    #[test]
    fn it_reports_the_enclosing_chain_outermost_first() {
        let items = vec![item(0, 0, "문서"), item(5, 1, "장"), item(9, 2, "절")];
        let got: Vec<String> = chain(&items, 20).into_iter().map(|i| i.label).collect();
        assert_eq!(got, ["문서", "장", "절"]);
    }

    /// 형제는 감싸는 것이 아니다 — 같은 깊이의 앞 항목은 빼야 한다.
    #[test]
    fn a_sibling_is_not_an_ancestor() {
        let items = vec![item(0, 0, "장1"), item(3, 1, "절1"), item(7, 1, "절2")];
        let got: Vec<String> = chain(&items, 10).into_iter().map(|i| i.label).collect();
        assert_eq!(got, ["장1", "절2"], "절1 은 절2 를 감싸지 않는다");
    }

    /// 이미 보이는 줄은 붙이지 않는다 — 붙이면 같은 줄이 두 번 보인다.
    #[test]
    fn a_line_already_on_screen_is_not_pinned() {
        let items = vec![item(0, 0, "장"), item(10, 1, "절")];
        let got: Vec<String> = chain(&items, 10).into_iter().map(|i| i.label).collect();
        assert_eq!(got, ["장"], "10번 줄은 화면 맨 위라 이미 보인다");
    }

    /// 맨 위에서는 붙일 것이 없다.
    #[test]
    fn nothing_is_pinned_at_the_very_top() {
        let items = vec![item(0, 0, "장")];
        assert!(chain(&items, 0).is_empty());
        assert!(chain(&[], 100).is_empty());
    }

    /// 너무 깊으면 **안쪽**을 남긴다 — 지금 자리에 가까운 것이 더 알고 싶은 것이다.
    #[test]
    fn when_it_is_too_deep_the_inner_ones_win() {
        let items: Vec<OutlineItem> =
            (0..6).map(|d| item(d, d as u8, &format!("d{d}"))).collect();
        let got: Vec<String> = chain(&items, 50).into_iter().map(|i| i.label).collect();
        assert_eq!(got, ["d3", "d4", "d5"], "바깥 셋이 아니라 안쪽 셋이다");
    }

    #[test]
    fn a_broken_row_height_does_not_blow_up() {
        assert_eq!(first_visible_line(100.0, 0.0), 0);
        assert_eq!(first_visible_line(f32::NAN, 10.0), 0);
        assert_eq!(first_visible_line(100.0, f32::NAN), 0);
        assert_eq!(first_visible_line(-5.0, 10.0), 0);
        assert_eq!(first_visible_line(105.0, 10.0), 10);
    }
}
