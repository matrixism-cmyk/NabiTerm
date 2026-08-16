//! nabiPad 일반 편집기 시각 보조 — 현재 줄 강조 + 괄호 매칭((), [], {}) 윤곽.
//! editortab.rs의 본문 렌더에서 호출한다(작은 파일에서만 — 비용 제한은 호출부에서).

use egui::text::CCursor;
use egui::{Color32, Painter, Rect, CornerRadius, Stroke};

// premultiplied 값은 RGB ≤ alpha를 지켜야 정상 알파혼합(반투명). RGB>alpha면 가산 혼합되어
// 거의 순백/과포화로 글자를 덮는다(현재 줄 안 보임 버그였음). 값=unmultiplied×alpha/255.
const CURLINE: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22); // 현재 줄 은은한 배경(white·a22).
const BRACKET: Color32 = Color32::from_rgb(120, 170, 100); // 괄호 매칭 윤곽.
const MATCH: Color32 = Color32::from_rgba_premultiplied(70, 60, 14, 90); // 찾기 일치(노랑·a90).
const WORDHL: Color32 = Color32::from_rgba_premultiplied(28, 38, 47, 60); // 커서 단어 출현(파랑·a60).

/// 파일:줄:열 점프의 대기 커서 오프셋(doc.find.pending_cursor)을 TextEdit에 적용한다(1회 소비).
pub(crate) fn apply_pending_cursor(ui: &egui::Ui, id: egui::Id, doc: &mut crate::editor::EditorDoc) {
    let Some(off) = doc.find.pending_cursor.take() else { return };
    if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
        let c = CCursor::new(off.min(doc.text.chars().count()));
        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(c, c)));
        st.store(ui.ctx(), id);
    }
}
const OPEN: [char; 3] = ['(', '[', '{'];
const CLOSE: [char; 3] = [')', ']', '}'];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 더블클릭 시 egui의 ASCII 전용 단어 선택을 Unicode 단어 기준으로 덮어쓴다(#6 한글 더블클릭).
pub(crate) fn unicode_word_dblclick(ui: &egui::Ui, out: &egui::text_edit::TextEditOutput, text: &str) {
    if !out.response.double_clicked() {
        return;
    }
    let Some(pos) = out.response.interact_pointer_pos() else { return };
    let idx = out.galley.cursor_from_pos(pos - out.galley_pos).index;
    let (s, e) = word_range_at(text, idx);
    if e > s {
        if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), out.response.id) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(s), egui::text::CCursor::new(e))));
            st.store(ui.ctx(), out.response.id);
        }
    }
}

/// 커서 char 인덱스의 단어 범위(s,e)를 Unicode 단어 기준으로 구한다(egui의 ASCII 전용 is_word_char
/// 대체 — 한글 더블클릭이 한 줄 통째 선택되던 문제 해결). 단어 위가 아니면 (idx,idx).
pub(crate) fn word_range_at(text: &str, idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let on_word = chars.get(idx).is_some_and(|c| is_word(*c)) || (idx > 0 && chars.get(idx - 1).is_some_and(|c| is_word(*c)));
    if !on_word {
        return (idx, idx);
    }
    let mut s = idx;
    while s > 0 && is_word(chars[s - 1]) {
        s -= 1;
    }
    let mut e = idx.min(chars.len());
    while e < chars.len() && is_word(chars[e]) {
        e += 1;
    }
    (s, e)
}

/// chars 내 q의 일치를 galley 위치에 강조한다(한 행 내 일치만). whole면 단어 경계 일치만.
#[allow(clippy::too_many_arguments)]
fn draw_match_rects(painter: &Painter, galley: &egui::Galley, origin: egui::Pos2, chars: &[char], q: &[char], ci: bool, whole: bool, color: Color32) {
    if q.is_empty() || q.len() > chars.len() {
        return;
    }
    let mut i = 0;
    while i + q.len() <= chars.len() {
        let eq = chars[i..i + q.len()].iter().zip(q.iter()).all(|(a, b)| if ci { a.eq_ignore_ascii_case(b) } else { a == b });
        let bound = !whole || ((i == 0 || !is_word(chars[i - 1])) && (i + q.len() == chars.len() || !is_word(chars[i + q.len()])));
        if eq && bound {
            let r0 = galley.pos_from_cursor(CCursor::new(i));
            let r1 = galley.pos_from_cursor(CCursor::new(i + q.len()));
            if (r0.top() - r1.top()).abs() < 0.5 {
                let rect = Rect::from_min_max(egui::pos2(origin.x + r0.left(), origin.y + r0.top()), egui::pos2(origin.x + r1.left(), origin.y + r0.bottom()));
                painter.rect_filled(rect, CornerRadius::ZERO, color);
            }
            i += q.len();
        } else {
            i += 1;
        }
    }
}

/// 찾기 질의의 모든 일치를 강조(평문 부분일치).
pub(crate) fn highlight_matches(painter: &Painter, galley: &egui::Galley, origin: egui::Pos2, text: &str, query: &str, ci: bool) {
    let (q, chars): (Vec<char>, Vec<char>) = (query.chars().collect(), text.chars().collect());
    draw_match_rects(painter, galley, origin, &chars, &q, ci, false, MATCH);
}

