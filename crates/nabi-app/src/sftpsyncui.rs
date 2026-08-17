//! 폴더 동기화 다이얼로그(S6-51~53, WinSCP식) — 방향·모드·기준 선택 → 미리보기 체크리스트 → 실행.
//!
//! 실행은 일반 전송 큐를 그대로 탄다(진행률·히스토리 공유). 삭제는 Mirror 모드에서만
//! 후보로 나오며 **기본 체크 해제** — 사용자가 명시적으로 선택해야만 지운다.

use crate::app::NabiApp;
use crate::syncplan::{plan, to_map, walk_local, SyncAction, SyncBy, SyncDir};
use nabi_i18n::tr;
use nabi_proto::Command;

/// 다이얼로그 상태(Some=열림).
pub struct SyncDlg {
    pub local: String,
    pub remote: String,
    pub dir: SyncDir,
    pub mirror: bool,
    pub by: SyncBy,
    /// 원격 트리 수집 대기 중인 seq.
    pub pending: Option<u64>,
    /// 미리보기 결과: (동작, 실행 체크 여부).
    pub items: Option<Vec<(SyncAction, bool)>>,
}

impl NabiApp {
    /// SFTP 연결이 열려 있을 때 현재 경로들로 다이얼로그를 연다.
    pub(crate) fn open_sync_dialog(&mut self) {
        if self.sftp.id.is_none() {
            self.notify = Some((tr(self.lang, "sync.needconn").to_string(), std::time::Instant::now()));
            return;
        }
        self.sync_dlg = Some(SyncDlg {
            local: self.browser.path.to_string_lossy().into_owned(),
            remote: self.sftp.path.clone(),
            dir: SyncDir::Up,
            mirror: false,
            by: SyncBy::SizeAndTime,
            pending: None,
            items: None,
        });
    }

    /// 원격 트리 회신(Event::SftpTree) — 로컬 walk와 비교해 계획을 만든다.
    pub(crate) fn on_sync_tree(&mut self, seq: u64, files: Vec<(String, u64, u64)>) {
        let Some(dlg) = &mut self.sync_dlg else { return };
        if dlg.pending != Some(seq) {
            return;
        }
        dlg.pending = None;
        let local = to_map(&walk_local(std::path::Path::new(&dlg.local)));
        let remote = to_map(&files);
        let (src, dst) = match dlg.dir {
            SyncDir::Up => (&local, &remote),
            SyncDir::Down => (&remote, &local),
        };
        let acts = plan(src, dst, dlg.by, dlg.mirror);
        // 삭제는 기본 체크 해제(안전) — 복사/갱신은 기본 체크.
        dlg.items = Some(acts.into_iter().map(|a| { let del = matches!(a, SyncAction::Delete(_)); (a, !del) }).collect());
    }

