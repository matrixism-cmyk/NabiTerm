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
    pub fn replace_current(&mut self) {
        let pat = self.find_pattern();
        if !pat.is_empty() && self.selected_bytes() == pat {
            if let Some((lo, hi)) = self.selection() {
                let rep = self.replacement();
                let hi = hi.min(self.data.len());
                self.splice(lo, hi - lo, &rep, false);
                self.cursor = (lo + rep.len()).min(self.data.len().saturating_sub(1));
                self.anchor = None;
            }
        }
        self.find_step(true);
    }

    /// 모든 일치를 바꾼다. 바꾼 횟수를 돌려준다.
    ///
    /// 예전에는 전체를 새 `Vec`로 다시 써서 파일 크기만큼 메모리를 더 썼다. 지금은
    /// **일치한 자리만** 하나씩 고친다 — 조각 표에서 한 번의 교체는 조각 몇 개를 손대는 일이다.
    /// 뒤에서 앞으로 훑으므로 앞의 오프셋이 밀리지 않는다.
    pub fn replace_all(&mut self) -> usize {
        let (pat, rep) = (self.find_pattern(), self.replacement());
        if pat.is_empty() || pat.len() > self.data.len() {
            return 0;
        }
        // 겹치지 않는 일치 위치를 앞에서부터 모은 뒤 뒤에서부터 고친다.
        let mut hits = Vec::new();
        let mut at = 0usize;
        while let Some(pos) = self.data.find(&pat, at) {
            hits.push(pos);
            at = pos + pat.len();
        }
        // 뒤에서 앞으로 고친다(앞 오프셋이 밀리지 않는다). 첫 기록 뒤는 전부 같은 묶음이라
        // **취소 한 번**으로 모두 되돌아간다.
        for (k, &pos) in hits.iter().rev().enumerate() {
            self.splice_chained(pos, pat.len(), &rep, k > 0);
        }
        if !hits.is_empty() {
            self.cursor = self.cursor.min(self.data.len().saturating_sub(1));
            self.anchor = None;
        }
        hits.len()
    }

    /// 커서 다음(forward)/이전부터 패턴을 찾는다. 끝에 닿으면 반대쪽 끝에서 한 번 더(래핑).
    pub fn find_step(&mut self, forward: bool) {
        let pat = self.find_pattern();
        if pat.is_empty() || pat.len() > self.data.len() {
            return;
        }
        let hit = if forward {
            // 커서 다음부터, 끝에 닿으면 처음부터 한 번 더(래핑).
            self.data.find(&pat, self.cursor + 1).or_else(|| self.data.find(&pat, 0))
        } else {
            self.rfind_before(&pat, self.cursor).or_else(|| self.rfind_before(&pat, self.data.len()))
        };
        if let Some(pos) = hit {
            self.cursor = pos;
            self.anchor = Some(pos + pat.len() - 1); // 일치 구간 선택.
            self.scroll_to = Some(pos / COLS);
            self.low_nibble = false;
        }
    }

    /// `upto` **앞**에서 마지막으로 일치하는 위치.
    ///
    /// 앞에서부터 훑되 마지막 것만 남긴다. 조각 표는 뒤로 훑기가 비싸고 역방향 찾기는
    /// 드물어, 단순한 쪽이 낫다.
    fn rfind_before(&self, pat: &[u8], upto: usize) -> Option<usize> {
        let (mut at, mut last) = (0usize, None);
        while let Some(pos) = self.data.find(pat, at) {
            if pos >= upto {
                break;
            }
            last = Some(pos);
            at = pos + 1;
        }
        last
    }
}




/// HEX 찾기/바꾸기 바: 입력 + 16진/ASCII 토글 + 이전/다음 + (편집 가능 시) 바꾸기.
pub fn find_bar(ui: &mut egui::Ui, h: &mut HexBuf, readonly: bool, lang: Lang) {
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

    /// 모두 바꾸기는 **취소 한 번**으로 전부 되돌아가야 한다. 조각 표로 옮기면서
    /// 자리마다 따로 기록하게 됐고, 그때 이 성질이 깨졌다(고친 횟수만큼 눌러야 했다).
    #[test]
    fn replace_all_undoes_as_one_step() {
        let mut h = HexBuf::from_bytes(vec![0xAA, 0x11, 0xAA, 0x22, 0xAA, 0x33]);
        h.find = "AA".into();
        h.replace = "FF".into();
        assert_eq!(h.replace_all(), 3);
        assert_eq!(h.bytes(), vec![0xFF, 0x11, 0xFF, 0x22, 0xFF, 0x33]);
        h.undo();
        assert_eq!(h.bytes(), vec![0xAA, 0x11, 0xAA, 0x22, 0xAA, 0x33], "한 번에 전부 되돌아온다");
        h.redo();
        assert_eq!(h.bytes(), vec![0xFF, 0x11, 0xFF, 0x22, 0xFF, 0x33], "다시 실행도 한 번에");
    }

    #[test]
    fn replace_all_works() {
        let mut h = HexBuf::from_bytes(vec![0xAA, 0xBB, 0xAA, 0xBB]);
        h.find = "AABB".into();
        h.replace = "FF".into();
        assert_eq!(h.replace_all(), 2);
        assert_eq!(h.bytes(), vec![0xFF, 0xFF]);
        h.undo();
        assert_eq!(h.bytes(), vec![0xAA, 0xBB, 0xAA, 0xBB]);
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
