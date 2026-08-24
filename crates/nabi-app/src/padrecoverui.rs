//! 미저장 문서 복구 안내 — 시작할 때 한 번 묻는다(padrecover의 UI 쪽).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 지난 실행이 비정상 종료였는지 확인해 되살릴 문서를 챙겨 둔다(시작 시 1회).
    pub(crate) fn load_pad_recovery(&mut self) {
        self.pad_recover = crate::padrecover::take_all(&self.cfg_dir());
    }

    /// 되살릴 문서가 있으면 묻는다. 되살리기 전에는 파일을 지우지 않는다 —
    /// 여기서 앱이 또 죽더라도 다음 실행에 다시 물어봐야 한다.
    pub(crate) fn show_pad_recovery(&mut self, ctx: &egui::Context) {
        if self.pad_recover.is_empty() {
            return;
        }
        let lang = self.lang;
        let (mut restore, mut discard) = (false, false);
        crate::modal::foreground_modal(ctx, "pad_recover", |ui| {
            ui.heading(tr(lang, "recover.title"));
            ui.label(tr(lang, "recover.body"));
            ui.add_space(6.0);
            for r in self.pad_recover.iter().take(8) {
                let head: String = r.text.lines().next().unwrap_or("").chars().take(50).collect();
                ui.horizontal(|ui| {
                    ui.label(format!("\u{1f4c4} {}", r.name));
                    ui.weak(head);
                });
            }
            if self.pad_recover.len() > 8 {
                ui.weak(format!("\u{2026} +{}", self.pad_recover.len() - 8));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "recover.restore")).clicked() {
                    restore = true;
                }
                if ui.button(tr(lang, "recover.discard")).clicked() {
                    discard = true;
                }
            });
        });
        if restore {
            for r in std::mem::take(&mut self.pad_recover) {
                self.open_recovered_doc(r);
            }
            crate::padrecover::clear(&self.cfg_dir());
        } else if discard {
            self.pad_recover.clear();
            crate::padrecover::clear(&self.cfg_dir());
        }
    }

    /// 되살린 내용을 새 문서 탭으로 연다 — **경로 없이**. 어디에 저장할지는 사용자가 정한다.
    fn open_recovered_doc(&mut self, r: crate::padrecover::Recovered) {
        let title = if r.name.is_empty() { tr(self.lang, "nabipad.newdoc").to_string() } else { r.name };
        let mut doc = nabi_editor::editor::EditorDoc::make(
            title, std::path::PathBuf::new(), None, r.text, true, self.font_size, "UTF-8".into(), "LF",
        );
        doc.dirty = true; // 아직 어디에도 저장되지 않았다 — 표시가 남아 있어야 한다.
        self.add_editor_tab(doc);
    }
}