    pub(crate) fn show_sync_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.sync_dlg.take() else { return };
        let lang = self.lang;
        let mut open = true;
        let mut do_preview = false;
        let mut do_run = false;
        let (mut do_watch, mut stop_watch) = (false, false);
        let watch_on = self.sync_watch.is_some();
        egui::Window::new(tr(lang, "sync.title"))
            .open(&mut open).collapsible(false).resizable(true).default_size([620.0, 420.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "sync.local"));
                    ui.add(egui::TextEdit::singleline(&mut dlg.local).desired_width(380.0));
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "sync.remote"));
                    ui.add(egui::TextEdit::singleline(&mut dlg.remote).desired_width(380.0));
                });
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut dlg.dir, SyncDir::Up, tr(lang, "sync.up"));
                    ui.selectable_value(&mut dlg.dir, SyncDir::Down, tr(lang, "sync.down"));
                    ui.separator();
                    ui.selectable_value(&mut dlg.by, SyncBy::SizeAndTime, tr(lang, "sync.bytime"));
                    ui.selectable_value(&mut dlg.by, SyncBy::Size, tr(lang, "sync.bysize"));
                    ui.separator();
                    ui.checkbox(&mut dlg.mirror, tr(lang, "sync.mirror")).on_hover_text(tr(lang, "sync.mirrorhint"));
                });
                ui.horizontal(|ui| {
                    if ui.button(format!("\u{1f50d} {}", tr(lang, "sync.preview"))).clicked() { do_preview = true; }
                    // 원격 최신유지(S6-54): 로컬→원격 방향에서 감시 시작/중지.
                    if watch_on {
                        if ui.button(format!("\u{23f9} {}", tr(lang, "watch.stop"))).clicked() { stop_watch = true; }
                    } else if dlg.dir == SyncDir::Up
                        && ui.button(format!("\u{1f441} {}", tr(lang, "watch.start"))).on_hover_text(tr(lang, "watch.hint")).clicked()
                    {
                        do_watch = true;
                    }
                    if dlg.pending.is_some() { ui.spinner(); ui.label(tr(lang, "sync.scanning")); }
                    if let Some(items) = &dlg.items {
                        let on = items.iter().filter(|(_, c)| *c).count();
                        ui.label(format!("{} {on}/{}", tr(lang, "sync.planned"), items.len()));
                        if on > 0 && ui.button(format!("\u{25b6} {}", tr(lang, "sync.run"))).clicked() { do_run = true; }
                    }
                });
                if let Some(items) = &mut dlg.items {
                    ui.separator();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        for (a, checked) in items.iter_mut() {
                            let (mark, color) = match a {
                                SyncAction::Copy(_) => ("\u{2795}", crate::theme_ui::OK),
                                SyncAction::Update(_) => ("\u{21bb}", crate::theme_ui::BROADCAST),
                                SyncAction::Delete(_) => ("\u{2715}", crate::theme_ui::ERR),
                            };
                            ui.horizontal(|ui| {
                                ui.checkbox(checked, "");
                                ui.colored_label(color, mark);
                                ui.label(a.path());
                            });
                        }
                        if items.is_empty() { ui.weak(tr(lang, "sync.insync")); }
                    });
                }
            });
        if do_preview {
            if let Some(id) = self.sftp.id {
                self.sync_seq += 1;
                dlg.pending = Some(self.sync_seq);
                dlg.items = None;
                self.orch.send(Command::SftpListTree { id, root: dlg.remote.clone(), seq: self.sync_seq });
            }
        }
        if do_run {
            self.run_sync(&mut dlg);
        }
        if do_watch {
            self.sync_watch = Some(crate::sftpwatch::SyncWatch::new(dlg.local.clone(), dlg.remote.clone()));
            self.notify = Some((tr(self.lang, "watch.started").to_string(), std::time::Instant::now()));
        }
        if stop_watch {
            self.sync_watch = None;
        }
        if open {
            self.sync_dlg = Some(dlg);
        }
    }

    /// 체크된 항목 실행 — 복사/갱신은 전송 큐, 삭제는 파일 작업. 실행 후 목록 초기화.
    fn run_sync(&mut self, dlg: &mut SyncDlg) {
        let Some(id) = self.sftp.id else { return };
        let (lroot, rroot) = (dlg.local.clone(), dlg.remote.trim_end_matches('/').to_string());
        let items: Vec<_> = dlg.items.take().into_iter().flatten().filter(|(_, c)| *c).map(|(a, _)| a).collect();
        let mut n = 0usize;
        for a in items {
            let rel = a.path().to_string();
            let lpath = std::path::Path::new(&lroot).join(rel.replace('/', "\\"));
            let rpath = format!("{rroot}/{rel}");
            match (&a, dlg.dir) {
                (SyncAction::Copy(_) | SyncAction::Update(_), SyncDir::Up) => {
                    let size = std::fs::metadata(&lpath).map(|m| m.len()).unwrap_or(0);
                    let (l, name) = (lpath.to_string_lossy().into_owned(), rel.clone());
                    self.push_xfer(name, true, size, |xfer| Command::SftpUpload { id, xfer, local: l, remote: rpath });
                }
                (SyncAction::Copy(_) | SyncAction::Update(_), SyncDir::Down) => {
                    if let Some(dir) = lpath.parent() { let _ = std::fs::create_dir_all(dir); }
                    let l = lpath.to_string_lossy().into_owned();
                    self.push_xfer(rel.clone(), false, 0, |xfer| Command::SftpDownload { id, xfer, remote: rpath, local: l, resume: 0 });
                }
                (SyncAction::Delete(_), SyncDir::Up) => self.orch.send(Command::SftpRemove { id, path: rpath }),
                (SyncAction::Delete(_), SyncDir::Down) => { let _ = std::fs::remove_file(&lpath); }
            }
            n += 1;
        }
        self.notify = Some((format!("{} {n}", tr(self.lang, "sync.started")), std::time::Instant::now()));
    }
}
