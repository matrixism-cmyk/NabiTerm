//! SFTP 패널 액션 처리 — render_sftp_tab가 모은 SftpAct를 실행한다(sftptab에서 분리).

use crate::app::NabiApp;
use crate::sftppath::join_path;
use crate::sftptab::SftpAct;
use nabi_i18n::tr;
use nabi_proto::Command;

impl NabiApp {
    /// SFTP 전송 완료 알림(H6): 토스트 + 큐가 비고 창이 비포커스면 작업표시줄 attention(E3 재사용).
    pub(crate) fn sftp_xfer_notify(&mut self, ctx: &egui::Context, ok: bool, name: &str, drained: bool) {
        let icon = if ok { "\u{2713}" } else { "\u{2717}" };
        self.notify = Some((format!("{icon} {name}"), std::time::Instant::now()));
        if drained && ctx.input(|i| !i.focused) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
    }

    /// render_sftp_tab가 모은 액션을 실행한다(명령 전송·상태 갱신).
    pub(crate) fn process_sftp_act(&mut self, mut a: SftpAct, _ctx: &egui::Context) {
        // 항목 작업(크기계산·터미널열기·재연결·편집·권한)은 sftpops로 분리(파일 크기 규율).
        self.process_sftp_ops(&mut a);
        // 원격 항목을 탐색기로 드래그-아웃(드롭 시 다운로드). DoDragDrop은 블로킹.
        if let (Some((name, size, is_dir)), Some(id)) = (&a.os_drag, self.sftp.id) {
            self.start_remote_drag(name, *size, *is_dir, id);
        }
        self.sftp_apply_select(a.select);
        if let (Some(path), Some(id)) = (a.go, self.sftp.id) {
            if path != self.sftp.path {
                self.sftp.hist_back.push(self.sftp.path.clone()); // 히스토리 기록.
                self.sftp.hist_fwd.clear();
            }
            self.remote_nav(id, path);
        }
        // 마우스 엄지 버튼: 뒤로/앞으로(스택 간 이동, 새 기록 없이).
        if a.back {
            if let (Some(p), Some(id)) = (self.sftp.hist_back.pop(), self.sftp.id) {
                self.sftp.hist_fwd.push(self.sftp.path.clone());
                self.remote_nav(id, p);
            }
        }
        if a.fwd {
            if let (Some(p), Some(id)) = (self.sftp.hist_fwd.pop(), self.sftp.id) {
                self.sftp.hist_back.push(self.sftp.path.clone());
                self.remote_nav(id, p);
            }
        }
        if let (Some((name, size)), Some(id)) = (a.dl, self.sftp.id) {
            // 다중 선택에 속하면 선택 파일 전체(FileZilla식, 폴더 제외), 아니면 단일.
            // 목적지는 사용자에게 물어본다(무단 다운로드 방지 — 취소 시 아무 동작 없음).
            // 여러 개를 고른 상태에서 그중 하나를 지정했으면 고른 것 전부가 대상이다.
            let batch = self.sftp.multi.len() > 1 && self.sftp.multi.contains(&name);
            let targets: Vec<(String, u64)> = if batch {
                let sel = self.sftp.entries.iter();
                let sel = sel.filter(|e| !e.is_dir && self.sftp.multi.contains(&e.name));
                sel.map(|e| (e.name.clone(), e.size)).collect()
            } else {
                vec![(name, size)]
            };
            self.download_prompt(id, targets);
        }
        if let Some(name) = a.del {
            self.sftp.pending_delete = Some(name);
        }
        if a.cancel_del {
            self.sftp.pending_delete = None;
        }
        if a.do_del {
            if let (Some(name), Some(id)) = (self.sftp.pending_delete.take(), self.sftp.id) {
                // 삭제 대상이 다중 선택에 속하면 선택 전체 일괄 삭제.
                let targets: Vec<String> =
                    if self.sftp.multi.len() > 1 && self.sftp.multi.contains(&name) {
                        self.sftp.multi.drain().collect()
                    } else {
                        vec![name]
                    };
                for n in &targets {
                    let remote = join_path(&self.sftp.path, n);
                    self.orch.send(Command::SftpRemove { id, path: remote });
                }
                self.orch.send(Command::SftpList {
                    id,
                    path: self.sftp.path.clone(),
                });
            }
        }
        if let Some(name) = a.rename {
            self.sftp.input = name.clone();
            self.sftp.rename_from = Some(name);
            self.sftp.rename_focus = true; // 인라인 편집기 자동 포커스.
        }
        if a.do_rename {
            let new = self.sftp.input.trim().to_string();
            if crate::sftppath::valid_name(&new) {
                if let (Some(from_name), Some(id)) = (self.sftp.rename_from.take(), self.sftp.id) {
                    let from = join_path(&self.sftp.path, &from_name);
                    let to = join_path(&self.sftp.path, &new);
                    self.orch.send(Command::SftpRename { id, from, to });
                    self.orch.send(Command::SftpList {
                        id,
                        path: self.sftp.path.clone(),
                    });
                }
                self.sftp.input.clear();
            } // 무효 이름이면 입력·모드 유지(사용자가 수정).
        }
        if a.new_folder || a.new_file {
            self.new_sftp_item(a.new_file); // 기본명 즉시 생성→인라인 이름변경(중복 시 '(1)').
        }
        if a.paste {
            self.sftp_paste_upload(); // OS 클립보드 파일 → 현재 폴더 업로드.
        }
        if let Some(name) = a.dldir {
            self.sftp_download_dir(name);
        }
        if a.dl_cur {
            self.sftp_download_current();
        }
        self.handle_sftp_bookmarks(a.bookmark_add, a.bookmark_go.clone(), a.bookmark_del.clone()); // 북마크 추가/이동/삭제.
        // OS 파일 드롭은 dispatch_dropped_files가 커서 위치로 라우팅(a.over는 드래그 중 부정확).
        if let Some(p) = a.upload_path {
            self.upload_local_path(std::path::PathBuf::from(p)); // 앱 내 DnD 업로드.
        }
        if let Some((folder, local)) = a.upload_into {
            self.upload_into(&folder, std::path::PathBuf::from(local)); // 폴더 행 드롭.
        }
        if a.cycle_sort {
            // 오름→내림(같은 키)→다음 키 오름… 순환 — 컬럼 헤더 없는 보기에서도 방향 전환 가능.
            if self.browser.sort_desc {
                self.browser.sort = self.browser.sort.next();
                self.browser.sort_desc = false;
            } else {
                self.browser.sort_desc = true;
            }
            crate::sftpentries::sort_sftp(&mut self.sftp.entries, self.browser.sort, self.browser.sort_desc);
        }
        if let Some(s) = a.set_sort {
            // 탐색기식: 같은 컬럼 재클릭=방향 토글, 다른 컬럼=오름차순.
            if self.browser.sort == s {
                self.browser.sort_desc = !self.browser.sort_desc;
            } else {
                self.browser.sort = s;
                self.browser.sort_desc = false;
            }
            crate::sftpentries::sort_sftp(&mut self.sftp.entries, self.browser.sort, self.browser.sort_desc);
        }
        if a.toggle_compare {
            self.compare_on = !self.compare_on;
        }
        if a.toggle_sync {
            self.toggle_sync_browse();
        }
        if a.search && !self.sftp.filter.trim().is_empty() {
            if let Some(id) = self.sftp.id {
                self.orch.send(Command::SftpSearch {
                    id,
                    root: self.sftp.path.clone(),
                    needle: self.sftp.filter.to_lowercase(),
                });
                self.sftp.status = tr(self.lang, "sftp.searching").to_string();
            }
        }
        if a.batch_toggle {
            self.sftp.batch_mode = !self.sftp.batch_mode;
        }
        if a.batch_apply {
            if let Some(id) = self.sftp.id {
                let find = self.sftp.batch_find.clone();
                let repl = self.sftp.batch_replace.clone();
                let path = self.sftp.path.clone();
                let mut idx = 1; // {n} 순번(바뀌는 파일마다 1씩 증가).
                for e in &self.sftp.entries {
                    if let Some(new) = crate::sftpentries::batch_new_name(&e.name, &find, &repl, idx) {
                        self.orch.send(Command::SftpRename {
                            id,
                            from: join_path(&path, &e.name),
                            to: join_path(&path, &new),
                        });
                        idx += 1;
                    }
                }
                self.orch.send(Command::SftpList { id, path });
            }
        }
        self.apply_queue_act(a.queue);
        if a.sync_up { self.sync_upload_diff(); }
        if a.sync_down { self.sync_download_diff(); }
        if a.sync_apply { self.apply_sync_pending(); }
        if a.sync_cancel { self.sftp.sync_pending = None; }
        if a.close {
            self.close_sftp();
        }
    }

