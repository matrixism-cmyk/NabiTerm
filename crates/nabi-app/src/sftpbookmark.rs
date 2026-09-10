//! 즐겨찾기 넣고 빼기 — 원격(SFTP)과 로컬 탐색기 **둘 다**.
//!
//! 넣고 빼는 규칙은 두 창이 같다: 빈 경로는 안 넣고, 이미 있으면 두 번 안 넣고,
//! 바뀌면 곧바로 저장한다. 그래서 규칙을 한 곳에 두고 **목록만 갈아 끼운다** —
//! 두 벌로 두면 언젠가 한쪽만 고쳐져서, 원격에서는 되는데 로컬에서는 안 되는
//! (또는 그 반대인) 상태가 된다. 그런 어긋남은 조용하고 오래간다.

use crate::app::NabiApp;
use nabi_proto::Command;

/// 목록에 넣고 빼는 규칙. 바뀌었으면 `true`(부르는 쪽이 저장한다).
///
/// 순수 함수라 시험할 수 있다 — 화면도 설정 파일도 건드리지 않는다.
#[must_use = "바뀐 것을 저장하지 않으면 다음에 켤 때 사라진다"]
pub(crate) fn edit(list: &mut Vec<String>, add: Option<String>, del: Option<String>) -> bool {
    let mut changed = false;
    if let Some(p) = add.filter(|p| !p.trim().is_empty()) {
        if !list.contains(&p) {
            list.push(p);
            changed = true;
        }
    }
    if let Some(p) = del {
        let before = list.len();
        list.retain(|x| x != &p);
        changed |= list.len() != before;
    }
    changed
}

impl NabiApp {
    /// 원격 북마크 추가(현재 경로)/이동(go)/삭제(del)를 처리하고 config에 저장한다.
    pub(crate) fn handle_sftp_bookmarks(&mut self, add: bool, go: Option<String>, del: Option<String>) {
        let here = add.then(|| self.sftp.path.clone());
        if edit(&mut self.config.terminal.sftp_bookmarks, here, del) {
            self.save_config();
        }
        if let Some(p) = go {
            if let Some(id) = self.sftp.id {
                self.orch.send(Command::SftpList { id, path: p });
            }
        }
    }

    /// 로컬 탐색기 북마크 추가/삭제. **이동은 여기서 하지 않는다** —
    /// 로컬은 `a.nav` 하나로 옮겨 가므로 메뉴가 그 자리에서 끝낸다.
    pub(crate) fn handle_browser_bookmarks(&mut self, add: Option<String>, del: Option<String>) {
        if edit(&mut self.config.terminal.browser_bookmarks, add, del) {
            self.save_config();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::edit;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 넣고_빼면_제자리다() {
        let mut l = v(&["/a"]);
        assert!(edit(&mut l, Some("/b".into()), None));
        assert_eq!(l, v(&["/a", "/b"]));
        assert!(edit(&mut l, None, Some("/b".into())));
        assert_eq!(l, v(&["/a"]), "뺀 자리가 남으면 목록이 계속 자란다");
    }

    /// 같은 곳을 두 번 꽂아도 한 줄이다 — 목록이 같은 이름으로 채워지면 못 쓴다.
    #[test]
    fn 같은_곳은_두_번_안_들어간다() {
        let mut l = v(&["/a"]);
        assert!(!edit(&mut l, Some("/a".into()), None), "안 바뀌었는데 바뀌었다고 했다");
        assert_eq!(l.len(), 1);
    }

    /// 빈 경로는 안 넣는다 — 아직 아무 데도 안 간 상태에서 눌렀을 때다.
    #[test]
    fn 빈_경로는_안_넣는다() {
        let mut l: Vec<String> = Vec::new();
        assert!(!edit(&mut l, Some("   ".into()), None));
        assert!(l.is_empty());
    }

    /// 없는 것을 빼면 "안 바뀌었다"고 답한다 — 그래야 쓸데없이 저장하지 않는다.
    #[test]
    fn 없는_것을_빼면_안_바뀐다() {
        let mut l = v(&["/a"]);
        assert!(!edit(&mut l, None, Some("/없다".into())));
        assert_eq!(l, v(&["/a"]));
    }
}
