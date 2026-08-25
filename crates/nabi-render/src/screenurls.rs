//! 화면 전체의 소프트랩-인지 링크 감지 — 줄바꿈된 URL/경로도 한 링크로 이어 잡는다.
#![allow(clippy::needless_range_loop)]

use nabi_vt::RenderCell;

/// 화면의 링크 한 개 — 소프트랩으로 여러 시각 행에 걸칠 수 있어 행별 세그먼트로 보관.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenUrl {
    /// (행, 시작열, 끝열)[포함] 세그먼트들. 줄바꿈되면 2개 이상.
    pub segs: Vec<(usize, usize, usize)>,
    pub url: String,
}

impl ScreenUrl {
    /// (row, col)이 이 링크의 한 세그먼트에 포함되는가.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        self.segs.iter().any(|&(r, s, e)| r == row && col >= s && col <= e)
    }
}

/// 화면 전체에서 링크를 찾는다 — `wrapped[i]`가 true면 행 i가 다음 행으로 소프트랩된 것이라
/// 한 논리 줄로 이어 붙여 감지하므로, 줄바꿈된 URL/경로도 끊기지 않는다.
pub fn screen_urls(rows: &[Vec<RenderCell>], wrapped: &[bool]) -> Vec<ScreenUrl> {
    let mut out = Vec::new();
    // 그룹 간 재사용 스크래치(할당 재사용).
    let (mut chars, mut owner, mut map): (Vec<char>, Vec<usize>, Vec<(usize, usize)>) =
        (Vec::new(), Vec::new(), Vec::new());
    let mut r = 0;
    while r < rows.len() {
        let mut end = r;
        while *wrapped.get(end).unwrap_or(&false) && end + 1 < rows.len() {
            end += 1; // 소프트랩 그룹 확장.
        }
        // URL 후보(':'·'.')가 있는 그룹만 처리(없으면 스캔 생략 — 성능).
        let hint = (r..=end).any(|gr| rows[gr].iter().any(|c| c.text.contains([':', '.'])));
        if hint {
            // 셀을 복제하지 않고 곧바로 (문자, 소유 셀 인덱스)로 평탄화한다. 버퍼는 그룹마다
            // 비워 재사용 — 예전엔 후보 그룹마다 RenderCell(문자열 포함)을 통째로 복제했다.
            chars.clear();
            owner.clear();
            map.clear();
            for gr in r..=end {
                for (c, cell) in rows[gr].iter().enumerate() {
                    crate::urls::push_cell(cell, map.len(), &mut chars, &mut owner);
                    map.push((gr, c));
                }
            }
            for sp in crate::urls::row_urls_from(&chars, &owner) {
                let mut segs: Vec<(usize, usize, usize)> = Vec::new();
                // map이 비면 `len-1`이 0으로 포화해 `0..=0`이 되고 인덱싱이 패닉한다.
                for k in sp.start..=sp.end.min(map.len().saturating_sub(1)) {
                    let Some(&(gr, gc)) = map.get(k) else { break };
                    match segs.last_mut() {
                        Some(s) if s.0 == gr && s.2 + 1 == gc => s.2 = gc,
                        _ => segs.push((gr, gc, gc)),
                    }
                }
                if !segs.is_empty() {
                    out.push(ScreenUrl { segs, url: sp.url });
                }
            }
        }
        r = end + 1;
    }
    out
}

/// (모델 주소, 세대, 스크롤)로 화면 URL 목록을 캐시해 빌려준다.
///
/// 호버 커서 판정이 매 프레임 화면 전체를 다시 스캔하던 비용을 없앤다(성능 리뷰 2026-08-19).
/// 페인터의 밑줄 맵 계산도 같은 캐시를 쓰므로, 한 프레임에 스캔은 최대 한 번이다.
/// ⚠️ `f`는 캐시 빌림 상태에서 실행된다 — 그 안에서 이 함수를 다시 부르지 말 것.
pub fn with_screen_urls<R>(
    key: usize,
    gen: u64,
    offset: usize,
    build: impl FnOnce() -> Vec<ScreenUrl>,
    f: impl FnOnce(&[ScreenUrl]) -> R,
) -> R {
    // 사용자 규칙이 바뀌면 화면 내용이 그대로여도 링크는 달라진다 — 세대를 섞어
    // 규칙을 고친 순간 다시 훑게 한다(안 그러면 스크롤할 때까지 옛 링크가 남는다).
    let gen = gen ^ (crate::urlrules::generation() << 32);
    let fresh = URL_CACHE
        .with(|c| c.borrow().get(&key).is_some_and(|(g, o, _)| *g == gen && *o == offset));
    if !fresh {
        let urls = build();
        URL_CACHE.with(|c| {
            let mut m = c.borrow_mut();
            if m.len() > 32 {
                m.clear(); // 닫힌 pane의 잔여 항목 정리(주소 재사용도 세대 키로 걸러진다).
            }
            m.insert(key, (gen, offset, urls));
        });
    }
    URL_CACHE.with(|c| {
        let b = c.borrow();
        f(b.get(&key).map_or(&[][..], |(_, _, v)| v.as_slice()))
    })
}