    /// 활성 원격 연결을 닫고 그 탭을 제거한다. 배경 원격 패널이 있으면 활성으로 승격.
    pub(crate) fn close_sftp(&mut self) {
        if let Some(id) = self.sftp.id.take() {
            self.orch.send(Command::SftpClose { id });
        }
        if let Some(pane) = self.sftp_pane.take() {
            if let Some(loc) = self.dock.find_tab(&pane) {
                self.dock.remove_tab(loc);
            }
        }
        self.sftp = crate::sftppanel::SftpPanel::default();
        let next = self.sftp_bg.keys().next().copied();
        if let Some(pane) = next {
            if let Some(p) = self.sftp_bg.remove(&pane) {
                self.sftp = p;
                self.sftp_pane = Some(pane);
            }
        }
    }

    /// 연결 id로 해당 원격 패널(활성 또는 배경)을 찾는다.
    pub(crate) fn remote_panel_mut(
        &mut self,
        id: nabi_proto::SftpId,
    ) -> Option<&mut crate::sftppanel::SftpPanel> {
        if self.sftp.id == Some(id) {
            return Some(&mut self.sftp);
        }
        self.sftp_bg.values_mut().find(|p| p.id == Some(id))
    }

    /// 탭 ✕로 닫힌 원격 탭(활성 또는 배경) 정리.
    pub(crate) fn close_remote_tab(&mut self, pane: nabi_types::PaneId) {
        if Some(pane) == self.sftp_pane {
            self.close_sftp();
        } else if let Some(p) = self.sftp_bg.remove(&pane) {
            if let Some(id) = p.id {
                self.orch.send(Command::SftpClose { id });
            }
            if let Some(loc) = self.dock.find_tab(&pane) {
                self.dock.remove_tab(loc);
            }
        }
    }
}
