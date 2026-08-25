//! 파일 속성 창 — fileprops가 읽어 온 것을 보여 주고, 요청하면 해시를 낸다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 속성 창을 연다(파일 브라우저 우클릭).
    pub(crate) fn open_file_props(&mut self, path: std::path::PathBuf) {
        self.file_props = crate::fileprops::read(&path);
        // 폴더면 안에 무엇이 얼마나 있는지도 함께 — 크기 칸이 0으로 보이면 뜻이 없다.
        if let Some(p) = self.file_props.as_mut().filter(|p| p.is_dir) {
            p.dir_total = Some(crate::browserops::dir_stats(&path));
        }
    }

    /// 창을 그린다. 해시는 단추를 눌렀을 때만, 곁 스레드에서.
    pub(crate) fn show_file_props(&mut self, ctx: &egui::Context) {
        let Some(p) = self.file_props.clone() else { return };
        let lang = self.lang;
        let (mut open, mut want_hash, mut copy) = (true, false, None);
        egui::Window::new(tr(lang, "props.title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                egui::Grid::new("props_grid").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
                    let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    row(ui, tr(lang, "props.name"), &name);
                    row(ui, tr(lang, "props.where"), &p.path.parent().map(|d| d.display().to_string()).unwrap_or_default());
                    let kind = if p.is_dir { tr(lang, "props.folder").to_string() } else { crate::fileprops::ext_of(&p.path) };
                    row(ui, tr(lang, "props.kind"), &kind);
                    let size = match p.dir_total {
                        Some((files, bytes)) => format!("{} \u{00b7} {files}", crate::browserfs::human(bytes)),
                        None => format!("{} ({} bytes)", crate::browserfs::human(p.bytes), p.bytes),
                    };
                    row(ui, tr(lang, "props.size"), &size);
                    row(ui, tr(lang, "props.modified"), &crate::fileprops::stamp(p.modified));
                    row(ui, tr(lang, "props.created"), &crate::fileprops::stamp(p.created));
                    row(ui, tr(lang, "props.readonly"), if p.readonly { "\u{2713}" } else { "" });
                });
                if p.is_dir {
                    return; // 폴더에는 해시가 없다.
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("SHA-256");
                    match &p.sha256 {
                        // 계산 중.
                        Some(None) => {
                            ui.spinner();
                            ui.weak(tr(lang, "props.hashing"));
                            ctx.request_repaint_after(std::time::Duration::from_millis(200));
                        }
                        Some(Some(h)) => {
                            ui.add(egui::Label::new(egui::RichText::new(h).monospace()).truncate());
                            if ui.small_button(tr(lang, "logview.copy")).clicked() {
                                copy = Some(h.clone());
                            }
                        }
                        None => {
                            // 자동으로 돌리지 않는다 — 큰 파일에서 창이 멈춘 것처럼 보인다.
                            if ui.button(tr(lang, "props.hash")).clicked() {
                                want_hash = true;
                            }
                        }
                    }
                });
            });
        if let Some(h) = copy {
            ctx.copy_text(h);
            self.notify = Some((tr(lang, "logview.copied").to_string(), std::time::Instant::now()));
        }
        if want_hash {
            self.start_hash(p.path.clone());
        }
        if !open {
            self.file_props = None;
        }
    }

    /// 해시를 곁 스레드에서 계산한다. 끝나면 채널로 돌아와 창에 채워진다.
    fn start_hash(&mut self, path: std::path::PathBuf) {
        if let Some(p) = self.file_props.as_mut() {
            p.sha256 = Some(None); // 계산 중 표시.
        }
        let tx = self.hash_tx.clone();
        std::thread::spawn(move || {
            let got = crate::fileprops::sha256_of(&path).unwrap_or_else(|e| format!("\u{2715} {e}"));
            let _ = tx.send((path, got));
        });
    }

    /// 곁 스레드가 낸 해시를 창에 반영한다(매 프레임).
    pub(crate) fn drain_hashes(&mut self) {
        while let Ok((path, hash)) = self.hash_rx.try_recv() {
            // 그 사이 다른 파일을 열었으면 버린다 — 엉뚱한 파일의 해시를 붙이면 안 된다.
            if let Some(p) = self.file_props.as_mut().filter(|p| p.path == path) {
                p.sha256 = Some(Some(hash));
            }
        }
    }
}

/// 이름·값 한 줄.
fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    ui.add(egui::Label::new(value).wrap());
    ui.end_row();
}
