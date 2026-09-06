//! nabiPad 미니맵 — 문서 전체를 우측에 축소 개요로 그리고, 현재 뷰포트를 표시한다.
//! 클릭/드래그하면 그 위치로 스크롤(반환한 목표 오프셋을 본문 ScrollArea가 적용).
//! 줄당이 아니라 미니맵 픽셀 행마다 한 줄을 매핑해 그려, 대용량에서도 비용이 일정하다.

use egui::{Color32, Rect, CornerRadius, Sense};

const BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 40); // 미니맵 배경.
const LINE: Color32 = Color32::from_rgba_premultiplied(170, 180, 200, 90); // 코드 줄.
const VIEW: Color32 = Color32::from_rgba_premultiplied(150, 170, 210, 36); // 현재 뷰포트.
const HIT: Color32 = Color32::from_rgb(230, 190, 80); // 찾기 일치.
const BOOK: Color32 = Color32::from_rgb(110, 180, 230); // 북마크.
const CUR: Color32 = Color32::from_rgb(240, 240, 240); // 커서가 있는 줄.

/// 미니맵 위에 얹을 표식들.
///
/// 터미널 스크롤바와 같은 생각이다 — 전체 중 어디인지를 이미 나타내는 자리에 눈금을 얹는
/// 것이, 같은 것을 알려 주는 화면을 새로 만드는 것보다 낫다. 편집기 쪽에서는 오래된
/// 관습이기도 하다(VS Code 의 오른쪽 눈금).
#[derive(Default, Clone, Copy)]
pub struct Marks<'a> {
    /// 찾기 일치 줄(0기반).
    pub hits: &'a [usize],
    /// 북마크 줄(0기반).
    pub bookmarks: &'a [usize],
    /// 커서가 있는 줄(0기반).
    pub cur_line: usize,
}

/// 줄 번호를 미니맵 **픽셀 행**으로. 문서 밖이면 None.
///
/// 미니맵이 줄을 고르는 식은 `li = py * n / rows`(내림)이다. 즉 픽셀 행 `py` 는 줄
/// `[py*n/rows, (py+1)*n/rows)` 구간을 대표한다. 그 구간에 `line` 이 드는 `py` 는
/// `line * rows / n`(역시 내림)이다.
///
/// ⚠️ 여기에 [`sampled_len`] 이 쓰는 `div_ceil` 을 그대로 가져다 썼다가 시험에 걸렸다.
/// 그쪽은 "줄 `i` 를 **정확히** 가리킨 첫 픽셀 행"을 찾는 계산이라 뜻이 다르다. 줄보다
/// 픽셀 행이 적을 때(큰 파일) 두 식은 크게 벌어진다 — 1000줄을 200행에 그리면 1번 줄의
/// 눈금이 5번 줄 자리에 찍혔다.
pub fn mark_row(line: usize, n_lines: usize, rows: usize) -> Option<usize> {
    match (n_lines, rows) {
        (0, _) | (_, 0) => None,
        (n, _) if line >= n => None,
        (n, r) => Some(line * r / n),
    }
}

