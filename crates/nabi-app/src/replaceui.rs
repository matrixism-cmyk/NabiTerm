//! 파일에서 찾아 바꾸기(Replace in Files) 모달 — 현재 로컬 브라우저 폴더 대상.
//! 안전: 미리보기(계산만)로 영향 범위 확인 후 "적용"에서만 파일을 기록한다(대소문자 구분).

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

impl NabiApp {
    /// 찾아 바꾸기 모달을 그린다(replace_open일 때). 적용 시 파일 기록 후 닫힘.
    pub(crate) fn show_replace_in_files(&mut self, ctx: &egui::Context) {
        if !self.replace_open {
            return;
        }
        let lang = self.lang;
        let mut close = false;
        crate::modal::foreground_modal(ctx, "replace_in_files", |ui| {
            ui.heading(tr(lang, "replace.title"));
            ui.label(format!("{}: {}", tr(lang, "replace.scope"), self.browser.path.display()));
            ui.add_space(4.0);
            egui::Grid::new("replace_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(tr(lang, "replace.find"));
                let a = ui.add(egui::TextEdit::singleline(&mut self.replace_find).desired_width(260.0));
                ui.end_row();
                ui.label(tr(lang, "replace.to"));
                let b = ui.add(egui::TextEdit::singleline(&mut self.replace_to).desired_width(260.0));
                ui.end_row();
                // 찾을 말이나 바꿀 말을 고치면 **미리보기 숫자를 지운다**(배치 AI).
                //
                // 안 지우면 옛 질의로 센 숫자가 새 질의 아래에 그대로 남는다. 사용자는 그
                // 숫자를 보고 "바꾸기"를 누르고, 실제로 바뀌는 것은 다른 개수다.
                // 아무것도 안 보여 주는 편이 틀린 숫자를 보여 주는 것보다 낫다.
                if a.changed() || b.changed() {
                    self.replace_count = None;
                }
            });
            if let Some((f, m)) = self.replace_count {
                ui.colored_label(crate::theme_ui::ACCENT, format!("{m} \u{00d7} · {f} {}", tr(lang, "find.files.unit")));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has = !self.replace_find.is_empty();
                if ui.add_enabled(has, egui::Button::new(tr(lang, "replace.preview"))).clicked() {
                    // 미리보기는 쓰지 않으므로 실패 목록이 비어 있다.
                    let (f, m, _) = crate::findfiles::replace_in_dir(&self.browser.path, &self.replace_find, &self.replace_to, false, 3000);
                    self.replace_count = Some((f, m));
                }
                if ui.add_enabled(has, egui::Button::new(tr(lang, "replace.apply"))).clicked() {
                    let (f, m, failed) = crate::findfiles::replace_in_dir(&self.browser.path, &self.replace_find, &self.replace_to, true, 3000);
                    // 못 쓴 파일이 있으면 **그것부터** 말한다. 숫자만 보여 주면 사용자는 전부
                    // 바뀐 줄 알고, 그것은 자기 소스 코드에 대해 거짓말을 듣는 것이다(배치 AF).
                    let done = format!("{} {m}\u{00d7}/{f}", tr(lang, "replace.title"));
                    let msg = match failed.is_empty() {
                        true => done,
                        false => format!(
                            "{} {} \u{00b7} {done}",
                            tr(lang, "replace.unwritable"),
                            failed.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                        ),
                    };
                    self.notify = Some((msg, Instant::now()));
                    close = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.replace_open = false;
            self.replace_count = None;
        }
    }
}
