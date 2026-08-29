//! 탭을 닫을 때 **무엇을 닫는 것인지** 가려내는 곳 — tabs.rs에서 갈라 왔다(줄 한도).
//!
//! 탭 하나가 터미널일 수도, 파일 브라우저·편집기·원격(SFTP)일 수도 있다. 닫는 방법이
//! 저마다 달라서, 무엇인지 가려내는 판단만 순수 함수로 떼어 시험할 수 있게 뒀다.

use nabi_types::PaneId;

/// 이 탭이 무엇인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabKind {
    Browser,
    Editor,
    /// 원격(SFTP/FTP) — 닫는 일 자체를 중앙이 하므로 여기서는 닫지 않는다.
    Remote,
    Terminal,
}

/// 있는지 없는지만 보고 종류를 정한다. 순서가 곧 우선순위다.
pub(crate) fn kind(browser: bool, editor: bool, remote: bool) -> TabKind {
    match (browser, editor, remote) {
        (true, _, _) => TabKind::Browser,
        (_, true, _) => TabKind::Editor,
        (_, _, true) => TabKind::Remote,
        _ => TabKind::Terminal,
    }
}

impl crate::tabs::TermTabViewer<'_> {
    /// pane이 원격(SFTP/FTP) 탭이면 그 호스트를, 아니면 None.
    pub(crate) fn remote_host(&self, pane: PaneId) -> Option<String> {
        match Some(pane) == self.sftp_pane {
            true => Some(self.sftp.host.clone()),
            false => self.sftp_bg.get(&pane).map(|p| p.host.clone()),
        }
    }

    /// 이 탭이 무엇인지 — 위 `kind`에 실제 상태를 물어 넘긴다.
    pub(crate) fn tab_kind(&self, tab: PaneId) -> TabKind {
        kind(
            self.browser_tabs.contains_key(&tab),
            self.editors.contains_key(&tab),
            Some(tab) == self.sftp_pane || self.sftp_bg.contains_key(&tab),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 브라우저가_가장_먼저다() {
        assert_eq!(kind(true, true, true), TabKind::Browser);
        assert_eq!(kind(false, true, true), TabKind::Editor);
        assert_eq!(kind(false, false, true), TabKind::Remote);
        assert_eq!(kind(false, false, false), TabKind::Terminal);
    }
}
