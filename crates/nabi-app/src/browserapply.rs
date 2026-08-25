//! 브라우저 액션 적용 + 사이드패널/탭 진입점(렌더는 browser.rs).

use crate::app::NabiApp;
use crate::browser::{render_browser_tab, BrowserAct};
use std::path::PathBuf;

impl NabiApp {
    /// 파일 브라우저 사이드패널을 토글한다(열 때 포커스 pane의 cwd에서 시작).
    pub(crate) fn toggle_browser(&mut self) {
        self.browser.open = !self.browser.open;
        if self.browser.open {
            if let Some(d) = self.spawn_cwd() {
                self.browser.path = PathBuf::from(d);
            }
        }
    }

    /// 파일 브라우저를 새 도크 탭으로 연다(포커스 그룹에 — 여러 개 가능, 각자 독립 상태).
    /// 새 탭의 PaneId를 돌려준다(제어 평면이 경로 지정에 사용).
    pub(crate) fn open_browser_tab(&mut self) -> nabi_types::PaneId {
        let p = nabi_types::next_pane_id(); // 오케스트레이터 pane 없는 UI 전용 id.
        let mut panel = crate::browserpanel::BrowserPanel::default();
        if let Some(d) = self.spawn_cwd() {
            panel.path = std::path::PathBuf::from(d); // 포커스 셸의 cwd에서 시작.
        }
        self.browser_tabs.insert(p, panel);
        self.add_pane(p);
        p
    }

    /// 지정 경로에서 파일 브라우저를 연다 — mode 0=탭, 1=새 창(분리), 2=사이드바.
    pub(crate) fn open_browser_path(&mut self, path: PathBuf, mode: u8) {
        if mode == 2 {
            (self.browser.path, self.browser.open) = (path, true); // 사이드바.
            return;
        }
        let p = nabi_types::next_pane_id();
        self.browser_tabs.insert(p, crate::browserpanel::BrowserPanel { path, ..Default::default() });
        self.add_pane(p);
        if mode == 1 {
            if let Some(loc) = self.dock.find_tab(&p) { self.dock.remove_tab(loc); } // 새 창.
            self.floating.push(p);
        }
    }

