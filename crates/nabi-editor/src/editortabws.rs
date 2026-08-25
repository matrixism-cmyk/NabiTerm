//! 공백 표시 그리기 — editortab이 소프트 라인 한도에 닿아 분리했다.
//!
//! 그리기만 하는 순수한 일이라 편집기 본문과 섞여 있을 이유가 없었다.

/// 공백/탭을 흐린 점·화살표로 오버레이(편집 가이드). 보이는 행만 처리(클립 밖은 건너뜀).
pub(crate) fn draw_whitespace(ui: &egui::Ui, galley: &egui::Galley, origin: egui::Pos2, mono: &egui::FontId) {
    let clip = ui.clip_rect();
    let faint = ui.visuals().weak_text_color();
    let painter = ui.painter();
    for row in &galley.rows {
        let y = origin.y + row.rect().top();
        if y + row.rect().height() < clip.top() || y > clip.bottom() {
            continue; // 화면 밖 행은 그리지 않음(대용량 정상 파일에서 비용 절감).
        }
        for g in &row.glyphs {
            let mark = match g.chr {
                ' ' => "\u{00b7}",
                '\t' => "\u{2192}",
                _ => continue,
            };
            painter.text(origin + g.pos.to_vec2(), egui::Align2::LEFT_TOP, mark, mono.clone(), faint);
        }
    }
}
