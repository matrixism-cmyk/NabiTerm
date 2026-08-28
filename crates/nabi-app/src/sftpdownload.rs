//! SFTP 다운로드 — 목적지 선택(rfd) + 파일/폴더/DnD 다운로드 경로. sftpxfer(큐)에서 분리.

use crate::app::NabiApp;
use crate::sftppath::join_path;
use nabi_i18n::tr;
use nabi_proto::Command;

impl NabiApp {
    /// 설정된 다운로드 폴더(존재하는 유효 경로일 때만).
    fn configured_download_dir(&self) -> Option<std::path::PathBuf> {
        let d = &self.config.terminal.download_dir;
        (!d.is_empty() && std::path::Path::new(d).is_dir()).then(|| std::path::PathBuf::from(d))
    }

    /// 다운로드 시작 기본 폴더 — 설정 폴더 > 로컬 브라우저 경로 > 홈. 목적지 대화상자의 시작 위치.
    fn default_download_dir(&self) -> std::path::PathBuf {
        self.configured_download_dir().unwrap_or_else(|| {
            if self.browser.open {
                self.browser.path.clone()
            } else {
                crate::browser::home_dir()
            }
        })
    }

    /// 묻지 않고 바로 저장할 폴더('매번 묻기' 꺼짐 + 유효한 설정 폴더가 있을 때만). 그 외 None → 대화상자.
    fn auto_download_dir(&self) -> Option<std::path::PathBuf> {
        if self.config.terminal.download_ask {
            return None;
        }
        self.configured_download_dir()
    }

    /// 폴더 다운로드 대상 폴더를 정한다: 자동 폴더가 있으면 그것, 없으면 사용자에게 묻는다.
    /// 사용자가 취소하면 None + 상태 표시(무단 다운로드 방지).
    fn resolve_download_folder(&mut self) -> Option<std::path::PathBuf> {
        if let Some(d) = self.auto_download_dir() {
            return Some(d);
        }
        match rfd::FileDialog::new().set_directory(self.default_download_dir()).pick_folder() {
            Some(d) => Some(d),
            None => {
                self.sftp.status = tr(self.lang, "sftp.canceled").to_string();
                None
            }
        }
    }

    /// 부분 파일 이어받기 오프셋. 백엔드가 `{local}.filepart`에 쓰므로 그 크기로 판정
    /// (완료 파일은 `local`로 rename됨). 부분 크기 < 원격 크기일 때만 오프셋, 아니면 0.
    fn resume_at(&self, local: &str, size: u64) -> u64 {
        let ls = std::fs::metadata(format!("{local}.filepart")).map(|m| m.len()).unwrap_or(0);
        if ls > 0 && ls < size { ls } else { 0 }
    }

