//! 터미널 인코딩 오류 감지(P4) — 디코드 결과의 치환문자(U+FFFD) 비율로 현재 인코딩이
//! 틀렸는지 판정하고 한국 환경의 흔한 대안을 제안한다. 순수 함수 위주(단위테스트).
//!
//! 설계 근거: 인코딩이 틀리면 디코더(encoding_rs/alacritty UTF-8 파서)가 U+FFFD를 만든다.
//! raw 바이트는 오케스트레이터에만 있어 앱은 못 받으므로, 이미 디코드된 화면 텍스트의
//! 치환문자 비율로 "현재 인코딩이 깨졌다"를 감지한다(확정 감지가 아닌 전환 제안).

use crate::app::NabiApp;
pub(crate) use nabi_editor::encdetect::{replacement_ratio, suggest_alt};
use nabi_types::PaneId;


/// 화면 텍스트/raw 표본으로 제안을 계산한다(순수 — 캐시 미스 때만 호출).
fn compute_suggestion(md: &nabi_vt::TermModel, current: &str) -> Option<&'static str> {
    if replacement_ratio(&md.visible_text(60)) < 0.02 {
        return None; // 정상(깨짐 거의 없음).
    }
    // 정확 감지: raw(디코드 전) 표본에 chardetng. 현재와 다른 인코딩이면 그걸 제안.
    let sample = md.detect_sample();
    if sample.len() >= 64 {
        let name = crate::editload::detect_encoding(sample).name();
        if !name.eq_ignore_ascii_case(current) {
            return Some(name);
        }
    }
    suggest_alt(current, 1.0) // 표본 부족/동일 → 한국 환경 흔한 대안 토글.
}

// 현재 인코딩과 치환문자 비율로 전환을 제안한다(순수 로직은 nabi_editor::encdetect).
impl NabiApp {
    /// 포커스 pane을 검사해 인코딩 전환 제안을 계산한다(없으면 None).
    /// 화면이 깨졌으면(U+FFFD↑) raw 표본으로 chardetng 정확 감지(B9), 표본 부족 시 토글 폴백(P4).
    ///
    /// **캐시**(성능 리뷰 2026-08-19): 이 판정은 상태바가 매 프레임 부르는데, 계산은 화면
    /// 60줄을 문자열로 덤프(줄마다 할당+12KB 복사)한다. 화면 내용이 바뀌지 않으면 결과도
    /// 같으므로 `render_gen`(+현재 인코딩)으로 메모이즈해 재계산을 건너뛴다.
    pub(crate) fn enc_suggestion(&mut self, pane: PaneId, current: &str) -> Option<&'static str> {
        let model = self.orch.panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.model.clone()))?;
        let md = model.lock().ok()?;
        let gen = md.render_gen();
        if let Some((p, g, c, r)) = &self.enc_cache {
            if *p == pane && *g == gen && c == current {
                return *r;
            }
        }
        let r = compute_suggestion(&md, current);
        drop(md);
        self.enc_cache = Some((pane, gen, current.to_owned(), r));
        r
    }

    /// 상태바 인코딩 컨트롤: 메뉴(직접 선택) + 깨짐 감지 시 제안 칩. 선택된 라벨 반환.
    pub(crate) fn encoding_controls(&self, ui: &mut egui::Ui, encoding: &str, suggest: Option<&str>) -> Option<String> {
        let mut chosen = None;
        ui.menu_button(encoding, |ui| {
            if let Some(e) = crate::encodings::encoding_menu(ui, encoding) { chosen = Some(e); }
        })
        .response
        .on_hover_text(nabi_i18n::tr(self.lang, "status.cycleenc"));
        if let Some(alt) = suggest {
            let chip = egui::RichText::new(format!("\u{26a0} {alt}?")).color(crate::theme_ui::BROADCAST);
            if ui.selectable_label(false, chip).on_hover_text(nabi_i18n::tr(self.lang, "status.encsuggest")).clicked() {
                chosen = Some(alt.to_string());
            }
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_counts_replacement_chars() {
        assert_eq!(replacement_ratio(""), 0.0);
        assert!(replacement_ratio("abc").abs() < 1e-6);
        assert!((replacement_ratio("a\u{fffd}\u{fffd}\u{fffd}") - 0.75).abs() < 1e-6);
        assert!((replacement_ratio("\u{fffd}\u{fffd}") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn suggests_common_korean_alternate() {
        assert_eq!(suggest_alt("UTF-8", 0.5), Some("EUC-KR"));
        assert_eq!(suggest_alt("EUC-KR", 0.5), Some("UTF-8"));
        assert_eq!(suggest_alt("CP949", 0.5), Some("UTF-8"));
        // 비율이 낮으면(정상) 제안 안 함.
        assert_eq!(suggest_alt("UTF-8", 0.001), None);
        assert_eq!(suggest_alt("EUC-KR", 0.0), None);
    }
}
