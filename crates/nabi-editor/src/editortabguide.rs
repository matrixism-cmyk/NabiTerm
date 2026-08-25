//! 편집기 보조선 그리기(세로 눈금·들여쓰기 안내선) — editortab이 소프트 라인 한도에
//! 닿아 분리했다.
//!
//! 둘 다 글자 아래 층에 흐리게 긋는 같은 성격의 것이라 한 파일에 둔다.

/// 세로 눈금을 긋는다(설정한 열이 없으면 아무것도 안 한다).
pub(crate) fn draw_rulers(ui: &egui::Ui, doc: &crate::editor::EditorDoc, origin: egui::Pos2, h: f32, char_w: f32) {
    let cols = crate::rulers::parse_columns(&doc.rulers);
    let xs = crate::rulers::offsets(&cols, char_w);
    if xs.is_empty() {
        return;
    }
    // 글자보다 아래 층(Background)에 그려 본문을 가리지 않는다.
    let p = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Background, ui.id().with("rulers")));
    let color = ui.visuals().weak_text_color().gamma_multiply(0.35);
    for x in xs {
        let (a, b) = (egui::pos2(origin.x + x, origin.y), egui::pos2(origin.x + x, origin.y + h));
        p.line_segment([a, b], egui::Stroke::new(1.0, color));
    }
}

/// 들여쓰기 안내선을 긋는다. 눈금과 **같은 층**(글자 아래)에 같은 색으로 — 둘 다 보조선이고
/// 굵기·색이 다르면 화면이 어지러워진다.
pub(crate) fn draw_guides(
    ui: &egui::Ui,
    doc: &crate::editor::EditorDoc,
    galley: &egui::Galley,
    origin: egui::Pos2,
    char_w: f32,
) {
    if !doc.guides {
        return;
    }
    // 문서에는 탭 폭이 없다 — 화면 열 계산이 쓰는 것과 같은 값을 쓴다(눈금·줄바꿈과 일치).
    const TAB: usize = 4;
    let tab = TAB;
    let lines: Vec<&str> = doc.text.lines().collect();
    let depths = crate::guides::depths(&lines, tab);
    let p = ui
        .painter()
        .clone()
        .with_layer_id(egui::LayerId::new(egui::Order::Background, ui.id().with("guides")));
    let color = ui.visuals().weak_text_color().gamma_multiply(0.25);
    // galley의 행 하나하나에 그린다 — 줄바꿈된 행은 논리 줄을 따라간다.
    let mut logical = 0usize;
    for row in &galley.rows {
        let d = depths.get(logical).copied().unwrap_or(0);
        let (top, bot) = (origin.y + row.min_y(), origin.y + row.max_y());
        for x in crate::guides::offsets(d, tab, char_w) {
            let (a, b) = (egui::pos2(origin.x + x, top), egui::pos2(origin.x + x, bot));
            p.line_segment([a, b], egui::Stroke::new(1.0, color));
        }
        if row.ends_with_newline {
            logical += 1;
        }
    }
}