    /// 대상 폴더에 이미 있는 파일이 있으면 덮어쓰기/기존 건너뜀/취소를 묻는다. 겹침 없으면 All.
    fn ask_overwrite(&self, dir: &std::path::Path, names: &[String]) -> Overwrite {
        let n = conflicting_names(names, |x| dir.join(x).exists()).len();
        if n == 0 {
            return Overwrite::All;
        }
        let body = tr(self.lang, "sftp.overwrite.body").replace("{n}", &n.to_string());
        match rfd::MessageDialog::new()
            .set_title(tr(self.lang, "sftp.overwrite.title"))
            .set_description(body)
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => Overwrite::All,
            rfd::MessageDialogResult::No => Overwrite::SkipExisting,
            _ => Overwrite::Cancel,
        }
    }

    /// 파일 목록을 대상 폴더로 다운로드한다(겹치면 덮어쓰기 확인). 취소 시 아무 것도 하지 않는다.
    fn download_files_into(&mut self, id: nabi_proto::SftpId, dir: &std::path::Path, targets: Vec<(String, u64)>) {
        // 서버가 준 이름을 그대로 `dir.join` 하면 **서버가 우리 디스크의 어디에 쓸지 고른다**
        // (배치 AE). `C:evil` 같은 이름은 러스트의 join 이 뿌리를 통째로 갈아치우고, `..` 이나
        // `a/b` 도 폴더 밖을 가리킨다. 목록의 한 항목은 경로가 아니라 **이름**이어야 한다.
        let (targets, unsafe_names): (Vec<_>, Vec<_>) =
            targets.into_iter().partition(|(n, _)| crate::syncplan::safe_name(n));
        if !unsafe_names.is_empty() {
            // 조용히 버리지 않는다 — 서버가 이상한 이름을 준 것은 사용자가 알아야 할 일이다.
            let shown: Vec<&str> = unsafe_names.iter().take(3).map(|(n, _)| n.as_str()).collect();
            self.notify = Some((
                format!("{} {}", tr(self.lang, "sftp.unsafename"), shown.join(", ")),
                std::time::Instant::now(),
            ));
        }
        if targets.is_empty() {
            return;
        }
        let names: Vec<String> = targets.iter().map(|(n, _)| n.clone()).collect();
        let skip_existing = match self.ask_overwrite(dir, &names) {
            Overwrite::Cancel => {
                self.sftp.status = tr(self.lang, "sftp.canceled").to_string();
                return;
            }
            Overwrite::SkipExisting => true,
            Overwrite::All => false,
        };
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        for (n, sz) in targets {
            let target = dir.join(&n);
            if skip_existing && target.exists() {
                continue;
            }
            let remote = join_path(&self.sftp.path, &n);
            let local = target.to_string_lossy().into_owned();
            let resume = self.resume_at(&local, sz);
            self.push_xfer(n, false, sz, |xfer| Command::SftpDownload { id, xfer, remote, local, resume });
        }
    }

    /// 파일 목록의 다운로드 목적지를 사용자에게 물어보고 전송을 시작한다(무단 다운로드 방지).
    /// 단일=다른 이름으로 저장(OS가 덮어쓰기 확인), 다중=대상 폴더 선택+겹침 확인. 취소 시 미다운로드.
    pub(crate) fn download_prompt(&mut self, id: nabi_proto::SftpId, targets: Vec<(String, u64)>) {
        if targets.is_empty() {
            return;
        }
        // '매번 묻기' 꺼짐 + 설정 폴더 있으면 묻지 않고 그 폴더로(가시적으로 설정된 목적지).
        if let Some(dir) = self.auto_download_dir() {
            self.download_files_into(id, &dir, targets);
            return;
        }
        let start = self.default_download_dir();
        if let [(name, size)] = targets.as_slice() {
            let Some(path) = rfd::FileDialog::new().set_file_name(name).set_directory(&start).save_file() else {
                self.sftp.status = tr(self.lang, "sftp.canceled").to_string();
                return;
            };
            let remote = join_path(&self.sftp.path, name);
            let local = path.to_string_lossy().into_owned();
            let resume = self.resume_at(&local, *size);
            self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
            self.push_xfer(name.clone(), false, *size, |xfer| Command::SftpDownload { id, xfer, remote, local, resume });
            return;
        }
        let Some(dir) = rfd::FileDialog::new().set_directory(&start).pick_folder() else {
            self.sftp.status = tr(self.lang, "sftp.canceled").to_string();
            return;
        };
        self.download_files_into(id, &dir, targets);
    }

    /// 현재 보고 있는 원격 디렉터리 전체를 재귀 다운로드한다(대상 폴더를 물어본다).
    pub(crate) fn sftp_download_current(&mut self) {
        let Some(id) = self.sftp.id else { return };
        let remote = self.sftp.path.clone();
        let base = crate::sftppath::remote_basename(&remote);
        let Some(dir) = self.resolve_download_folder() else { return };
        let local = dir.join(&base).to_string_lossy().into_owned();
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        self.push_xfer(base, false, 0, |xfer| Command::SftpDownloadDir { id, xfer, remote, local });
    }

    /// 원격 디렉터리를 재귀 다운로드한다(대상 폴더를 물어본다).
    pub(crate) fn sftp_download_dir(&mut self, name: String) {
        let Some(id) = self.sftp.id else { return };
        let remote = crate::sftppath::join_path(&self.sftp.path, &name);
        let Some(dir) = self.resolve_download_folder() else { return };
        let local = dir.join(&name).to_string_lossy().into_owned();
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        self.push_xfer(name, false, 0, |xfer| Command::SftpDownloadDir { id, xfer, remote, local });
    }

    /// 부분 파일이 남아 있으면 이어받기 오프셋(이름으로 원격 크기 조회). DnD 드롭 경로용.
    fn resume_offset(&self, local: &str, name: &str) -> u64 {
        let remote = self.sftp.entries.iter().find(|e| e.name == name).map(|e| e.size).unwrap_or(0);
        self.resume_at(local, remote)
    }

    /// 원격 항목을 로컬 브라우저의 하위 폴더로 다운로드한다(로컬 폴더 행에 드롭).
    pub(crate) fn download_remote_into(&mut self, subfolder: &str, name: String, is_dir: bool) {
        let Some(id) = self.sftp.id else { return };
        let remote = join_path(&self.sftp.path, &name);
        let local = self
            .browser
            .path
            .join(subfolder)
            .join(&name)
            .to_string_lossy()
            .into_owned();
        // 부분 파일 이어받기(폴더는 해당 없음). 분기는 클로저 안에서 — 밖에서 나누면 타입이 달라진다.
        let resume = if is_dir { 0 } else { self.resume_offset(&local, &name) };
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        self.push_xfer(format!("{subfolder}/{name}"), false, 0, move |xfer| {
            if is_dir {
                Command::SftpDownloadDir { id, xfer, remote, local }
            } else {
                Command::SftpDownload { id, xfer, remote, local, resume }
            }
        });
    }

    /// 원격 항목을 로컬 브라우저 폴더로 다운로드한다(SFTP→로컬 DnD).
    pub(crate) fn download_remote_to_browser(&mut self, name: String, is_dir: bool) {
        let Some(id) = self.sftp.id else { return };
        let remote = join_path(&self.sftp.path, &name);
        let local = self.browser.path.join(&name).to_string_lossy().into_owned();
        let resume = if is_dir { 0 } else { self.resume_offset(&local, &name) };
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        self.push_xfer(name, false, 0, move |xfer| {
            if is_dir {
                Command::SftpDownloadDir { id, xfer, remote, local }
            } else {
                Command::SftpDownload { id, xfer, remote, local, resume }
            }
        });
    }
}

/// 다중 파일 다운로드 시 대상 폴더에 이미 있는 파일 처리 방식.
#[derive(Clone, Copy, PartialEq)]
enum Overwrite {
    All,
    SkipExisting,
    Cancel,
}

/// `names` 중 `exists`가 참인(이미 있는) 이름만 골라낸다(순수 — 존재 판정은 호출자 위임, 테스트 가능).
fn conflicting_names(names: &[String], exists: impl Fn(&str) -> bool) -> Vec<String> {
    names.iter().filter(|n| exists(n)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::conflicting_names;

    #[test]
    fn conflicts_only_existing() {
        let names = vec!["a.txt".to_string(), "b.txt".to_string(), "c.txt".to_string()];
        let existing = ["a.txt", "c.txt"];
        let got = conflicting_names(&names, |n| existing.contains(&n));
        assert_eq!(got, vec!["a.txt".to_string(), "c.txt".to_string()]);
        // 겹침 없으면 빈 목록.
        assert!(conflicting_names(&names, |_| false).is_empty());
    }
}
