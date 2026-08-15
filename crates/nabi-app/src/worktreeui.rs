//! 워크트리 UI(B6) — 만들기 입력 모달 + 목록(열기/제거) 모달. 팔레트로 연다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 팔레트: 워크트리 만들기(포커스 pane의 cwd 기준). 모달로 브랜치 이름을 받는다.
    pub(crate) fn open_worktree_prompt(&mut self) {
        self.worktree_prompt = Some(String::new());
    }

    /// 팔레트: 워크트리 목록.
    pub(crate) fn open_worktree_list(&mut self) {
        let cwd = self.focused_cwd();
        match crate::worktree::list(&cwd) {
            Ok(items) => self.worktree_list = Some((cwd, items)),
            Err(e) => self.notify = Some((format!("\u{2715} {e}"), std::time::Instant::now())),
        }
    }

    /// 포커스 pane의 cwd(OSC 7). 없으면 현재 프로세스 cwd.
    fn focused_cwd(&mut self) -> String {
        self.focused_pane()
            .and_then(|p| self.cwds.get(&p).cloned())
            .map(|c| crate::workspace::strip_uri_slash(&c))
            .unwrap_or_else(|| std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default())
    }

    /// 두 모달을 렌더한다(차단형 — 분리 창 위에도 뜨도록 foreground_modal).
    pub(crate) fn show_worktree_modals(&mut self, ctx: &egui::Context) {
        self.worktree_create_modal(ctx);
        self.worktree_list_modal(ctx);
    }

    fn worktree_create_modal(&mut self, ctx: &egui::Context) {
        let Some(mut branch) = self.worktree_prompt.clone() else { return };
        let lang = self.lang;
        let (mut go, mut close) = (false, false);
        crate::modal::foreground_modal(ctx, "worktree_create", |ui| {
            ui.strong(tr(lang, "wt.create"));
            ui.label(tr(lang, "wt.branch"));
            let r = ui.add(egui::TextEdit::singleline(&mut branch).hint_text("feat/my-change").desired_width(260.0));
            r.request_focus();
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "wt.make")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        });
        self.worktree_prompt = Some(branch.clone());
        if close {
            self.worktree_prompt = None;
        }
        if go && !branch.trim().is_empty() {
            self.worktree_prompt = None;
            let cwd = self.focused_cwd();
            match crate::worktree::create(&cwd, branch.trim()) {
                Ok(path) => {
                    // 워크트리에서 바로 작업 시작 — 새 탭(cwd=워크트리).
                    let p = path.display().to_string();
                    self.spawn_local_at(p.clone());
                    self.notify = Some((format!("\u{2713} {} {p}", tr(lang, "wt.created")), std::time::Instant::now()));
                }
                Err(e) => self.notify = Some((format!("\u{2715} {e}"), std::time::Instant::now())),
            }
        }
    }

    fn worktree_list_modal(&mut self, ctx: &egui::Context) {
        let Some((cwd, items)) = self.worktree_list.clone() else { return };
        let lang = self.lang;
        let mut close = false;
        let mut open_at: Option<String> = None;
        let mut remove_at: Option<String> = None;
        crate::modal::foreground_modal(ctx, "worktree_list", |ui| {
            ui.strong(tr(lang, "wt.list"));
            for wt in &items {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{}  [{}]", wt.path, wt.branch));
                    if ui.small_button(tr(lang, "wt.open")).clicked() { open_at = Some(wt.path.clone()); }
                    if ui.small_button(tr(lang, "wt.remove")).clicked() { remove_at = Some(wt.path.clone()); }
                });
            }
            if ui.button(tr(lang, "qc.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
        if let Some(p) = open_at {
            self.spawn_local_at(p);
            close = true;
        }
        if let Some(p) = remove_at {
            match crate::worktree::remove(&cwd, &p) {
                Ok(()) => {
                    // 목록 갱신(제거 반영).
                    self.worktree_list = crate::worktree::list(&cwd).ok().map(|v| (cwd.clone(), v));
                    self.notify = Some((format!("\u{2713} {}", tr(lang, "wt.removed")), std::time::Instant::now()));
                }
                Err(e) => self.notify = Some((format!("\u{2715} {e}"), std::time::Instant::now())),
            }
            return; // close 덮어쓰지 않게(목록 유지).
        }
        if close {
            self.worktree_list = None;
        }
    }
}
