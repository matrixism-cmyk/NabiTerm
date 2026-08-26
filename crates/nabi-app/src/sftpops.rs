//! SFTP 항목 작업 처리(크기 계산·여기서 터미널·재연결·편집·권한) — process_sftp_act에서 분리.
//! 시작 시점에 호출되며, 처리한 필드는 take()로 소비한다(이후 나머지 액션 처리에 영향 없음).

use crate::app::NabiApp;
use crate::sftppath::join_path;
use crate::sftptab::SftpAct;
use nabi_i18n::tr;
use nabi_proto::Command;

impl NabiApp {
    pub(crate) fn process_sftp_ops(&mut self, a: &mut SftpAct) {
        // 서버 안에서 사본 만들기. 같은 폴더라 이름이 겹칠 수밖에 없으므로 사본 이름을
        // 붙인다 — 로컬 브라우저의 복제와 **같은 규칙**을 쓴다(두 곳이 다르게 지으면
        // 사용자가 규칙을 두 번 배워야 한다).
        // 고른 파일들에 명령을 건다. 여러 개 골랐으면 한 번에 넘긴다.
        if let Some((name, op)) = a.run_cmd.take() {
            let mut names = self.sftp.multi.iter().cloned().collect::<Vec<_>>();
            if names.is_empty() {
                names.push(name);
            }
            self.prepare_remote_cmd(op, names);
        }
        if a.compare {
            self.compare_remote_selected();
        }
        if let Some((name, size, dir)) = a.copy_here.take() {
            self.start_remote_copy(&name, size, dir);
        }
        if let Some(name) = a.dirsize.take() {
            if let Some(id) = self.sftp.id {
                self.orch.send(Command::SftpDirSize { id, path: join_path(&self.sftp.path, &name) });
                self.sftp.status = tr(self.lang, "sftp.calcsize").to_string();
            }
        }
        if let Some(name) = a.open_term.take() {
            // 빠른연결을 이 원격 폴더 정보로 프리필하고 연다(비밀번호는 사용자가 입력).
            let path = join_path(&self.sftp.path, &name);
            let (h, u, p) = (self.sftp.conn_host.clone(), self.sftp.conn_user.clone(), self.sftp.conn_port.clone());
            if !h.is_empty() {
                let qc = &mut self.quick_connect;
                qc.host = h;
                qc.user = u;
                qc.port = if p.is_empty() { "22".into() } else { p };
                qc.password.clear();
                qc.on_connect = format!("cd '{path}'");
                qc.open = true;
            }
        }
        if a.reconnect {
            // 끊긴 SFTP 세션 재연결: 접속 정보 프리필(비밀번호 재입력 후 SFTP로 연결).
            let (h, u, p) = (self.sftp.conn_host.clone(), self.sftp.conn_user.clone(), self.sftp.conn_port.clone());
            if !h.is_empty() {
                let qc = &mut self.quick_connect;
                qc.host = h;
                qc.user = u;
                qc.port = if p.is_empty() { "22".into() } else { p };
                qc.password.clear();
                qc.with_sftp = true;
                qc.open = true;
            }
        }
        if let Some(name) = a.preview.take() {
            self.request_preview(join_path(&self.sftp.path, &name));
        }
        if let Some(name) = a.edit.take() {
            self.edit_remote_dispatch(name); // 내장(기본)/외부 에디터.
        }
        if let Some((name, mode)) = a.chmod.take() {
            if let Some(id) = self.sftp.id {
                // 대상이 다중 선택에 속하면 선택 전체에 같은 권한 적용(FileZilla식).
                let targets: Vec<String> = if self.sftp.multi.len() > 1 && self.sftp.multi.contains(&name) { self.sftp.multi.iter().cloned().collect() } else { vec![name] };
                for n in &targets {
                    self.orch.send(Command::SftpChmod { id, path: join_path(&self.sftp.path, n), mode });
                }
                self.orch.send(Command::SftpList { id, path: self.sftp.path.clone() });
            }
        }
        if let Some((name, mode)) = a.chmod_rec.take() {
            if let Some(id) = self.sftp.id {
                self.orch.send(Command::SftpChmodRecursive { id, path: join_path(&self.sftp.path, &name), mode });
                self.orch.send(Command::SftpList { id, path: self.sftp.path.clone() });
            }
        }
    }

    /// 같은 폴더에 사본을 만든다(서버 안에서). 큐에 넣어 진행률·취소를 그대로 쓴다.
    fn start_remote_copy(&mut self, name: &str, size: u64, dir: bool) {
        let Some(id) = self.sftp.id else { return };
        // 지금 목록에 있는 이름들을 피해 사본 이름을 짓는다.
        let taken: Vec<String> = self.sftp.entries.iter().map(|e| e.name.clone()).collect();
        let word = nabi_i18n::tr(self.lang, "browser.copyword");
        let new = (1..1000)
            .map(|n| crate::browserops::copy_suffixed(name, n, word))
            .find(|c| !taken.contains(c));
        let Some(new) = new else {
            self.notify = Some((
                nabi_i18n::tr(self.lang, "sftp.copy.toomany").to_string(),
                std::time::Instant::now(),
            ));
            return;
        };
        let from = crate::sftppath::join_path(&self.sftp.path, name);
        let to = crate::sftppath::join_path(&self.sftp.path, &new);
        self.push_xfer(new, false, size, move |xfer| nabi_proto::Command::SftpCopy {
            id,
            xfer,
            from,
            to,
            dir,
        });
    }
}