/// 표식 줄들을 픽셀 행으로 바꾼다(오름차순·중복 제거) — 겹쳐 그려 봐야 보이지 않는다.
pub fn mark_rows(lines: &[usize], n_lines: usize, rows: usize) -> Vec<usize> {
    let mut v: Vec<usize> = lines.iter().filter_map(|&l| mark_row(l, n_lines, rows)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// 미니맵을 그린다. `off`/`content_h`/`viewport_h`는 직전 프레임의 본문 스크롤 상태.
/// 클릭/드래그 시 그 지점으로 가는 목표 스크롤 오프셋을 Some으로 돌려준다.
pub fn minimap(ui: &mut egui::Ui, text: &str, off: f32, content_h: f32, viewport_h: f32, marks: Marks<'_>) -> Option<f32> {
    let n = text.bytes().filter(|b| *b == b'\n').count() + 1;
    // 미니맵이 실제로 쓰는 줄은 픽셀 행 수(수백)뿐이다. 그런데 예전에는 매 프레임 문서 전체를
    // `Vec<&str>`로 쪼갰다 — 100만 줄이면 프레임마다 16MB를 잡았다 잃는 셈이었다.
    // 이제 필요한 줄만 골라 잰다(§`sampled_line_lens`). 훑기는 하지만 아무것도 잡지 않는다.
    let rows = ui.available_height().max(1.0) as usize;
    let lens = sampled_line_lens(text, n, rows);
    minimap_by(ui, n, |i| sampled_len(&lens, i, n, rows), off, content_h, viewport_h, marks)
}

/// 미니맵이 그릴 픽셀 행마다 대응하는 줄의 길이(글자 수, 줄 끝 공백 제외).
///
/// `rows`개만 재고 나머지는 보지 않는다. 미니맵은 픽셀 행마다 한 줄만 그리므로 그 밖의 줄은
/// 재 봐야 버린다 — 큰 파일에서 이 차이가 곧 프레임 시간이다.
/// 결과는 **픽셀 행마다 한 칸**(`rows`개)이다. 줄 번호로 색인하지 않는 이유는 아래 참조.
pub fn sampled_line_lens(text: &str, n_lines: usize, rows: usize) -> Vec<usize> {
    let rows = rows.max(1);
    let n = n_lines.max(1);
    let mut out = vec![0usize; rows];
    let mut py = 0usize;
    for (i, line) in text.split('\n').enumerate() {
        if py >= rows {
            break; // 필요한 줄을 다 쟀다 — 남은 문서는 훑지도 않는다.
        }
        // 줄보다 픽셀 행이 많으면 여러 py가 같은 줄을 가리킨다. 길이는 한 번만 잰다.
        let mut len: Option<usize> = None;
        while py < rows && py * n / rows == i {
            out[py] = *len.get_or_insert_with(|| line.trim_end().chars().count());
            py += 1;
        }
    }
    out
}

/// `sampled_line_lens`의 결과에서 **줄 번호** `i`에 해당하는 길이를 꺼낸다.
///
/// 미니맵은 `li = py * n / rows`로 줄을 고르므로, 역으로 `py = ceil(li * rows / n)`이 그 줄을
/// 가리킨 **첫 픽셀 행**이다. 같은 줄을 가리키는 픽셀 행들은 모두 같은 값을 담고 있으니
/// 어느 것을 꺼내도 같다.
fn sampled_len(lens: &[usize], i: usize, n_lines: usize, rows: usize) -> usize {
    let rows = rows.max(1);
    let n = n_lines.max(1);
    let py = (i * rows).div_ceil(n).min(rows - 1);
    lens.get(py).copied().unwrap_or(0)
}

/// 줄 수 + 줄 길이 클로저 버전 — rope 등 &str이 아닌 저장소도 같은 미니맵을 쓴다(T6-3).
pub fn minimap_by(
    ui: &mut egui::Ui,
    n_lines: usize,
    line_len: impl Fn(usize) -> usize,
    off: f32,
    content_h: f32,
    viewport_h: f32,
    marks: Marks<'_>,
) -> Option<f32> {
    let rect = ui.max_rect();
    let (mm_h, mm_w) = (rect.height().max(1.0), rect.width());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, BG);

    // 줄 길이를 가로 막대로(미니맵 픽셀 행 ↔ 문서 줄 매핑 — 행 수 무관 일정 비용).
    let n = n_lines.max(1);
    let rows = mm_h as usize;
    for py in 0..rows {
        let li = py * n / rows.max(1);
        let len = line_len(li).min(100);
        if len > 0 {
            let w = (len as f32 / 100.0) * (mm_w - 8.0);
            let y = rect.top() + py as f32;
            painter.rect_filled(Rect::from_min_size(egui::pos2(rect.left() + 4.0, y), egui::vec2(w, 1.0)), CornerRadius::ZERO, LINE);
        }
    }

    // 표식 — 뷰포트 박스보다 **먼저** 그린다. 박스는 반투명이라 눈금을 가리지 않는다.
    let bar = |ys: Vec<usize>, c: Color32, w: f32| {
        for py in ys {
            let y = rect.top() + py as f32;
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(w, 2.0)),
                CornerRadius::ZERO,
                c,
            );
        }
    };
    bar(mark_rows(marks.bookmarks, n, rows), BOOK, mm_w * 0.35);
    bar(mark_rows(marks.hits, n, rows), HIT, mm_w);
    bar(mark_row(marks.cur_line, n, rows).into_iter().collect(), CUR, mm_w);

    // 현재 뷰포트 박스.
    if content_h > 1.0 {
        let vy = rect.top() + (off / content_h) * mm_h;
        let vh = ((viewport_h / content_h) * mm_h).max(4.0);
        painter.rect_filled(Rect::from_min_size(egui::pos2(rect.left(), vy), egui::vec2(mm_w, vh)), CornerRadius::ZERO, VIEW);
    }

    // 클릭/드래그 → 그 지점을 뷰포트 중앙에 두는 목표 오프셋.
    let resp = ui.interact(rect, ui.id().with("mm_interact"), Sense::click_and_drag());
    if (resp.clicked() || resp.dragged()) && content_h > viewport_h {
        if let Some(p) = resp.interact_pointer_pos() {
            let frac = ((p.y - rect.top()) / mm_h).clamp(0.0, 1.0);
            return Some((frac * content_h - viewport_h / 2.0).clamp(0.0, content_h - viewport_h));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 눈금이 가리키는 픽셀 행은 그리는 쪽이 그 줄을 고르는 행과 같아야 한다.
    /// 다르면 큰 파일에서 눈금이 엉뚱한 자리를 가리키는데, 눈으로는 잡기 어렵다.
    #[test]
    fn 눈금은_그리는_행과_같은_줄을_가리킨다() {
        let (n, rows) = (1000usize, 200usize);
        for line in [0usize, 1, 499, 500, 999] {
            let py = mark_row(line, n, rows).expect("문서 안");
            // 그리는 쪽의 식: li = py * n / rows.
            let drawn = py * n / rows;
            assert!(drawn <= line, "줄 {line}: 눈금 행 {py} 이 {drawn} 줄을 그린다");
            assert!(line - drawn < n.div_ceil(rows) + 1, "줄 {line} 이 {drawn} 과 너무 멀다");
        }
    }

    /// 문서 밖·빈 문서에서는 아무것도 그리지 않는다.
    #[test]
    fn 밖이면_그리지_않는다() {
        assert_eq!(mark_row(10, 10, 100), None);
        assert_eq!(mark_row(0, 0, 100), None);
        assert_eq!(mark_row(0, 10, 0), None);
    }

    /// 겹친 표식은 한 번만, 떨어진 것은 각각.
    #[test]
    fn 겹친_눈금은_한_번만() {
        // 1000줄을 10px 에 - 100줄이 1px.
        assert_eq!(mark_rows(&[0, 1, 2, 3], 1000, 10), vec![0]);
        assert_eq!(mark_rows(&[0, 500, 999], 1000, 10), vec![0, 5, 9]);
        // 밖에 있는 것이 섞여도 나머지는 남는다.
        assert_eq!(mark_rows(&[9999, 0], 1000, 10), vec![0]);
    }

    /// 예전 방식(전체를 Vec 으로 쪼개 줄 번호로 재기)과 **같은 값**이 나와야 한다.
    /// 빨라진 대신 그림이 달라졌다면 그것은 고친 것이 아니라 바꾼 것이다.
    fn old_way(text: &str, i: usize) -> usize {
        text.split('\n').nth(i).map(|l| l.trim_end().chars().count()).unwrap_or(0)
    }

    #[test]
    fn sampling_matches_the_old_full_scan() {
        let text: String = (0..500).map(|i| format!("{}\n", "x".repeat(i % 37))).collect();
        let n = text.bytes().filter(|b| *b == b'\n').count() + 1;
        for rows in [1usize, 7, 64, 300, 900] {
            let lens = sampled_line_lens(&text, n, rows);
            for py in 0..rows {
                let li = py * n / rows;
                assert_eq!(
                    sampled_len(&lens, li, n, rows),
                    old_way(&text, li),
                    "rows={rows} py={py} li={li}"
                );
            }
        }
    }

    #[test]
    fn more_pixel_rows_than_lines_is_fine() {
        // 세 줄짜리 문서를 100픽셀 높이에 그리면 여러 픽셀 행이 같은 줄을 가리킨다.
        let text = "a\nbb\nccc";
        let lens = sampled_line_lens(text, 3, 100);
        assert_eq!(lens.len(), 100);
        assert_eq!(lens[0], 1);
        assert_eq!(*lens.last().unwrap(), 3);
    }

    #[test]
    fn trailing_whitespace_is_not_drawn() {
        // 줄 끝 공백까지 그리면 미니맵이 실제보다 넓어 보인다.
        let lens = sampled_line_lens("ab   \ncd", 2, 2);
        assert_eq!(lens, vec![2, 2]);
    }

    #[test]
    fn empty_text_does_not_panic() {
        assert_eq!(sampled_line_lens("", 1, 10).len(), 10);
        assert_eq!(sampled_line_lens("", 0, 0).len(), 1);
    }

    #[test]
    fn wide_chars_count_as_one_each() {
        // 미니맵은 글자 수로 폭을 잡는다(전각을 둘로 세면 한글 문서가 늘 꽉 차 보인다).
        let lens = sampled_line_lens("가나다", 1, 1);
        assert_eq!(lens, vec![3]);
    }
}
