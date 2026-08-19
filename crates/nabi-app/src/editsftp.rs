//! 원격 파일 편집(MobaXterm/FileZilla식): 원격 파일을 임시로 내려받아 OS 기본
//! 편집기로 열고, 저장(임시파일 수정)이 감지되면 원격으로 자동 재업로드한다.
//!
//! 별도 연결 없이 기존 SFTP 다운로드/업로드 명령을 재사용한다. 다운로드 완료는
//! SftpTransferDone(로컬 경로=임시파일)로, 저장은 매 프레임 임시파일 mtime 폴링으로 감지.

use crate::app::NabiApp;
use nabi_proto::Command;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

/// 편집 중인 원격 파일 1건.
pub(crate) struct EditWatch {
    /// 이 파일을 내려받은 SFTP 세션. 업로드는 반드시 같은 세션으로 보낸다
    /// (패널이 다른 호스트로 바뀌어도 엉뚱한 서버를 덮어쓰지 않게).
    pub id: nabi_proto::SftpId,
    /// 원격 경로.
    pub remote: String,
    /// 로컬 임시 파일.
    pub temp: PathBuf,
    /// 마지막으로 본 임시파일 수정 시각(unix 초).
    pub mtime: u64,
    /// 다운로드 완료 + 편집기 오픈됨.
    pub started: bool,
}

