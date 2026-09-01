//! `/etc/passwd`·`/etc/group` 을 받아 오는 쪽 — 세는 일은 [`crate::passwdmap`] 이 한다.
//!
//! 미리보기 명령(`SftpPreview`)을 그대로 쓴다. 상한을 걸고 앞부분만 받는 명령이라
//! 이 목적에 정확히 맞고, 새 명령을 만들지 않으니 서버 쪽에 늘어나는 것도 없다.
//!
//! 같은 명령을 이미 셋이 나눠 쓴다 — 미리보기 창, 내용 찾기, 그리고 여기. 응답은
//! 경로로 갈라 가져간다(`ids_on_preview` 가 먼저 보고, 아니면 다음 주인에게 넘긴다).

use crate::app::NabiApp;
use crate::passwdmap::{GROUP, MAX, PASSWD};

impl NabiApp {
    /// 소유자·그룹 열이 켜져 있으면, 이번 접속에서 한 번만 두 파일을 청한다.
    ///
    /// **켜지 않은 사람의 서버에서는 아무것도 읽지 않는다.** 남의 계정 목록을 읽는 일은
    /// 필요할 때만 해야 하고, 서버 감사 기록에도 그렇게 남는 편이 낫다.
    pub(crate) fn fetch_ids_if_needed(&mut self) {
        if self.sftp.ids_asked {
            return;
        }
        let want = crate::colset::on(&self.sftp.cols, "owner") || crate::colset::on(&self.sftp.cols, "group");
        let Some(id) = self.sftp.id.filter(|_| want) else { return };
        self.sftp.ids_asked = true;
        for path in [PASSWD, GROUP] {
            self.orch.send(nabi_proto::Command::SftpPreview { id, path: path.to_string(), max: MAX });
        }
    }

    /// 이 미리보기 응답이 우리가 청한 두 파일 중 하나면 받아 챙긴다. 가져갔으면 true.
    ///
    /// 못 읽어도 조용히 넘어간다 — 권한이 없거나(읽기 금지), 파일이 없거나(윈도우 서버),
    /// LDAP 라서 로컬 파일에 없을 수 있다. 그때는 번호가 그대로 보인다.
    pub(crate) fn ids_on_preview(&mut self, path: &str, data: &[u8]) -> bool {
        let map = match path {
            PASSWD => &mut self.sftp.users,
            GROUP => &mut self.sftp.groups,
            _ => return false,
        };
        // 계정 파일에 UTF-8 이 아닌 바이트가 섞여 있어도 이름 대부분은 ASCII 라 살릴 수 있다.
        *map = crate::passwdmap::parse_ids(&String::from_utf8_lossy(data));
        true
    }

    /// 접속이 바뀌었다 — 다른 서버의 계정 목록을 그대로 쓰면 엉뚱한 이름이 나온다.
    pub(crate) fn forget_ids(&mut self) {
        self.sftp.users.clear();
        self.sftp.groups.clear();
        self.sftp.ids_asked = false;
    }
}
