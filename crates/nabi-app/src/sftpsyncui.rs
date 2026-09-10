//! 폴더 동기화 다이얼로그(S6-51~53, WinSCP식) — 방향·모드·기준 선택 → 미리보기 체크리스트 → 실행.
//!
//! 실행은 일반 전송 큐를 그대로 탄다(진행률·히스토리 공유). 삭제는 Mirror 모드에서만
//! 후보로 나오며 **기본 체크 해제** — 사용자가 명시적으로 선택해야만 지운다.

use crate::app::NabiApp;
use crate::syncplan::{plan, to_map, walk_local, SyncAction, SyncBy, SyncDir};
use nabi_i18n::tr;
use nabi_proto::Command;

/// 맞은편이 어디인가.
///
/// **열거형으로 둔다.** 새 갈래를 더하면 러스트가 이것을 받는 `match` 를 전부 찾아
/// 이름과 줄 번호까지 대 준다. 여기에 `bool` 을 뒀다면 배선을 빠뜨려도 아무 말이 없다
/// (이 저장소가 같은 실수를 여러 번 했다).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncPeer {
    /// 붙어 있는 SFTP 서버.
    Remote,
    /// 같은 PC 의 다른 폴더 — 백업 디스크·USB·네트워크 드라이브.
    Local,
}