/// 모델별 캐시 항목: (세대, 스크롤 오프셋, 그 상태의 링크 목록).
type UrlEntry = (u64, usize, Vec<ScreenUrl>);

thread_local! {
    /// 페인트·호버는 모두 UI 스레드에서 일어난다(잠금 불필요). 키=모델 주소.
    static URL_CACHE: std::cell::RefCell<std::collections::HashMap<usize, UrlEntry>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(test)]
mod tests {
    use super::screen_urls;
    use nabi_types::{CellAttrs, Rgba};
    use nabi_vt::RenderCell;

    fn cells(s: &str) -> Vec<RenderCell> {
        s.chars()
            .map(|c| RenderCell { text: c.to_string(), fg: Rgba::WHITE, bg: Rgba::BLACK, attrs: CellAttrs::default(), ul_color: None })
            .collect()
    }

    #[test]
    fn screen_url_spans_softwrapped_rows() {
        // 경로가 줄바꿈(소프트랩)되면 두 행을 이어 붙여 한 링크로 잡는다.
        let urls = screen_urls(&[cells("C:/Users/Use"), cells("r/file.txt x")], &[true, false]);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "C:/Users/User/file.txt");
        assert!(urls[0].segs.iter().any(|&(r, _, _)| r == 0)); // 첫 행 세그먼트.
        assert!(urls[0].segs.iter().any(|&(r, _, _)| r == 1)); // 둘째 행 세그먼트.
    }

    // 실제 터미널 파이프라인 전체 검증: TermModel에 긴 URL을 흘려보내 소프트랩시키고,
    // render_rows + row_wrapped로 만든 입력에 screen_urls를 돌려 한 링크로 이어지는지 본다.
    // (사용자 신고: "줄바꿈된 링크 밑줄이 전혀 이어지지 않음" — 런타임 경로 회귀 가드.)
    #[test]
    fn end_to_end_softwrapped_url_is_one_link() {
        use nabi_types::GridSize;
        use nabi_vt::{TermModel, Theme};
        let mut m = TermModel::new(GridSize::new(20, 6), 100);
        // 20열을 넘기는 긴 URL → 자동 줄바꿈(WRAPLINE).
        m.process(b"https://example.com/very/long/path/page.html");
        let theme = Theme::default();
        let rows = m.render_rows(&theme);
        let wrapped: Vec<bool> = (0..rows.len()).map(|r| m.row_wrapped(r as u16)).collect();
        assert!(wrapped[0], "첫 행이 소프트랩이어야 함");
        let urls = screen_urls(&rows, &wrapped);
        assert_eq!(urls.len(), 1, "줄바꿈돼도 링크는 하나여야 함: {urls:?}");
        assert_eq!(urls[0].url, "https://example.com/very/long/path/page.html");
        // 두 행 이상에 세그먼트가 걸쳐야 한다(밑줄이 이어짐).
        let row_count = urls[0].segs.iter().map(|&(r, _, _)| r).collect::<std::collections::BTreeSet<_>>().len();
        assert!(row_count >= 2, "링크가 여러 행에 걸쳐야 함: {:?}", urls[0].segs);
    }

    /// 폭 0인 행이 섞여도 죽지 않는다 — 렌더러 패닉은 UI 스레드를 통째로 죽인다.
    #[test]
    fn empty_rows_do_not_panic() {
        let rows: Vec<Vec<RenderCell>> = vec![Vec::new(), Vec::new()];
        assert!(screen_urls(&rows, &[false, false]).is_empty());
        // wrapped가 행 수보다 길거나 짧아도(리사이즈 도중 어긋남) 버텨야 한다.
        assert!(screen_urls(&rows, &[true, true, true]).is_empty());
        assert!(screen_urls(&rows, &[]).is_empty());
        assert!(screen_urls(&[], &[true]).is_empty());
    }
}
