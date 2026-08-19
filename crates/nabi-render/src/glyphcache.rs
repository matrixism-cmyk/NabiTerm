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
//!
//! ## 캐시 히트 경로의 비용(성능 리뷰 2026-08-19)
//!
//! 초판은 셀마다 ① 조회 키로 `String`을 새로 할당하고 ② 배율·아틀라스 검사로 Context
//! 읽기/쓰기 잠금을 잡았다 — **히트에도** 프레임당 수천 번. 지금은
//! **폰트 크기 → (글자 → 갤리)** 2단 맵이라 히트 시 `&str`로 조회해 할당이 없고,
//! 무효화 검사는 프레임 시작([`begin_frame`])과 **미스 직후**에만 한다. 아틀라스는
//! `layout_delayed_color`(미스)에서만 커지므로 이 두 지점이면 충분하다.

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
    /// 폰트 크기 비트 → (셀 문자열 → 갤리). 크기를 바깥 키로 두어 pane별 글꼴 크기가
    /// 섞여도 서로를 밀어내지 않고, 안쪽은 `&str` 조회라 히트에 할당이 없다.
    map: HashMap<u32, HashMap<String, Arc<Galley>>>,
    ppp: f32,
    /// 폰트 아틀라스 이미지 크기. FontsView는 아틀라스 Arc를 노출하지 않는다 —
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

/// 배율·아틀라스 상태를 확인한다(무효면 캐시 비움). 프레임의 페인트 시작에서 1회 호출.
fn revalidate(ctx: &Context) {
    let ppp = ctx.pixels_per_point();
    let atlas_size = ctx.fonts(|f| f.font_image_size());
    CACHE.with(|c| c.borrow_mut().invalidate_if_stale(ppp, atlas_size));
}

/// 프레임 시작 훅 — 페인트 전에 한 번 부른다(셀마다 검사하지 않기 위한 대체 지점).
pub fn begin_frame(ctx: &Context) {
    revalidate(ctx);
}

/// 셀 문자열의 갤리를 얻는다(색 무관 — 그릴 때 치환된다).
///
/// 반환된 갤리는 `Painter::galley(pos, galley, fg)` 또는 `Shape::galley(pos, galley, fg)`로
/// 그린다. 색이 `PLACEHOLDER`라 `fg`가 그대로 적용된다.
pub fn galley(ctx: &Context, text: &str, font: &FontId) -> Arc<Galley> {
    let size = font.size.to_bits();
    // 히트: 키 할당·Context 잠금 없이 &str로 조회한다(프레임당 수천 번 도는 경로).
    if let Some(g) = CACHE.with(|c| c.borrow().map.get(&size).and_then(|m| m.get(text)).cloned()) {
        return g;
    }
    // 미스: 여기서만 아틀라스가 커질 수 있으므로, 레이아웃 후 상태를 다시 확인하고 넣는다.
    let g = ctx.fonts_mut(|f| f.layout_delayed_color(text.to_owned(), font.clone(), f32::INFINITY));
    revalidate(ctx);
    CACHE.with(|c| {
        c.borrow_mut().map.entry(size).or_default().insert(text.to_owned(), g.clone());
    });
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_separate_buckets() {
        // 같은 글자라도 폰트 크기가 다르면 다른 버킷이어야 한다(줌 시 흐릿해지지 않게).
        let mut c = GlyphCache::default();
        c.map.entry(13.0_f32.to_bits()).or_default();
        c.map.entry(26.0_f32.to_bits()).or_default();
        assert_eq!(c.map.len(), 2);
    }

    #[test]
    fn invalidation_clears_on_ppp_or_atlas_change() {
        let mut c = GlyphCache::default();
        c.invalidate_if_stale(1.0, [512, 512]);
        c.map.entry(13.0_f32.to_bits()).or_default();
        c.invalidate_if_stale(1.0, [512, 512]);
        assert_eq!(c.map.len(), 1, "같은 상태면 유지");
        c.invalidate_if_stale(2.0, [512, 512]);
        assert!(c.map.is_empty(), "배율 변경 시 폐기");
        c.map.entry(13.0_f32.to_bits()).or_default();
        c.invalidate_if_stale(2.0, [1024, 1024]);
        assert!(c.map.is_empty(), "아틀라스 재생성 시 폐기");
    }
}