/// 다이얼로그 상태(Some=열림).
pub struct SyncDlg {
    pub local: String,
    /// 맞은편 경로. `peer` 가 [`SyncPeer::Local`] 이면 로컬 폴더 경로다.
    pub remote: String,
    pub peer: SyncPeer,
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
            peer: SyncPeer::Remote,
            dir: SyncDir::Up,
            mirror: false,
            by: SyncBy::SizeAndTime,
            pending: None,
            items: None,
        });
    }

    /// **로컬 폴더끼리** 동기화 창을 연다 — SFTP 연결이 없어도 된다.
    ///
    /// 맞은편 기본값은 비워 둔다. 여기에 그럴듯한 폴더를 넣어 두면, 미러 모드에서
    /// 사용자가 안 읽고 실행했을 때 **엉뚱한 폴더를 지운다.** 빈 칸은 실행을 막는다.
    pub(crate) fn open_local_sync_dialog(&mut self) {
        self.sync_dlg = Some(SyncDlg {
            local: self.browser.path.to_string_lossy().into_owned(),
            remote: String::new(),
            peer: SyncPeer::Local,
            dir: SyncDir::Up,
            mirror: false,
            by: SyncBy::SizeAndTime,
            pending: None,
            items: None,
        });
    }

    /// 양쪽이 모두 로컬일 때의 미리보기 — 서버에 물을 것이 없으니 그 자리에서 끝난다.
    fn preview_local(&mut self, dlg: &mut SyncDlg) {
        let (a, b) = (std::path::Path::new(&dlg.local), std::path::Path::new(&dlg.remote));
        if !a.is_dir() || !b.is_dir() {
            self.notify = Some((tr(self.lang, "sync.badlocal").to_string(), std::time::Instant::now()));
            return;
        }
        // 같은 폴더를 양쪽에 넣으면 미러가 자기 자신을 지우려 든다. 막는다.
        if a == b {
            self.notify = Some((tr(self.lang, "sync.samedir").to_string(), std::time::Instant::now()));
            return;
        }
        let (ma, mb) = (to_map(&walk_local(a)), to_map(&walk_local(b)));
        let (src, dst) = match dlg.dir {
            SyncDir::Up => (&ma, &mb),
            SyncDir::Down => (&mb, &ma),
        };
        let acts = crate::syncmove::detect_moves(plan(src, dst, dlg.by, dlg.mirror), src, dst);
        dlg.items = Some(acts.into_iter().map(|x| { let del = matches!(x, SyncAction::Delete(_)); (x, !del) }).collect());
    }

    /// 원격 트리 회신(Event::SftpTree) — 로컬 walk와 비교해 계획을 만든다.
    pub(crate) fn on_sync_tree(&mut self, seq: u64, files: Vec<(String, u64, u64)>) {
        let Some(dlg) = &mut self.sync_dlg else { return };
        if dlg.pending != Some(seq) {
            return;
        }
        dlg.pending = None;
        // 로컬 루트가 폴더가 아니면(오타 등) 계획을 만들지 않는다 — Up+미러에서 빈 src로
        // 원격 전체가 삭제 후보가 되는 사고 차단(리뷰 #2).
        if !std::path::Path::new(&dlg.local).is_dir() {
            self.notify = Some((tr(self.lang, "sync.badlocal").to_string(), std::time::Instant::now()));
            return;
        }
        let local = to_map(&walk_local(std::path::Path::new(&dlg.local)));
        // 원격 서버가 준 상대경로는 신뢰하지 않는다 — `..` 등 탈출 경로 제거(리뷰 #1).
        let safe: Vec<_> = files.into_iter().filter(|(r, _, _)| crate::syncplan::safe_rel(r)).collect();
        let remote = to_map(&safe);
        let (src, dst) = match dlg.dir {
            SyncDir::Up => (&local, &remote),
            SyncDir::Down => (&remote, &local),
        };
        // 이름만 바뀐 파일은 올리지 않고 옮긴다(폴더 이름 하나에 5GB 를 다시 보내던 자리).
        let acts = crate::syncmove::detect_moves(plan(src, dst, dlg.by, dlg.mirror), src, dst);
        // 삭제는 기본 체크 해제(안전) — 복사/갱신은 기본 체크.
        //
        // **이동은 기본 체크한다.** 삭제를 꺼 두는 이유는 되돌릴 수 없어서인데, 이동은
        // 바이트가 그대로 대상 쪽에 남는다 — 잘못 옮겨도 도로 옮기면 된다. 오히려 꺼 두면
        // 새 자리에 파일이 생기지 않아 동기화가 조용히 반쪽이 된다.
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
        // 방향/기준/미러/경로가 바뀌면 기존 미리보기는 무효 — 스테일 계획 실행 차단(리뷰 #3).
        let key_before = (dlg.dir, dlg.by, dlg.mirror, dlg.local.clone(), dlg.remote.clone());
        egui::Window::new(tr(lang, "sync.title"))
            .open(&mut open).collapsible(false).resizable(true).default_size([620.0, 420.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "sync.local"));
                    ui.add(egui::TextEdit::singleline(&mut dlg.local).desired_width(380.0));
                });
                ui.horizontal(|ui| {
                    // 맞은편이 서버냐 폴더냐에 따라 이름이 달라야 한다 — 로컬 폴더 칸에
                    // "원격"이라고 적혀 있으면 사용자는 서버 경로를 적는다.
                    let label = match dlg.peer {
                        SyncPeer::Remote => "sync.remote",
                        SyncPeer::Local => "sync.otherlocal",
                    };
                    ui.label(tr(lang, label));
                    ui.add(egui::TextEdit::singleline(&mut dlg.remote).desired_width(380.0));
                    if dlg.peer == SyncPeer::Local
                        && ui.button("\u{1f4c1}").on_hover_text(tr(lang, "sync.pick")).clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            dlg.remote = p.to_string_lossy().into_owned();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    // 방향 이름도 맞은편에 따라 달라야 한다. 로컬끼리인데 "로컬 → 원격"이라고
                    // 적혀 있으면, 어느 폴더가 원본인지 읽어 낼 수가 없다 — 그리고 그 오해가
                    // 미러 모드에서는 **엉뚱한 폴더를 지우는** 일이 된다.
                    let (up, down) = match dlg.peer {
                        SyncPeer::Remote => ("sync.up", "sync.down"),
                        SyncPeer::Local => ("sync.uplocal", "sync.downlocal"),
                    };
                    ui.selectable_value(&mut dlg.dir, SyncDir::Up, tr(lang, up));
                    ui.selectable_value(&mut dlg.dir, SyncDir::Down, tr(lang, down));
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
                        // 최신유지는 원격에만 있다. 로컬끼리도 뜻은 있지만 감시기가 원격
                        // 전송 큐를 전제로 만들어져 있어, 배선 없이 단추만 두면 **눌러도
                        // 아무 일이 없다** — 그건 사용자에게 고장으로 보인다.
                        && dlg.peer == SyncPeer::Remote
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
                                SyncAction::Move { .. } => ("\u{2192}", crate::theme_ui::ACCENT),
                            };
                            // 이동은 **어디서 어디로**를 다 보여 준다 — 새 자리만 적으면
                            // 왜 올리지 않는지 알 수 없고, 잘못 짝지은 것도 눈에 안 띈다.
                            let text = match a {
                                SyncAction::Move { from, to } => format!("{from}  \u{2192}  {to}"),
                                other => other.path().to_string(),
                            };
                            ui.horizontal(|ui| {
                                ui.checkbox(checked, "");
                                ui.colored_label(color, mark);
                                ui.label(text);
                            });
                        }
                        if items.is_empty() { ui.weak(tr(lang, "sync.insync")); }
                    });
                }
            });
        if key_before != (dlg.dir, dlg.by, dlg.mirror, dlg.local.clone(), dlg.remote.clone()) {
            dlg.items = None;
        }
        if do_preview {
            match dlg.peer {
                SyncPeer::Local => {
                    dlg.items = None;
                    self.preview_local(&mut dlg);
                }
                SyncPeer::Remote => {
                    if let Some(id) = self.sftp.id {
                        self.sync_seq += 1;
                        dlg.pending = Some(self.sync_seq);
                        dlg.items = None;
                        self.orch.send(Command::SftpListTree { id, root: dlg.remote.clone(), seq: self.sync_seq });
                    }
                }
            }
        }
        if do_run {
            match dlg.peer {
                SyncPeer::Local => self.run_sync_local(&mut dlg),
                SyncPeer::Remote => self.run_sync(&mut dlg),
            }
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

    /// 로컬↔로컬 실행 — 전송 큐를 타지 않는다. 같은 디스크 안이라 곧바로 끝나고,
    /// 큐에 넣으면 진행률만 번쩍이고 사라진다.
    ///
    /// 안 된 것이 있으면 **몇 개인지 말한다.** 조용히 넘기면 "다 됐다"로 읽힌다.
    fn run_sync_local(&mut self, dlg: &mut SyncDlg) {
        let items: Vec<SyncAction> = dlg
            .items
            .take()
            .into_iter()
            .flatten()
            .filter(|(a, c)| *c && crate::syncplan::safe_rel(a.path()))
            .map(|(a, _)| a)
            .collect();
        let (a, b) = (std::path::PathBuf::from(&dlg.local), std::path::PathBuf::from(&dlg.remote));
        let (src, dst) = match dlg.dir {
            SyncDir::Up => (&a, &b),
            SyncDir::Down => (&b, &a),
        };
        let done = crate::synclocal::run(src, dst, &items);
        let tail = match done.failed {
            0 => String::new(),
            n => format!(" \u{00b7} {} {n}", tr(self.lang, "sync.failed")),
        };
        self.notify = Some((
            format!("{} {}{tail}", tr(self.lang, "sync.done"), done.total()),
            std::time::Instant::now(),
        ));
    }

    /// 체크된 항목 실행 — 복사/갱신은 전송 큐, 삭제는 파일 작업. 실행 후 목록 초기화.
    fn run_sync(&mut self, dlg: &mut SyncDlg) {
        let Some(id) = self.sftp.id else { return };
        let (lroot, rroot) = (dlg.local.clone(), dlg.remote.trim_end_matches('/').to_string());
        let checked: Vec<_> = dlg
            .items
            .take()
            .into_iter()
            .flatten()
            .filter(|(a, c)| *c && crate::syncplan::safe_rel(a.path()))
            .map(|(a, _)| a)
            .collect();
        // 내려받을 때만, **윈도우가 만들 수 없는 이름**을 따로 걸러 낸다(배치 AE).
        //
        // 리눅스 서버에는 2026-08-28T10:00:00.log 처럼 콜론이 든 이름이 흔한데 윈도우는 그
        // 이름으로 파일을 못 만든다. 예전에는 이런 파일이 목록에서부터 아예 안 보였다 —
        // 사용자에게는 "서버에 없다"로 읽혔다. 이제 보여 주되, **왜 못 받는지 말한다.**
        let (items, unwritable): (Vec<_>, Vec<_>) = match dlg.dir {
            SyncDir::Down => checked
                .into_iter()
                .partition(|a| crate::syncplan::writable_on_windows(a.path())),
            SyncDir::Up => (checked, Vec::new()),
        };
        if !unwritable.is_empty() {
            let names: Vec<&str> = unwritable.iter().take(3).map(|a| a.path()).collect();
            self.notify = Some((
                format!("{} {} \u{00b7} {}", tr(self.lang, "sync.unwritable"), unwritable.len(), names.join(", ")),
                std::time::Instant::now(),
            ));
        }
        // 업로드 전 원격 부모 폴더를 얕은→깊은 순으로 만들어 둔다(리뷰 #6 — 없으면 개별 실패).
        if dlg.dir == SyncDir::Up {
            let mut dirs: Vec<String> = Vec::new();
            // 이동도 부모가 있어야 한다 — 옮겨 갈 자리의 폴더가 없으면 이름 바꾸기가 실패한다.
            for a in items.iter().filter(|a| !matches!(a, SyncAction::Delete(_))) {
                let mut acc = String::new();
                for seg in a.path().split('/').collect::<Vec<_>>().split_last().map(|(_, init)| init).unwrap_or(&[]) {
                    acc = if acc.is_empty() { (*seg).to_string() } else { format!("{acc}/{seg}") };
                    if !dirs.contains(&acc) {
                        dirs.push(acc.clone());
                    }
                }
            }
            for d in dirs {
                self.orch.send(Command::SftpMkdir { id, path: format!("{rroot}/{d}") });
            }
        }
        let (mut n, mut movefail) = (0usize, 0usize);
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
                // 이동 — 바이트를 보내지 않는다. 실패해도 잃는 것은 없다: 옛 자리에 그대로
                // 남아 있고 다음 동기화가 다시 계획한다(그때는 올리기로 잡힐 수도 있다).
                (SyncAction::Move { from, .. }, SyncDir::Up) => {
                    self.orch.send(Command::SftpRename { id, from: format!("{rroot}/{from}"), to: rpath });
                }
                (SyncAction::Move { from, .. }, SyncDir::Down) => {
                    if let Some(dir) = lpath.parent() { let _ = std::fs::create_dir_all(dir); }
                    let old = std::path::Path::new(&lroot).join(from.replace('/', "\\"));
                    if std::fs::rename(&old, &lpath).is_err() {
                        movefail += 1; // 조용히 넘기면 새 자리에 파일이 없는 채로 "끝났다"가 된다.
                    }
                }
            }
            n += 1;
        }
        let tail = if movefail > 0 { format!(" \u{00b7} {} {movefail}", tr(self.lang, "sync.movefail")) } else { String::new() };
        self.notify = Some((format!("{} {n}{tail}", tr(self.lang, "sync.started")), std::time::Instant::now()));
    }
}
