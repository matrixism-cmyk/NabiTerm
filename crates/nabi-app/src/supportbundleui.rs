//! 진단 묶음 창 — **무엇이 들어가는지 보여 주고** 내보낸다.
//!
//! 남에게 보내는 것이라 모르고 보내게 하지 않는다. 미리보기를 먼저 보이고, 복사와
//! 파일 저장을 그다음에 둔다.

use crate::app::NabiApp;
use crate::supportbundle::{assemble, redact, Piece};
use nabi_i18n::tr;

impl NabiApp {
    /// 도움말·팔레트에서 연다.
    pub(crate) fn open_support_bundle(&mut self) {
        self.bundle = Some(self.build_bundle());
    }

    /// 지금 상태로 묶음을 만든다.
    fn build_bundle(&self) -> Vec<Piece> {
        let lang = self.lang;
        let mut v = Vec::new();
        v.push(Piece {
            title: tr(lang, "bundle.part.version").to_string(),
            body: format!(
                "nabiTerm {}\nOS: Windows\n{}: {}",
                env!("CARGO_PKG_VERSION"),
                tr(lang, "bundle.part.lang"),
                lang_name(lang)
            ),
        });
        // 세션은 **개수만** 센다 — 호스트·계정은 남의 인프라 정보다.
        v.push(Piece {
            title: tr(lang, "bundle.part.counts").to_string(),
            body: format!(
                "sessions={} panes={} editors={}",
                self.sessions.ssh_count().1,
                self.orch.panes.read().map(|p| p.len()).unwrap_or(0),
                self.editors.len()
            ),
        });
        // 로그는 뒷부분만, 그리고 비밀 꼴을 지운 뒤에.
        let dir = self.cfg_dir().join("logs");
        let log = match crate::logview::latest(&dir) {
            Some(l) => redact(&l.body),
            None => tr(lang, "logview.none").to_string(),
        };
        v.push(Piece { title: tr(lang, "bundle.part.log").to_string(), body: log });
        v
    }

    /// 열려 있으면 그린다.
    pub(crate) fn show_support_bundle(&mut self, ctx: &egui::Context) {
        let Some(pieces) = self.bundle.clone() else { return };
        let lang = self.lang;
        let mut open = true;
        let (mut copy, mut save) = (false, false);
        egui::Window::new(tr(lang, "bundle.title"))
            .open(&mut open)
            .default_size([760.0, 560.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(tr(lang, "bundle.intro"));
                ui.weak(tr(lang, "bundle.excluded"));
                ui.separator();
                ui.horizontal(|ui| {
                    copy = ui.button(tr(lang, "bundle.copy")).clicked();
                    save = ui.button(tr(lang, "bundle.save")).clicked();
                });
                ui.separator();
                egui::ScrollArea::both().id_salt("bundle_body").auto_shrink([false, false]).show(ui, |ui| {
                    for p in &pieces {
                        ui.strong(&p.title);
                        ui.add(
                            egui::Label::new(egui::RichText::new(&p.body).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.add_space(8.0);
                    }
                });
            });
        if copy {
            ctx.copy_text(assemble(&pieces));
            self.notify = Some((tr(lang, "bundle.copied").to_string(), std::time::Instant::now()));
        }
        if save {
            self.save_bundle(&assemble(&pieces));
        }
        if !open {
            self.bundle = None;
        }
    }

    /// 파일로 저장한다(사용자가 위치를 고른다 — 조용히 어딘가에 떨구지 않는다).
    fn save_bundle(&mut self, text: &str) {
        let name = format!("nabiTerm-diagnostics-{}.txt", env!("CARGO_PKG_VERSION"));
        let Some(path) = rfd::FileDialog::new().set_file_name(&name).save_file() else { return };
        let msg = match std::fs::write(&path, text) {
            Ok(()) => tr(self.lang, "bundle.saved").to_string(),
            Err(e) => format!("{}: {e}", tr(self.lang, "bundle.savefail")),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

/// 언어 이름(진단 묶음 표시용). `Lang`에 코드 함수가 없어 여기서 짧게 적는다.
fn lang_name(l: nabi_i18n::Lang) -> &'static str {
    match l {
        nabi_i18n::Lang::En => "en",
        nabi_i18n::Lang::Ko => "ko",
        nabi_i18n::Lang::Ja => "ja",
    }
}
