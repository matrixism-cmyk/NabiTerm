//! 글리프 갤리 캐시 — 셀마다 `Painter::text()`를 부르는 비용을 없앤다.
//!
//! `Painter::text()`는 호출마다 문자열 힙 할당 + 레이아웃 잡 할당 + **egui Context 쓰기 잠금 2회**
//! + 잡 전체 해시를 한다. 셀당 호출하면 200×55 화면에서 프레임당 수천 번이 된다.
//!
//! 대신 글리프 문자열별로 갤리를 한 번만 만들어 재사용한다. 색은
//! [`egui::Fonts::layout_delayed_color`]로 `PLACEHOLDER`로 두고 그릴 때 치환하므로,
//! **같은 글자는 색이 달라도 갤리 하나**를 공유한다.
//!
//! 셀 x 좌표는 호출측이 직접 계산한다 — 레이아웃 엔진이 런 전체의 advance를 누적하게 두면
//! 글자별 픽셀 반올림이 쌓여 그리드가 어긋난다(v0.1.33 밑줄 어긋남 회귀의 원인).

use egui::{Context, FontId, Galley};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

thread_local! {
    /// 페인트는 UI 스레드에서만 일어나므로 thread_local로 충분하다(잠금 없음).
    static CACHE: RefCell<GlyphCache> = RefCell::new(GlyphCache::default());
}

#[derive(Default)]
struct GlyphCache {
    map: HashMap<(String, u32), Arc<Galley>>,
    ppp: f32,
    /// 폰트 아틀라스 이미지 크기. 0.34의 FontsView는 아틀라스 Arc를 노출하지 않는다 —
    /// 아틀라스는 가득 차면 더 큰 크기로 재생성되므로(기존 갤리 UV 무효) 크기 변화가
    /// 재생성의 값싼 신호다(ppp 변화와 조합).
    atlas_size: [usize; 2],
}

impl GlyphCache {
    /// 배율이나 아틀라스가 바뀌었으면 캐시를 통째로 버린다.
    fn invalidate_if_stale(&mut self, ppp: f32, atlas_size: [usize; 2]) {
        if self.ppp != ppp || self.atlas_size != atlas_size {
            self.map.clear();
            self.ppp = ppp;
            self.atlas_size = atlas_size;
        }
    }
}

/// 셀 문자열의 갤리를 얻는다(색 무관 — 그릴 때 치환된다).
///
/// 반환된 갤리는 `Painter::galley(pos, galley, fg)` 또는 `Shape::galley(pos, galley, fg)`로
/// 그린다. 색이 `PLACEHOLDER`라 `fg`가 그대로 적용된다.
pub fn galley(ctx: &Context, text: &str, font: &FontId) -> Arc<Galley> {
    let ppp = ctx.pixels_per_point();
    let atlas_size = ctx.fonts(|f| f.font_image_size());
    // 폰트 크기가 키에 들어가야 줌 시 다른 갤리를 쓴다.
    let key = (text.to_owned(), font.size.to_bits());
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.invalidate_if_stale(ppp, atlas_size);
        if let Some(g) = c.map.get(&key) {
            return g.clone();
        }
        let g = ctx.fonts_mut(|f| f.layout_delayed_color(text.to_owned(), font.clone(), f32::INFINITY));
        c.map.insert(key, g.clone());
        g
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn key_separates_by_size() {
        // 같은 글자라도 폰트 크기가 다르면 다른 키여야 한다(줌 시 흐릿해지지 않게).
        let a = ("A".to_owned(), 13.0_f32.to_bits());
        let b = ("A".to_owned(), 26.0_f32.to_bits());
        assert_ne!(a, b);
    }
}
