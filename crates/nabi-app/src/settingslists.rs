//! 설정의 가변 목록 편집기(스니펫·키워드 강조) — settingsui에서 분리(라인 한도).

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

/// 출력에서 강조할 낱말들.
pub(crate) fn highlight_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.highlightshint"));
    list_editor(ui, &mut cfg.terminal.highlight_keywords, 220.0, tr(lang, "settings.addhighlight"));
    // 정규식 문법 안내 — 모르면 없는 기능이다. `help_line` 은 폭에 맞춰 줄을 접는다.
    crate::settingsui::help_line(ui, tr(lang, "settings.highlightsregexhint"));
}

/// 출력 트리거 — 알림과 자동 응답이 **같은 목록**을 쓴다(액션으로 갈린다).
pub(crate) fn alert_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.alertshint"));
    list_editor(ui, &mut cfg.terminal.alert_patterns, 220.0, tr(lang, "settings.addalert"));
    crate::settingsui::help_line(ui, tr(lang, "settings.alertactions"));
    // 자동 응답 스위치는 규칙 목록 **바로 아래**에 둔다 — 켜야 동작한다는 것을
    // 규칙 옆에서 알려야 하기 때문이다(설정 페이지 다른 구석에 두면 못 찾는다).
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.terminal.auto_reply, tr(lang, "settings.autoreply"));
    crate::settingsui::help_line(ui, tr(lang, "settings.autoreply.help"));
}

/// 터미널 낱말을 주소로 만드는 규칙.
pub(crate) fn link_rule_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    crate::settingsui::help_line(ui, tr(lang, "settings.linkruleshint"));
    link_rules(ui, cfg, lang);
}

/// 사용자 정의 링크 규칙 편집 — 잘못된 규칙은 **그 자리에서** 표시한다.
///
/// 저장하고 나서 링크가 안 생기는 것을 보고 원인을 찾게 하면 안 된다. 정규식은 조용히
/// 틀리기 쉬운 것이라 특히 그렇다.
fn link_rules(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    let mut remove = None;
    for (i, rule) in cfg.terminal.link_rules.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(rule)
                    .desired_width(320.0)
                    .hint_text(r"PROJ-\d+ -> https://jira/browse/$0"),
            );
            if ui.small_button("\u{2715}").clicked() {
                remove = Some(i);
            }
            if !rule.trim().is_empty() {
                if let Some(why) = nabi_render::urlrules::rule_error(rule) {
                    let key = match why.as_str() {
                        "form" => "settings.linkrules.form",
                        _ => "settings.linkrules.regex",
                    };
                    ui.colored_label(egui::Color32::from_rgb(0xd0, 0x4a, 0x3a), tr(lang, key));
                }
            }
        });
    }
    if let Some(i) = remove {
        cfg.terminal.link_rules.remove(i);
    }
    if ui.button(tr(lang, "settings.addlinkrule")).clicked() {
        cfg.terminal.link_rules.push(String::new());
    }
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
