//! 렌더 계산 캐시 — 내용(dirty_gen)·스크롤 위치·테마색이 그대로면 render_rows·밑줄맵을
//! 재계산하지 않고 재사용한다(유휴/커서깜빡임/스크롤 프레임의 CPU·할당 급감).
//!
//! TermModel이 Arc<Mutex<>>로 공유되므로 잠금 중엔 단일 스레드 접근 → 내부 RefCell 안전.
//! rows·밑줄을 별개 RefCell로 둔다 — rows 빌림(Ref)을 든 채 밑줄을 갱신해도 충돌이 없도록.

use crate::cell::{RenderCell, Theme};
use crate::grid::TermModel;
use nabi_types::Rgba;
use std::cell::Ref;

/// render_rows 캐시: 키(세대·스크롤·테마 fg/bg) + 행 데이터.
#[derive(Default)]
pub struct RowsCache {
    key: (u64, usize, u32, u32),
    rows: Vec<Vec<RenderCell>>,
}

/// 밑줄(링크+하이퍼링크) 맵 캐시: 키(세대·스크롤) + bool 맵.
/// key는 Option — 기본 (0,0)이 새 모델의 첫 (gen=0,offset=0) 호출과 충돌해 build를 건너뛰는
/// 결함을 막는다(None은 어떤 실제 키와도 다르므로 첫 호출은 반드시 미스).
#[derive(Default)]
pub struct UlCache {
    key: Option<(u64, usize)>,
    map: Vec<Vec<bool>>,
}

fn pack(c: Rgba) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

impl TermModel {
    /// 내용 변경 세대(렌더 캐시 키). process/resize마다 증가.
    pub fn render_gen(&self) -> u64 {
        self.dirty_gen
    }

    /// 캐시된 render_rows — (세대·스크롤·테마 fg/bg)가 같으면 재계산·재할당을 생략하고 빌려준다.
    pub fn rows_cached(&self, theme: &Theme) -> Ref<'_, Vec<Vec<RenderCell>>> {
        let key = (self.dirty_gen, self.scrollback_offset(), pack(theme.fg), pack(theme.bg));
        if self.rows_cache.borrow().key != key {
            let rows = self.render_rows(theme); // 캐시 빌림이 없는 상태에서 재계산.
            let mut c = self.rows_cache.borrow_mut();
            c.rows = rows;
            c.key = key;
        }
        Ref::map(self.rows_cache.borrow(), |c| &c.rows)
    }

    /// 캐시된 밑줄 맵 — 키(세대,스크롤)가 같으면 재계산 없이 빌려주고, 다르면 build로 계산 후
    /// 저장하고 빌려준다(rows_cached와 동일 패턴). 매 프레임 전체화면 Vec 복제를 없앤다.
    /// build는 ul_cache 빌림이 없는 상태에서 호출되므로 model의 다른 RefCell을 자유로이 쓴다.
    pub fn underlines_cached(
        &self,
        gen: u64,
        offset: usize,
        build: impl FnOnce() -> Vec<Vec<bool>>,
    ) -> Ref<'_, Vec<Vec<bool>>> {
        if self.ul_cache.borrow().key != Some((gen, offset)) {
            let map = build();
            let mut c = self.ul_cache.borrow_mut();
            c.map = map;
            c.key = Some((gen, offset));
        }
        Ref::map(self.ul_cache.borrow(), |c| &c.map)
    }
}

#[cfg(test)]
mod tests {
    use super::pack;
    use crate::grid::TermModel;
    use nabi_types::{GridSize, Rgba};

    #[test]
    fn pack_rgb_to_u32() {
        assert_eq!(pack(Rgba { r: 0xFF, g: 0, b: 0, a: 0xFF }), 0xFF_0000);
        assert_eq!(pack(Rgba { r: 0x12, g: 0x34, b: 0x56, a: 0 }), 0x12_3456); // alpha 무시.
    }

    #[test]
    fn underline_cache_keyed_by_gen_and_offset() {
        let m = TermModel::new(GridSize::new(10, 3), 50);
        let g = m.render_gen();
        // 첫 호출(미스) → build 실행 후 저장·빌림.
        assert_eq!(*m.underlines_cached(g, 0, || vec![vec![true, false]]), vec![vec![true, false]]);
        // 같은 키 → build 미실행(다른 값을 build해도 캐시된 값 반환).
        assert_eq!(*m.underlines_cached(g, 0, || vec![vec![false]]), vec![vec![true, false]]);
        // 세대 다름 → build 재실행(stale 방지).
        assert_eq!(*m.underlines_cached(g + 1, 0, || vec![vec![false, true]]), vec![vec![false, true]]);
    }
}
