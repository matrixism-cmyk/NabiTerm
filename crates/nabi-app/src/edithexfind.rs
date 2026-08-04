//! HEX 바이트 검색 — 16진 패턴(예: "DE AD") 또는 ASCII 문자열로 바이너리에서 찾는다.
//! 찾으면 일치 구간을 선택하고 그 줄로 스크롤한다(Ctrl+F로 찾기 바 토글).

use crate::edithex::{HexBuf, COLS};
use crate::edithexedit::parse_hex;
use nabi_i18n::{tr, Lang};

impl HexBuf {
    /// 검색 패턴 바이트(모드에 따라 ASCII 또는 16진). 빈 패턴이면 빈 벡터.
    fn find_pattern(&self) -> Vec<u8> {
        if self.find_text {
            self.find.as_bytes().to_vec()
        } else {
            parse_hex(&self.find)
        }
    }

    /// 바꿀 값 바이트(모드에 따라 ASCII 또는 16진).
    fn replacement(&self) -> Vec<u8> {
        if self.find_text {
            self.replace.as_bytes().to_vec()
        } else {
            parse_hex(&self.replace)
        }
    }

    /// 현재 선택이 찾기 패턴과 같으면 바꿀 값으로 교체하고, 다음 일치로 이동한다.
    pub(crate) fn replace_current(&mut self) {
        let pat = self.find_pattern();
        if !pat.is_empty() && self.selected_bytes() == pat {
            if let Some((lo, hi)) = self.selection() {
                let rep = self.replacement();
                let hi = hi.min(self.bytes.len());
                self.splice(lo, hi - lo, &rep, false);
                self.cursor = (lo + rep.len()).min(self.bytes.len().saturating_sub(1));
                self.anchor = None;
            }
        }
        self.find_step(true);
    }

    /// 모든 일치를 바꾼다(겹치지 않게 한 번에 재작성). 바꾼 횟수를 돌려준다.
    pub(crate) fn replace_all(&mut self) -> usize {
        let (pat, rep) = (self.find_pattern(), self.replacement());
        if pat.is_empty() || pat.len() > self.bytes.len() {
            return 0;
        }
        let (mut out, mut i, mut n) = (Vec::with_capacity(self.bytes.len()), 0usize, 0usize);
        while i < self.bytes.len() {
            if self.bytes[i..].starts_with(&pat) {
                out.extend_from_slice(&rep);
                i += pat.len();
                n += 1;
            } else {
                out.push(self.bytes[i]);
                i += 1;
            }
        }
        if n > 0 {
            // 앞뒤 공통 부분을 빼고 실제로 달라진 가운데만 기록한다(전체 복제 방지).
            let (at, del, mid) = narrowed(&self.bytes, &out);
            self.splice(at, del, mid, false);
            self.cursor = self.cursor.min(self.bytes.len().saturating_sub(1));
            self.anchor = None;
        }
        n
    }

    /// 커서 다음(forward)/이전부터 패턴을 찾는다. 끝에 닿으면 반대쪽 끝에서 한 번 더(래핑).
    pub(crate) fn find_step(&mut self, forward: bool) {
        let pat = self.find_pattern();
        if pat.is_empty() || pat.len() > self.bytes.len() {
            return;
        }
        let hit = if forward {
            let from = self.cursor + 1;
            find_from(&self.bytes, &pat, from).or_else(|| find_from(&self.bytes, &pat, 0))
        } else {
            let upto = self.cursor;
            rfind_before(&self.bytes, &pat, upto).or_else(|| rfind_before(&self.bytes, &pat, self.bytes.len()))
        };
        if let Some(pos) = hit {
            self.cursor = pos;
            self.anchor = Some(pos + pat.len() - 1); // 일치 구간 선택.
            self.scroll_to = Some(pos / COLS);
            self.low_nibble = false;
        }
    }
}

/// 두 바이트열의 공통 앞·뒤를 잘라내 실제로 다른 가운데만 남긴다.
/// 반환 = (시작 위치, 지울 길이, 새로 넣을 조각).
fn narrowed<'a>(old: &[u8], new: &'a [u8]) -> (usize, usize, &'a [u8]) {
    let mut p = 0;
    while p < old.len() && p < new.len() && old[p] == new[p] {
        p += 1;
    }
    let mut s = 0;
    while s < old.len() - p && s < new.len() - p && old[old.len() - 1 - s] == new[new.len() - 1 - s] {
        s += 1;
    }
    (p, old.len() - p - s, &new[p..new.len() - s])
}

/// `from` 이상에서 needle이 처음 나타나는 시작 위치.
fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|i| from + i)
}

/// `upto` 미만에서 needle이 마지막으로 나타나는 시작 위치.
fn rfind_before(hay: &[u8], needle: &[u8], upto: usize) -> Option<usize> {
    let end = upto.min(hay.len());
    if needle.is_empty() || end < needle.len() {
        return None;
    }
    hay[..end].windows(needle.len()).rposition(|w| w == needle)
}

/// HEX 찾기/바꾸기 바: 입력 + 16진/ASCII 토글 + 이전/다음 + (편집 가능 시) 바꾸기.
pub(crate) fn find_bar(ui: &mut egui::Ui, h: &mut HexBuf, readonly: bool, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label("\u{1f50d}");
        let hint = if h.find_text { "ABC" } else { "DE AD" };
        let r = ui.add(egui::TextEdit::singleline(&mut h.find).desired_width(150.0).hint_text(hint));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            h.find_step(true); // Enter = 다음 찾기(텍스트 찾기와 동일 UX).
        }
        ui.toggle_value(&mut h.find_text, "Aa").on_hover_text(tr(lang, "nabipad.hexfindtext"));
        if ui.button("\u{25b2}").on_hover_text(tr(lang, "find.prev")).clicked() {
            h.find_step(false);
        }
        if ui.button("\u{25bc}").on_hover_text(tr(lang, "find.next")).clicked() {
            h.find_step(true);
        }
        if !readonly {
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut h.replace).desired_width(150.0).hint_text(tr(lang, "find.replace")));
            if ui.button(tr(lang, "nabipad.replaceone")).clicked() {
                h.replace_current();
            }
            if ui.button(tr(lang, "find.replaceall")).clicked() {
                h.replace_all();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_hex_and_text() {
        let mut h = HexBuf::from_bytes(b"abcXYZabc".to_vec());
        h.find_text = true;
        h.find = "abc".into();
        h.cursor = 0;
        h.find_step(true); // 0 다음의 abc(6)
        assert_eq!(h.cursor, 6);
        h.find_step(true); // 래핑 → 0
        assert_eq!(h.cursor, 0);
    }

    #[test]
    fn replace_all_works() {
        let mut h = HexBuf::from_bytes(vec![0xAA, 0xBB, 0xAA, 0xBB]);
        h.find = "AABB".into();
        h.replace = "FF".into();
        assert_eq!(h.replace_all(), 2);
        assert_eq!(h.bytes, vec![0xFF, 0xFF]);
        h.undo();
        assert_eq!(h.bytes, vec![0xAA, 0xBB, 0xAA, 0xBB]);
    }

    #[test]
    fn finds_hex_pattern() {
        let mut h = HexBuf::from_bytes(vec![0x00, 0xDE, 0xAD, 0x00]);
        h.find = "DEAD".into();
        h.cursor = 0;
        h.find_step(true);
        assert_eq!(h.cursor, 1);
    }
}
