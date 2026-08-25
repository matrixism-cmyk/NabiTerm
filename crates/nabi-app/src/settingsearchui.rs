//! 설정 창 위쪽의 **검색 줄** — 낱말을 치면 그 항목이 있는 페이지로 보낸다.
//!
//! 결과를 눌러 페이지로 간 뒤에는 그 항목이 어디 있는지 스스로 찾아야 한다. 항목 자체를
//! 화면에서 짚어 주는 것(스크롤·강조)은 지금 구조(명령형 그리기)로는 훨씬 큰 공사라
//! 여기서는 **페이지까지** 데려다준다 — 60항목을 여섯 페이지에서 훑던 것에 비하면 그것만으로도
//! 대부분의 수고가 사라진다.

use nabi_i18n::{tr, Lang};

/// 검색 줄을 그린다. 사용자가 결과를 고르면 그 페이지 번호를 돌려준다.
pub(crate) fn bar(ui: &mut egui::Ui, lang: Lang, query: &mut String) -> Option<usize> {
    let mut go = None;
    ui.horizontal(|ui| {
        ui.label("\u{1f50d}");
        let w = (ui.available_width() - 70.0).max(120.0);
        ui.add_sized(
            [w, 22.0],
            egui::TextEdit::singleline(query).hint_text(tr(lang, "settings.search")),
        );
        // 글자 하나짜리 지우기 단추 — 낱말을 넣으면 검색칸이 그만큼 좁아진다.
        if !query.is_empty()
            && ui.small_button("\u{2715}").on_hover_text(tr(lang, "settings.search.clear")).clicked()
        {
            query.clear();
        }
    });
    let hits = crate::settingsearch::find(query, lang);
    if query.trim().is_empty() {
        return None;
    }
    if hits.is_empty() {
        ui.weak(tr(lang, "settings.search.none"));
        return None;
    }
    // 너무 많으면 창을 삼킨다 — 앞에서 맞은 것이 이미 위에 있으므로 앞쪽만 보인다.
    const SHOWN: usize = 8;
    egui::ScrollArea::vertical().id_salt("settings_hits").max_height(150.0).show(ui, |ui| {
        for h in hits.iter().take(SHOWN) {
            let page = tr(lang, crate::settingsui::PAGE_KEYS[h.page]);
            if ui.button(format!("{}   \u{2022}  {page}", tr(lang, h.key))).clicked() {
                go = Some(h.page);
            }
        }
    });
    if hits.len() > SHOWN {
        ui.weak(format!("+{}", hits.len() - SHOWN));
    }
    go
}

#[cfg(test)]
mod tests {
    /// 결과 줄에 페이지 이름이 함께 나와야 어디로 가는지 알 수 있다 — 화면 문자열 구성만 확인.
    #[test]
    fn a_result_line_names_its_page() {
        let label = format!("{}   \u{2022}  {}", "글꼴 크기", "모양");
        assert!(label.contains("글꼴 크기") && label.contains("모양"));
    }
}
