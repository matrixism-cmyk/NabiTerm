//! 설정의 가변 목록 편집기(스니펫·키워드 강조) — settingsui에서 분리(라인 한도).

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

/// 키워드 하이라이트 규칙 편집(추가/수정/삭제). 출력에서 이 단어들이 강조된다.
pub(crate) fn highlight_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.highlightshint"));
    list_editor(ui, &mut cfg.terminal.highlight_keywords, 220.0, tr(lang, "settings.addhighlight"));
    ui.weak(tr(lang, "settings.highlightsregexhint")); // 정규식 문법 안내 — 모르면 없는 기능이다.
    ui.separator();
    ui.label(tr(lang, "settings.alertshint")); // 출력 트리거 알림 패턴.
    list_editor(ui, &mut cfg.terminal.alert_patterns, 220.0, tr(lang, "settings.addalert"));
    ui.weak(tr(lang, "settings.alertactions")); // C4: 접미 액션 문법 안내.
    // 자동 응답은 같은 규칙 목록의 `-> reply:` 액션으로 쓴다(목록을 둘로 나누지 않는다).
    // 스위치를 바로 옆에 두는 이유는 **켜야 동작한다**는 것을 규칙 옆에서 알리기 위해서다.
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.terminal.auto_reply, tr(lang, "settings.autoreply"));
    ui.weak(tr(lang, "settings.autoreply.help"));
}

/// 명령 스니펫 편집(추가/수정/삭제). 메뉴에서 클릭하면 포커스 pane에 전송·실행.
pub(crate) fn snippet_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.snippetshint"));
    list_editor(ui, &mut cfg.terminal.snippets, 280.0, tr(lang, "settings.addsnippet"));
}

/// 문자열 목록 공용 편집기: 행마다 입력칸+✕, 아래 +추가 버튼.
fn list_editor(ui: &mut egui::Ui, items: &mut Vec<String>, width: f32, add_label: &str) {
    let mut remove = None;
    for (i, s) in items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(s).desired_width(width));
            if ui.small_button("\u{2715}").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        items.remove(i);
    }
    if ui.button(add_label).clicked() {
        items.push(String::new());
    }
}