/// 파일 수정 시각(unix 초, 없으면 0).
fn file_mtime(p: &Path) -> u64 {
    std::fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl NabiApp {
    /// 새 원격 폴더/파일을 기본 이름으로 즉시 생성(중복 시 '(1)' 순번)하고, 목록 갱신 후 인라인 이름변경을 시작한다.
    pub(crate) fn new_sftp_item(&mut self, is_file: bool) {
        let Some(id) = self.sftp.id else { return };
        let (base, ext) = if is_file { (nabi_i18n::tr(self.lang, "browser.newfilename"), ".txt") } else { (nabi_i18n::tr(self.lang, "browser.newfoldername"), "") };
        let name = crate::sftppath::dedup_name(|n| self.sftp.entries.iter().any(|e| e.name == n), base, ext);
        let path = crate::sftppath::join_path(&self.sftp.path, &name);
        self.orch.send(if is_file { Command::SftpTouch { id, path } } else { Command::SftpMkdir { id, path } });
        self.orch.send(Command::SftpList { id, path: self.sftp.path.clone() });
        self.sftp.input = name.clone(); // 인라인 편집기 버퍼 시드.
        self.sftp.rename_from = Some(name); // 목록 갱신되면 그 항목을 인라인 편집.
        self.sftp.rename_focus = true;
    }

    /// 원격 파일 편집 시작 — 임시 파일로 다운로드(완료 시 편집기 오픈은 open_edit).
    pub(crate) fn edit_remote(&mut self, name: String) {
        let Some(id) = self.sftp.id else { return };
        let remote = crate::sftppath::join_path(&self.sftp.path, &name);
        let temp = std::env::temp_dir().join(format!("nabi-edit-{name}"));
        let local = temp.to_string_lossy().into_owned();
        self.orch.send(Command::SftpDownload {
            id,
            xfer: crate::sftpxfer::XFER_NONE,
            remote: remote.clone(),
            local,
            resume: 0, // 편집은 항상 새로(덮어쓰기).
        });
        self.sftp.status = nabi_i18n::tr(self.lang, "sftp.downloading").to_string();
        // 중복 방지: 같은 원격을 이미 편집 중이면 갱신만.
        self.edits.retain(|e| !(e.id == id && e.remote == remote));
        self.edits.push(EditWatch {
            id,
            remote,
            temp,
            mtime: 0,
            started: false,
        });
    }

    /// 다운로드 완료된 편집 임시파일(local 경로)을 OS 기본 편집기로 연다.
    pub(crate) fn open_edit(&mut self, local: &str) {
        if let Some(e) = self
            .edits
            .iter_mut()
            .find(|e| !e.started && e.temp.to_string_lossy() == local)
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", local])
                .spawn();
            e.started = true;
            e.mtime = file_mtime(&e.temp);
        }
    }

    /// 정렬/보기/숨김 설정이 바뀌었으면 config에 저장(재시작 후 유지).
    pub(crate) fn persist_view_prefs(&mut self) {
        let s = self.browser.sort.to_u8();
        let h = self.browser.show_hidden;
        let v = if self.sftp.open {
            self.sftp.view_mode.to_u8()
        } else {
            self.config.terminal.sftp_view
        };
        let d = self.browser.sort_desc;
        let bv = self.browser.view.to_u8();
        if self.config.terminal.browser_sort != s
            || self.config.terminal.browser_sort_desc != d
            || self.config.terminal.browser_show_hidden != h
            || self.config.terminal.sftp_view != v
            || self.config.terminal.browser_view != bv
        {
            self.config.terminal.browser_sort = s;
            self.config.terminal.browser_sort_desc = d;
            self.config.terminal.browser_show_hidden = h;
            self.config.terminal.sftp_view = v;
            self.config.terminal.browser_view = bv;
            let _ = nabi_config::save(&self.config_path, &self.config);
        }
    }

    /// 로컬 브라우저 비교 색칠용 원격 항목 맵(compare 모드 아니면 빈 맵).
    pub(crate) fn remote_compare_map(&self) -> std::collections::HashMap<String, (bool, u64)> {
        if self.compare_on && self.sftp.open {
            self.sftp
                .entries
                .iter()
                .map(|e| (e.name.clone(), (e.is_dir, e.size)))
                .collect()
        } else {
            std::collections::HashMap::new()
        }
    }

    /// 디렉터리 비교 모드일 때 로컬 폴더 맵을 만들어 SFTP 비교 색칠에 쓴다.
    ///
    /// **스로틀**(성능 리뷰 2026-08-19): 매 프레임 호출되는데 내용은 동기 `read_dir`+stat다.
    /// 비교 색칠은 실시간일 필요가 없으므로 0.5초마다만 다시 읽는다(경로·정렬이 바뀌면
    /// 다음 틱에 반영). 60fps 디스크 스캔을 없애 SFTP+브라우저 동시 사용 시 체감이 달라진다.
    pub(crate) fn update_compare_map(&mut self) {
        const REFRESH: std::time::Duration = std::time::Duration::from_millis(500);
        if self.compare_on && self.sftp.open && self.browser.open {
            if self.sftp.compare.is_some() && self.compare_at.is_some_and(|t| t.elapsed() < REFRESH) {
                return; // 아직 신선하다 — 디스크를 다시 읽지 않는다.
            }
            self.compare_at = Some(std::time::Instant::now());
            let map = crate::browserfs::read_entries(
                &self.browser.path,
                self.browser.sort,
                self.browser.sort_desc,
                self.browser.show_hidden,
            )
                .into_iter()
                .map(|r| (r.name, (r.is_dir, r.size)))
                .collect();
            self.sftp.compare = Some(map);
        } else {
            self.sftp.compare = None;
        }
    }

    /// 디렉터리 동기화 업로드(WinSCP식) — 로컬 브라우저 폴더에서 원격에 없거나 크기가 다른 파일만
    /// 현재 원격 폴더로 업로드한다(비재귀, 파일만). 로컬 브라우저가 열려 있어야 한다.
    pub(crate) fn sync_upload_diff(&mut self) {
        if self.sftp.id.is_none() || !self.browser.open {
            self.notify = Some((nabi_i18n::tr(self.lang, "sftp.syncneed").to_string(), Instant::now()));
            return;
        }
        let local = crate::browserfs::read_entries(&self.browser.path, self.browser.sort, self.browser.sort_desc, self.browser.show_hidden);
        let remote: std::collections::HashMap<&str, u64> = self.sftp.entries.iter().filter(|e| !e.is_dir).map(|e| (e.name.as_str(), e.size)).collect();
        let todo: Vec<String> = local.iter().filter(|r| !r.is_dir && remote.get(r.name.as_str()) != Some(&r.size)).map(|r| r.name.clone()).collect();
        if todo.is_empty() {
            self.notify = Some((format!("{} 0", nabi_i18n::tr(self.lang, "sftp.synced")), Instant::now()));
        } else {
            self.sftp.sync_pending = Some((true, todo)); // 적용 전 미리보기(확인 행).
        }
    }

    /// 디렉터리 동기화 다운로드(WinSCP식) — 원격에서 로컬에 없거나 크기가 다른 파일만 로컬 브라우저
    /// 폴더로 내려받는다(비재귀, 파일만). v225 업로드의 짝.
    pub(crate) fn sync_download_diff(&mut self) {
        if self.sftp.id.is_none() || !self.browser.open {
            self.notify = Some((nabi_i18n::tr(self.lang, "sftp.syncneed").to_string(), Instant::now()));
            return;
        }
        let local: std::collections::HashMap<String, u64> = crate::browserfs::read_entries(&self.browser.path, self.browser.sort, self.browser.sort_desc, self.browser.show_hidden)
            .into_iter().filter(|r| !r.is_dir).map(|r| (r.name, r.size)).collect();
        let todo: Vec<String> = self.sftp.entries.iter().filter(|e| !e.is_dir && local.get(&e.name) != Some(&e.size)).map(|e| e.name.clone()).collect();
        if todo.is_empty() {
            self.notify = Some((format!("{} 0", nabi_i18n::tr(self.lang, "sftp.synceddown")), Instant::now()));
        } else {
            self.sftp.sync_pending = Some((false, todo)); // 적용 전 미리보기(확인 행).
        }
    }

    /// 동기화 미리보기를 적용한다(업로드/다운로드) — 확인 행의 '적용'.
    pub(crate) fn apply_sync_pending(&mut self) {
        let Some((up, files)) = self.sftp.sync_pending.take() else { return };
        for name in &files {
            if up {
                self.upload_local_path(self.browser.path.join(name));
            } else {
                self.download_remote_to_browser(name.clone(), false);
            }
        }
        let key = if up { "sftp.synced" } else { "sftp.synceddown" };
        self.notify = Some((format!("{} {}", nabi_i18n::tr(self.lang, key), files.len()), Instant::now()));
    }

    /// 매 프레임: 편집 임시파일이 저장(수정)되었으면 원격으로 재업로드한다.
    pub(crate) fn poll_edits(&mut self) {
        let mut ups: Vec<(nabi_proto::SftpId, String, String)> = Vec::new();
        for e in self.edits.iter_mut().filter(|e| e.started) {
            let m = file_mtime(&e.temp);
            if m > e.mtime {
                e.mtime = m;
                // 내려받은 그 세션으로 올린다(현재 선택된 패널이 아니라).
                ups.push((e.id, e.temp.to_string_lossy().into_owned(), e.remote.clone()));
            }
        }
        for (id, local, remote) in ups {
            self.orch.send(Command::SftpUpload {
                id,
                xfer: crate::sftpxfer::XFER_NONE,
                local,
                remote: remote.clone(),
            });
            self.notify = Some((format!("\u{2191} {remote}"), Instant::now()));
        }
    }
}
