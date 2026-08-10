//! 워크스페이스 저장(workspace.rs 라인 한도 분리) — 열린 탭 출처·스크롤백 백로그·레이아웃 사이드카 저장.

use crate::app::NabiApp;
use nabi_session::{SavedSession, SessionKind, SessionTree};

impl NabiApp {
    pub(crate) fn save_workspace(&self) {
        let ordered: Vec<nabi_types::PaneId> = self
            .dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .filter(|p| Some(*p) != self.sftp_pane && !self.sftp_bg.contains_key(p)) // 원격 탭 제외.
            .collect();
        let pairs: Vec<(SavedSession, nabi_types::PaneId)> = ordered
            .iter()
            .filter_map(|p| {
                self.pane_origins.get(p).cloned().map(|kind| {
                    // 로컬 셸은 마지막 cwd + 종료 직전 실행 중이던 명령(설정 시)을 저장.
                    let (cwd, on_connect) = match kind {
                        SessionKind::Local { .. } => self.saved_local_state(*p),
                        SessionKind::Ssh { .. } => (None, self.saved_ssh_ai_command(*p)),
                    };
                    (
                        SavedSession {
                            name: "workspace".to_string(),
                            folder: None,
                            kind,
                            on_connect,
                            cwd,
                            is_ftp: false,
                            open_sftp: false,
                        },
                        *p,
                    )
                })
            })
            .collect();
        let term_ordered: Vec<nabi_types::PaneId> = ordered
            .iter()
            .copied()
            .filter(|p| !self.browser_tabs.contains_key(p))
            .collect();
        let sessions: Vec<SavedSession> = pairs.iter().map(|(s, _)| s.clone()).collect();
        let count = sessions.len();
        // 로컬 셸 스크롤백 백로그 저장(마지막 2000줄) — 재시작 시 화면 복구용.
        if let Some(dir) = self.workspace_path.parent() {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().starts_with("scroll_") {
                        let _ = std::fs::remove_file(e.path()); // 이전 백로그 정리.
                    }
                }
            }
            for (i, (s, p)) in pairs.iter().enumerate() {
                if !matches!(s.kind, SessionKind::Local { .. }) {
                    continue;
                }
                if let Some(v) = self.orch.panes.read().ok().and_then(|m| m.get(p).cloned()) {
                    if let Ok(md) = v.model.lock() {
                        let txt = md.dump_text(2000);
                        if !txt.is_empty() {
                            let _ = std::fs::write(dir.join(format!("scroll_{i}.txt")), txt);
                        }
                    }
                }
            }
        }
        let tree = SessionTree { sessions };
        let _ = nabi_session::save_tree(&self.workspace_path, &tree);
        // 분할 레이아웃 + pane 사이드카(글꼴·이름·색) 저장(worklayout.rs).
        self.save_layout_sidecars(&ordered, &term_ordered, count);
        self.save_browser_tabs(); // 브라우저 탭 상태(경로·보기·정렬)도 저장.
        self.save_floating(); // 분리 OS 창 위치·크기·출처(P10).
    }
}
