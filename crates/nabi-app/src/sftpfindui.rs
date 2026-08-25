//! 원격 파일 찾기 창 — 질의를 받고, 트리를 요청하고, 결과에서 그 폴더로 보낸다.
//!
//! 트리 요청은 동기화가 쓰는 `SftpListTree`를 그대로 쓴다(`sftpfind` 모듈 주석 참고).
//! `seq`로 회신을 짝지으므로 동기화 창과 동시에 떠 있어도 서로 결과를 훔치지 않는다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_proto::Command;

/// 찾기 창의 상태. 창을 닫으면 통째로 버린다.
#[derive(Default, Clone)]
pub(crate) struct SftpFind {
    pub query: String,
    /// 훑을 뿌리(열 때의 현재 경로).
    pub root: String,
    /// 회신을 기다리는 요청 번호 — 다른 요청의 회신을 받지 않기 위해.
    pub pending: Option<u64>,
    pub hits: Vec<crate::sftpfind::Hit>,
    /// 상한에 걸려 못 담은 수.
    pub cut: usize,
    /// 한 번이라도 찾아 봤는가 — "결과 없음"과 "아직 안 찾음"을 구분한다.
    pub searched: bool,
}

impl NabiApp {
    /// 찾기 창을 연다(현재 경로를 뿌리로).
    pub(crate) fn open_sftp_find(&mut self) {
        let root = self.sftp.path.clone();
        self.sftp_find = Some(SftpFind { root, ..Default::default() });
    }

    /// 트리 회신 — 받은 목록을 걸러 결과로 만든다.
    pub(crate) fn on_find_tree(&mut self, seq: u64, files: Vec<(String, u64, u64)>) -> bool {
        let Some(f) = &mut self.sftp_find else { return false };
        if f.pending != Some(seq) {
            return false; // 내 회신이 아니다(동기화 창의 것일 수 있다).
        }
        f.pending = None;
        // 원격이 준 상대경로는 믿지 않는다 — `..` 탈출을 걷어낸다(동기화와 같은 규칙).
        let safe: Vec<_> = files.into_iter().filter(|(r, _, _)| crate::syncplan::safe_rel(r)).collect();
        let (hits, cut) = crate::sftpfind::filter(&safe, &f.query);
        f.hits = hits;
        f.cut = cut;
        f.searched = true;
        true
    }

    /// 찾기 창을 그린다.
    pub(crate) fn show_sftp_find(&mut self, ctx: &egui::Context) {
        if self.sftp_find.is_none() {
            return;
        }
        let lang = self.lang;
        let st = self.sftp_find.clone().unwrap_or_default();
        let mut next = st.clone();
        let (mut open, mut search, mut goto) = (true, false, None);
        egui::Window::new(tr(lang, "sftp.find.title"))
            .open(&mut open)
            .default_size([640.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut next.query)
                            .desired_width(300.0)
                            .hint_text(tr(lang, "sftp.find.hint")),
                    );
                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        search = true;
                    }
                    if ui.button(tr(lang, "find.search")).clicked() {
                        search = true;
                    }
                });
                ui.weak(format!("{}: {}", tr(lang, "sftp.find.under"), st.root));
                ui.separator();
                if st.pending.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr(lang, "sftp.find.working"));
                    });
                } else if st.searched {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}: {}", tr(lang, "findall.found"), st.hits.len()));
                        if st.cut > 0 {
                            // 조용히 자르면 "이게 전부"로 읽힌다.
                            ui.colored_label(
                                crate::theme_ui::BROADCAST,
                                format!("{} +{}", tr(lang, "findall.truncated"), st.cut),
                            );
                        }
                    });
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for h in &st.hits {
                        let label = format!("{}  ({})", h.rel, crate::browserfs::human(h.size));
                        let r = ui.add(
                            egui::Label::new(egui::RichText::new(label).monospace())
                                .sense(egui::Sense::click()),
                        );
                        if r.on_hover_text(tr(lang, "sftp.find.gohint")).clicked() {
                            goto = Some(crate::sftpfind::parent_of(&h.rel).to_string());
                        }
                    }
                });
            });
        self.sftp_find = open.then_some(next);
        if search {
            self.start_sftp_find();
        }
        if let Some(sub) = goto {
            self.goto_found_folder(&sub);
        }
    }

    /// 트리 요청을 보낸다.
    fn start_sftp_find(&mut self) {
        let Some(id) = self.sftp.id else { return };
        let (root, empty) = match &self.sftp_find {
            Some(f) => (f.root.clone(), f.query.trim().is_empty()),
            None => return,
        };
        if empty {
            return; // 빈 질의로 서버를 훑게 하지 않는다.
        }
        self.sync_seq += 1; // 동기화와 같은 번호줄을 쓴다 — 회신이 섞이지 않게.
        let seq = self.sync_seq;
        if let Some(f) = &mut self.sftp_find {
            f.pending = Some(seq);
            f.hits.clear();
            f.cut = 0;
        }
        self.orch.send(Command::SftpListTree { id, root, seq });
    }

    /// 찾은 항목이 있는 폴더로 이동한다(뿌리 기준 상대경로).
    fn goto_found_folder(&mut self, sub: &str) {
        let root = self.sftp_find.as_ref().map(|f| f.root.clone()).unwrap_or_default();
        let path = match sub.is_empty() {
            true => root,
            false => crate::sftppath::join_path(&root, sub),
        };
        if let Some(id) = self.sftp.id {
            self.remote_nav(id, path);
        }
    }
}