/// 커서(char idx)에 걸친 단어의 모든 출현을 은은히 강조(전체 단어 일치). 단어가 아니면 무시.
pub(crate) fn highlight_word(painter: &Painter, galley: &egui::Galley, origin: egui::Pos2, text: &str, idx: usize) {
    let chars: Vec<char> = text.chars().collect();
    if idx > chars.len() {
        return;
    }
    // 커서 좌우로 단어 경계 확장(커서가 단어 끝 바로 뒤일 수도 있어 idx-1도 본다).
    let probe = if chars.get(idx).is_some_and(|c| is_word(*c)) {
        idx
    } else if idx > 0 && chars.get(idx - 1).is_some_and(|c| is_word(*c)) {
        idx - 1
    } else {
        return;
    };
    let (mut s, mut e) = (probe, probe + 1);
    while s > 0 && is_word(chars[s - 1]) {
        s -= 1;
    }
    while e < chars.len() && is_word(chars[e]) {
        e += 1;
    }
    if e - s >= 2 {
        let word = &chars[s..e];
        draw_match_rects(painter, galley, origin, &chars, word, false, true, WORDHL);
    }
}

/// 커서가 있는 시각 행(galley row)의 배경을 은은히 칠한다(편집 위치 식별).
pub(crate) fn current_line_bg(painter: &Painter, galley: &egui::Galley, origin: egui::Pos2, body_w: f32, row: usize) {
    if let Some(r) = galley.rows.get(row) {
        let y = origin.y + r.rect().top();
        painter.rect_filled(Rect::from_min_size(egui::pos2(origin.x, y), egui::vec2(body_w, r.rect().height())), CornerRadius::ZERO, CURLINE);
    }
}

/// 커서에 인접한 괄호와 그 짝을 찾아 두 칸을 윤곽으로 강조한다.
pub(crate) fn highlight_brackets(painter: &Painter, galley: &egui::Galley, origin: egui::Pos2, char_w: f32, text: &str, idx: usize) {
    let Some((a, b)) = match_bracket(text, idx) else { return };
    for bi in [a, b] {
        let r = galley.pos_from_cursor(CCursor::new(bi));
        let cell = Rect::from_min_size(egui::pos2(origin.x + r.left(), origin.y + r.top()), egui::vec2(char_w, r.height().max(1.0)));
        painter.rect_stroke(cell, CornerRadius::ZERO, Stroke::new(1.0, BRACKET), egui::StrokeKind::Inside);
    }
}

/// 커서 위치(앞 글자 우선)의 괄호와 짝의 char 인덱스를 찾는다. 중첩 깊이를 센다.
#[allow(clippy::needless_range_loop)]
pub(crate) fn match_bracket(text: &str, idx: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    for probe in [idx.checked_sub(1), Some(idx)].into_iter().flatten() {
        let Some(&c) = chars.get(probe) else { continue };
        if let Some(oi) = OPEN.iter().position(|&o| o == c) {
            let (close, mut depth) = (CLOSE[oi], 0i32);
            for j in probe + 1..chars.len() {
                if chars[j] == c {
                    depth += 1;
                } else if chars[j] == close {
                    if depth == 0 {
                        return Some((probe, j));
                    }
                    depth -= 1;
                }
            }
        } else if let Some(ci) = CLOSE.iter().position(|&cl| cl == c) {
            let (open, mut depth) = (OPEN[ci], 0i32);
            for j in (0..probe).rev() {
                if chars[j] == c {
                    depth += 1;
                } else if chars[j] == open {
                    if depth == 0 {
                        return Some((probe, j));
                    }
                    depth -= 1;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{match_bracket, word_range_at};

    #[test]
    fn matches_and_nests() {
        assert_eq!(match_bracket("(a+b)", 0), Some((0, 4))); // 여는 괄호에서 정방향.
        assert_eq!(match_bracket("(a+b)", 5), Some((4, 0))); // 닫는 괄호 뒤 커서 → 역방향.
        assert_eq!(match_bracket("{[()]}", 3), Some((2, 3))); // 중첩 안쪽 짝.
        assert_eq!(match_bracket("foo", 1), None); // 괄호 없음.
    }

    #[test]
    fn word_range_unicode() {
        // 영문: 공백 경계로 한 단어. 한글: 어절(공백 경계) 단위 — egui의 ASCII 전용 경계 보완(#6).
        assert_eq!(word_range_at("hello world", 2), (0, 5)); // "hello"
        assert_eq!(word_range_at("hello world", 8), (6, 11)); // "world"
        assert_eq!(word_range_at("안녕 세계", 0), (0, 2)); // "안녕"(공백 전까지)
        assert_eq!(word_range_at("안녕 세계", 3), (3, 5)); // "세계"
        assert_eq!(word_range_at("a 안녕b", 2), (2, 5)); // 영문+한글 연속도 한 단어(is_alphanumeric)
        assert_eq!(word_range_at("a  b", 2), (2, 2)); // 양옆 공백 → 빈 범위.
    }
}