    /// 열려 있는 브라우저 탭 상태(도크 순서대로 경로·보기·정렬)를 파일로 저장한다.
    pub(crate) fn save_browser_tabs(&self) {
        let saves: Vec<(String, u8, u8, bool, bool)> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, p)| self.browser_tabs.get(p))
            .map(|b| {
                (
                    b.path.to_string_lossy().into_owned(),
                    b.view.to_u8(),
                    b.sort.to_u8(),
                    b.sort_desc,
                    b.show_hidden,
                )
            })
            .collect();
        let path = self.workspace_path.with_extension("btabs");
        if saves.is_empty() {
            let _ = std::fs::remove_file(path);
        } else if let Ok(s) = ron::to_string(&saves) {
            let _ = std::fs::write(path, s);
        }
    }

    /// 저장된 브라우저 탭들을 다시 연다(시작 시). 생성한 PaneId들을 저장 순서대로 돌려준다.
    pub(crate) fn restore_browser_tabs(&mut self) -> Vec<nabi_types::PaneId> {
        let mut out = Vec::new();
        let Some(s) = std::fs::read_to_string(self.workspace_path.with_extension("btabs")).ok()
        else {
            return out;
        };
        let Ok(saves) = ron::from_str::<Vec<(String, u8, u8, bool, bool)>>(&s) else {
            return out;
        };
        for (path, view, sort, sort_desc, show_hidden) in saves {
            let panel = crate::browserpanel::BrowserPanel {
                path: PathBuf::from(path),
                view: crate::viewmode::ViewMode::from_u8(view),
                sort: crate::browserfs::Sort::from_u8(sort),
                sort_desc,
                show_hidden,
                ..Default::default()
            };
            let p = nabi_types::next_pane_id();
            self.browser_tabs.insert(p, panel);
            self.add_pane(p);
            out.push(p);
        }
        out
    }

    /// 현재 도크 순서의 브라우저 탭 pane들(수동 워크스페이스 복원 시 레이아웃 매핑용).
    pub(crate) fn dock_browser_panes(&self) -> Vec<nabi_types::PaneId> {
        self.dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .filter(|p| self.browser_tabs.contains_key(p))
            .collect()
    }

    /// 사이드패널 렌더(켜져 있을 때) + 액션 적용.
    pub(crate) fn show_browser(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        if !self.browser.open {
            return;
        }
        let remote_map = self.remote_compare_map();
        let can_upload = self.sftp.open && self.sftp.id.is_some();
        let lang = self.lang;
        let mut act: Option<BrowserAct> = None;
        egui::Panel::right("file_browser")
            .default_size(300.0)
            .size_range(180.0..=560.0) // 터미널을 가리지 않도록 상한 제한.
            .show(ui, |ui| {
                act = Some(render_browser_tab(ui, &mut self.browser, &remote_map, can_upload, lang, 0));
            });
        if let Some(a) = act {
            if let Some(r) = a.rect {
                self.drop_zones.push((crate::dnd::DropTarget::SidebarBrowser, r));
            }
            self.apply_browser_act(ctx, a);
        }
    }

    /// 수집된 브라우저 액션을 실행한다(사이드패널·탭 공용).
    pub(crate) fn apply_browser_act(&mut self, ctx: &egui::Context, mut a: BrowserAct) {
        let path = self.browser.path.clone();
        self.apply_clip_drag(&a, &path); // 탐색기 복사/붙여넣기/드래그-아웃.
        let mut nav = a.nav;
        // 우클릭 대상이 다중 선택에 속하면 선택 전체에 일괄 적용.
        let bulk = |name: &str, multi: &std::collections::HashSet<String>| -> Vec<String> {
            if multi.len() > 1 && multi.contains(name) { multi.iter().cloned().collect() } else { vec![name.to_string()] }
        };
        if let Some(name) = a.duplicate {
            for n in bulk(&name, &self.browser.multi) {
                crate::browserops::duplicate_in_dir(&path, &n); // 복제(충돌 시 번호).
            }
        }
        if let Some(name) = a.edit { self.edit_local_dispatch(name); } // 내장/외부 편집.
        if let Some(n) = a.edit_hex { let p = self.browser.path.join(n); self.open_local_as_hex(p); } // HEX 강제.
        if let Some(name) = a.preview { let p = self.browser.path.join(name); self.open_file_preview(p); } // E9.
        if let Some(pat) = a.content_search { self.content_search(pat); } // Find in Files(내용 검색).
        if a.dir_tree { self.open_dir_tree(); }
        if a.dir_stats { self.open_dir_stats(); }
        if let Some(name) = a.props {
            self.open_file_props(path.join(&name));
        }
        if let Some(name) = a.calc_size {
            let (files, bytes) = crate::browserops::dir_stats(&path.join(&name));
            self.notify = Some((format!("{name}: {} \u{00b7} {files}", crate::browserfs::human(bytes)), std::time::Instant::now()));
        }
        if let Some((folder, rn)) = a.dl_into {
            self.download_remote_into(&folder, rn.name, rn.is_dir); // 폴더 행 드롭.
        } else if let Some(rn) = a.drop_remote {
            self.download_remote_to_browser(rn.name, rn.is_dir); // 빈 영역 드롭.
        }
        self.browser.view = a.view;
        self.browser.scroll = false; // 이번 프레임 스크롤 요청 소비.
        let entries = crate::browserfs::read_entries(&path, self.browser.sort, self.browser.sort_desc, self.browser.show_hidden);
        let kb = crate::browserinput::keyboard_nav(ctx, a.over, &entries, &self.browser.filter, &path, &mut self.browser.selected, &mut self.browser.scroll);
        nav = nav.or(kb);
        // 표준 파일관리 단축키(브라우저 위에서만): F2=이름변경, Delete=삭제. 기존 핸들러로 위임.
        if a.over && self.browser.rename.is_none() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) { a.rename_start = a.rename_start.or_else(|| self.browser.selected.clone()); }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) { a.delete = a.delete.or_else(|| self.browser.selected.clone()); }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) { self.browser.multi = entries.iter().map(|r| r.name.clone()).collect(); } // Ctrl+A 전체선택.
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) { nav = nav.or_else(|| Some(path.clone())); } // F5=현재 폴더 새로고침(재탐색).
        }
        // 오름→내림(같은 키)→다음 키 오름… 순환 — 컬럼 헤더 없는 보기에서도 방향 전환 가능.
        if a.cycle_sort { if self.browser.sort_desc { self.browser.sort = self.browser.sort.next(); self.browser.sort_desc = false; } else { self.browser.sort_desc = true; } }
        if let Some(s) = a.set_sort {
            // 탐색기식: 같은 컬럼 재클릭=방향 토글, 다른 컬럼=오름차순.
            if self.browser.sort == s {
                self.browser.sort_desc = !self.browser.sort_desc;
            } else {
                self.browser.sort = s;
                self.browser.sort_desc = false;
            }
        }
        if let Some((s, ctrl, shift)) = a.select {
            let b = &mut self.browser;
            if ctrl {
                if !b.multi.remove(&s) {
                    b.multi.insert(s.clone()); // Ctrl=토글.
                }
            } else if shift {
                // Shift=anchor(selected)→s 범위(현재 정렬·필터 순서 기준).
                let names: Vec<&str> = entries
                    .iter()
                    .filter(|r| crate::browserfilter::name_matches(&b.filter.to_lowercase(), &r.name))
                    .map(|r| r.name.as_str())
                    .collect();
                let i1 = b.selected.as_deref().and_then(|x| names.iter().position(|n| *n == x));
                let i2 = names.iter().position(|n| *n == s);
                if let (Some(x), Some(y)) = (i1, i2) {
                    b.multi.clear();
                    for n in &names[x.min(y)..=x.max(y)] {
                        b.multi.insert((*n).to_string());
                    }
                }
            } else {
                b.multi.clear();
                b.multi.insert(s.clone()); // 일반 클릭=단일.
            }
            b.selected = Some(s);
        }
        if let Some(p) = nav {
            if p != self.browser.path {
                self.browser.back.push(self.browser.path.clone()); // 히스토리.
                self.browser.fwd.clear();
            }
            self.browser.selected = None;
            self.browser.multi.clear(); // 폴더 이동 시 다중 선택 해제.
            self.browser.path = p.clone();
            self.sync_after_local_nav(&p); // 동기 브라우징.
        }
        self.browser_history_nav(ctx, a.over); // 마우스 이전/다음(엄지).
        // OS 파일 드롭은 dispatch_dropped_files가 커서 위치로 라우팅(a.over는 드래그 중 부정확).
        if a.new_folder || a.new_file {
            // 기본 이름('새 폴더'/'새 파일.txt')으로 즉시 생성(중복 시 '(1)' 순번) 후, 목록에서 바로 인라인 이름변경.
            let (base, ext) = if a.new_file { (nabi_i18n::tr(self.lang, "browser.newfilename"), ".txt") } else { (nabi_i18n::tr(self.lang, "browser.newfoldername"), "") };
            let name = crate::sftppath::dedup_name(|n| path.join(n).exists(), base, ext);
            let ok = if a.new_file { std::fs::File::create(path.join(&name)).is_ok() } else { std::fs::create_dir(path.join(&name)).is_ok() };
            if ok {
                self.browser.rename = Some((name.clone(), name)); // 생성 직후 인라인 편집 시작.
                self.browser.rename_focus = true;
            }
        }
        if let (true, Some(d)) = (a.term_here, path.to_str()) {
            self.spawn_local_at(d.to_string());
        }
        if a.toggle_hidden {
            self.browser.show_hidden = !self.browser.show_hidden;
        }
        if let Some(name) = a.delete {
            for n in bulk(&name, &self.browser.multi) {
                let _ = trash::delete(path.join(&n)); // 휴지통(복구 가능).
            }
            self.browser.multi.clear();
        }
        if let Some(name) = a.rename_start {
            self.browser.rename = Some((name.clone(), name));
            self.browser.rename_focus = true; // 인라인 편집기 자동 포커스.
        }
        if a.rename_cancel {
            self.browser.rename = None;
        }
        if a.rename_ok {
            if let Some((old, new)) = self.browser.rename.take() {
                let new = new.trim();
                if crate::sftppath::valid_name(new) && new != old {
                    let _ = std::fs::rename(path.join(&old), path.join(new));
                }
            }
        }
        if let Some(name) = a.upload {
            for n in bulk(&name, &self.browser.multi.clone()) {
                self.upload_local_path(path.join(&n)); // SFTP 업로드(다중 선택 일괄).
            }
        }
        if a.cd_here {
            // 활성 탭이 실제 셸 pane이면 거기로 cd, 아니면(브라우저/에디터 탭 등) 새 터미널을 그 폴더에 연다.
            let term = self.focused_pane().filter(|p| self.orch.panes.read().ok().is_some_and(|m| m.contains_key(p)));
            if let Some(pane) = term {
                self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(format!("cd \"{}\"\r", path.display()).into_bytes()) });
            } else {
                self.spawn_local_at(path.display().to_string()); // 셸이 없으면 새 터미널을 그 폴더에.
            }
        }
    }
}
