//! 세션 워크스페이스: 열린 탭(로컬/SSH)의 출처를 파일로 저장하고 복원한다.
//!
//! 분할 트리는 보존하지 않고 "열린 탭 집합"을 다시 연다(로컬은 즉시, SSH는 볼트/프리필).

use crate::app::NabiApp;
use egui_dock::DockState;
use nabi_proto::ShellKind;
use nabi_session::SessionKind;

/// 미완료 스폰 1건의 정보 — PaneSpawned.seq로 조회해 출처/명령/백로그/레이아웃 위치를 적용한다.
pub(crate) struct PendingSpawn {
    pub origin: SessionKind,
    pub oncmd: Option<String>,
    pub backlog: Option<Vec<u8>>,     // 로컬 복원 스크롤백(표시 전용)
    pub ordinal: Option<usize>,       // 워크스페이스 레이아웃 매핑용 ordinal(복원만)
    pub float_geom: Option<[f32; 4]>, // Some이면 도크가 아닌 분리 OS 창으로 복원([x,y,w,h], P10)
}

/// 복원 중 비동기 스폰을 모았다가 분할 레이아웃을 재구성하는 상태.
pub(crate) struct PendingLayout {
    pub saved: DockState<usize>,
    /// ordinal → 도착한 pane. 도착 순서와 무관하게 ordinal로 매핑(SSH/로컬 완료 순서 뒤바뀜 방지).
    pub arrived: std::collections::HashMap<usize, nabi_types::PaneId>,
    pub expected: usize,
    /// 순서(ordinal)별 pane 글꼴 크기(복원 시 적용). 비면 전역.
    pub fonts: Vec<f32>,
    pub names: Vec<String>,
    pub colors: Vec<[u8; 4]>,
    /// 먼저 생성된 브라우저 탭 pane들(레이아웃 서수 1000+i 매핑).
    pub browser_panes: Vec<nabi_types::PaneId>,
    /// 먼저 재연결한 원격(SFTP/FTP) 탭 pane들(레이아웃 서수 2000+i 매핑).
    pub sftp_panes: Vec<nabi_types::PaneId>,
}

/// OSC 7 file URI 디코드가 "/C:/.." 형태면 앞 슬래시를 제거(Windows 경로화).
pub(crate) fn strip_uri_slash(raw: &str) -> String {
    if raw.len() >= 3 && raw.starts_with('/') && raw.as_bytes()[2] == b':' {
        raw[1..].to_string()
    } else {
        raw.to_string()
    }
}

/// 설정 문자열 → ShellKind(미지정/미상은 Windows PowerShell).
pub(crate) fn shell_from_str(s: &str) -> ShellKind {
    match s.to_ascii_lowercase().as_str() {
        "pwsh" => ShellKind::Pwsh,
        "cmd" => ShellKind::Cmd,
        "wsl" => ShellKind::Wsl { distro: None },
        "gitbash" => ShellKind::GitBash,
        _ => ShellKind::WindowsPowerShell,
    }
}

/// ShellKind → config 문자열(워크스페이스 저장용).
pub(crate) fn shell_to_str(s: &ShellKind) -> String {
    match s {
        ShellKind::Pwsh => "pwsh",
        ShellKind::Cmd => "cmd",
        ShellKind::Wsl { .. } => "wsl",
        ShellKind::GitBash => "gitbash",
        _ => "powershell",
    }
    .to_string()
}

impl NabiApp {
    /// 새 pane을 추가한다(탭이 없으면 새 DockState, 있으면 포커스 leaf에 탭으로).
    pub(crate) fn add_pane(&mut self, pane: nabi_types::PaneId) {
        if self.dock.iter_all_tabs().next().is_none() {
            self.dock = egui_dock::DockState::new(vec![pane]);
        } else {
            self.dock.push_to_focused_leaf(pane);
        }
    }

    /// 새 pane을 포커스된 leaf의 분할로 배치한다(right=오른쪽, false=아래). 불가 시 탭.
    pub(crate) fn split_pane(&mut self, pane: nabi_types::PaneId, right: bool) {
        let main = egui_dock::SurfaceIndex::main();
        // 포커스 리프 기준 분할. 포커스가 없으면(사이드바·메뉴 조작 후 흔함) 메인 트리 첫 탭 기준으로
        // 폴백해 분할이 묵살되지 않게 한다(탭이 전혀 없을 때만 일반 추가).
        // 0.19: focused_leaf/find_tab이 튜플 대신 NodePath/TabPath를 돌려준다.
        let target = self
            .dock
            .focused_leaf()
            .filter(|p| p.surface == main)
            .map(|p| p.node)
            .or_else(|| {
                let first = self.dock.iter_all_tabs().next().map(|(_, t)| *t)?;
                self.dock
                    .find_tab(&first)
                    .filter(|loc| loc.surface == main)
                    .map(|loc| loc.node)
            });
        if let Some(node) = target {
            let tree = self.dock.main_surface_mut();
            if right {
                tree.split_right(node, 0.5, vec![pane]);
            } else {
                tree.split_below(node, 0.5, vec![pane]);
            }
        } else {
            self.add_pane(pane);
        }
    }

    /// 새 로컬 셸의 시작 디렉터리: 포커스된 로컬 pane의 cwd(실재 디렉터리만).
    pub(crate) fn spawn_cwd(&mut self) -> Option<String> {
        let p = self.focused_pane()?;
        if matches!(self.pane_origins.get(&p), Some(SessionKind::Ssh { .. })) {
            return None; // 원격 cwd는 로컬 경로가 아님.
        }
        let norm = strip_uri_slash(self.cwds.get(&p)?);
        std::path::Path::new(&norm).is_dir().then_some(norm)
    }

    /// 기본 셸을 분할로 연다(right=오른쪽, false=아래).
    pub(crate) fn split_shell(&mut self, right: bool) {
        self.pending_split = Some(right);
        let s = shell_from_str(&self.config.terminal.default_shell);
        self.spawn_local(s);
    }

    /// 워크스페이스 저장용: 로컬 pane의 마지막 cwd(실재 디렉터리) + 실행 중 명령(설정 시).
    /// 명령은 종료(OSC 133;D) 시 run_cmd에서 지워지므로, 남아 있으면 "아직 실행 중"이다.
    pub(crate) fn saved_local_state(
        &self,
        p: nabi_types::PaneId,
    ) -> (Option<String>, Option<String>) {
        let cwd = self
            .cwds
            .get(&p)
            .map(|c| strip_uri_slash(c))
            .filter(|d| std::path::Path::new(d).is_dir());
        let live = self.config.terminal.restore_running_command;
        let cmd = live
            .then(|| {
                self.run_cmd
                    .get(&p)
                    .cloned()
                    .filter(|c| !c.trim().is_empty())
            })
            .flatten()
            .map(|c| {
                // AI CLI는 재실행 대신 재개(세션 ID가 보고돼 있으면 그 세션으로 정확히 — A6).
                let sid = self.pane_status.get(&p).and_then(|m| m.get("agent_session")).map(String::as_str);
                crate::wsairesume::local_resume_command(&c, sid)
            });
        (cwd, cmd)
    }

    /// 현재 열린 탭들의 출처(+분할 레이아웃)를 워크스페이스 파일로 저장한다.
    /// 워크스페이스 파일을 읽어 각 세션을 다시 열고, 가능하면 분할 레이아웃을 복원한다.
    /// 저장된 워크스페이스를 복원한다. 복원할 세션이 하나라도 있으면 true.
    /// `browser_panes`: 먼저 복원해 둔 브라우저 탭(레이아웃 서수 1000+i와 매핑).
    /// 복원 시 이 세션이 '즉시' pane을 스폰하는가(레이아웃 매핑 대상). 볼트 잠금 상태를 반영 —
    /// 자격증명이 볼트에 있어도 볼트가 잠겨 있으면 즉시 연결되지 못하므로 false(로그인 대기).
    pub(crate) fn session_will_spawn(&self, kind: &SessionKind) -> bool {
        match kind {
            SessionKind::Local { .. } => true,
            SessionKind::Ssh {
                credential_ref,
                key_path,
                ..
            } => {
                key_path.is_some()
                    || credential_ref
                        .as_ref()
                        .is_some_and(|k| self.vault_get(k).is_some())
            }
        }
    }

    /// 시작 시 볼트를 먼저 풀어야 하는가 — 볼트 파일이 있고 아직 잠겨 있으며, 저장된 워크스페이스에
    /// 볼트 자격증명을 쓰는 SSH/SFTP 세션이 있으면 true(볼트 우선 잠금해제 → 자동 연결).
    pub(crate) fn workspace_wants_vault(&self) -> bool {
        if self.vault.is_some() || !self.vault_path.exists() {
            return false;
        }
        nabi_session::load_tree(&self.workspace_path)
            .sessions
            .iter()
            .any(|s| {
                matches!(
                    &s.kind,
                    SessionKind::Ssh {
                        credential_ref: Some(_),
                        ..
                    }
                )
            })
    }

    pub(crate) fn restore_workspace(&mut self, browser_panes: Vec<nabi_types::PaneId>) -> bool {
        while self.sftp_pane.is_some() {
            self.close_sftp(); // 복원 전 모든 원격 탭/연결 정리(stale 방지).
        }
        // 원격 탭은 여기서 재연결한다 — 볼트 잠금해제 이후여야 자격증명을 꺼낼 수 있고,
        // start_sftp가 pane을 동기적으로 만들어 주므로 레이아웃 매핑(2000+i)에 바로 쓸 수 있다.
        let sftp_panes = self.restore_sftp_tabs();
        let tree = nabi_session::load_tree(&self.workspace_path);
        let restored = !tree.sessions.is_empty() || !sftp_panes.is_empty(); // 원격 탭만 있어도 복원으로 친다(기본 셸 자동스폰 방지).
        // 즉시 스폰되는 세션(로컬·키/볼트-해제 SSH)의 ordinal만 레이아웃에 매핑한다. 로그인 필요
        // 세션(자격증명 없음·볼트 잠김)은 분할에서 빼 로컬 분할을 보존하고, 나중에 연결되면 합류.
        let spawn_ords: Vec<usize> = tree
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| self.session_will_spawn(&s.kind))
            .map(|(i, _)| i)
            .collect();
        if !spawn_ords.is_empty() {
            if let Some(saved) =
                std::fs::read_to_string(self.workspace_path.with_extension("layout"))
                    .ok()
                    .and_then(|s| ron::from_str::<DockState<usize>>(&s).ok())
            {
                let fonts = std::fs::read_to_string(self.workspace_path.with_extension("fonts"))
                    .ok()
                    .and_then(|s| ron::from_str::<Vec<f32>>(&s).ok())
                    .unwrap_or_default();
                let names = std::fs::read_to_string(self.workspace_path.with_extension("names"))
                    .ok()
                    .and_then(|s| ron::from_str::<Vec<String>>(&s).ok())
                    .unwrap_or_default();
                let colors = std::fs::read_to_string(self.workspace_path.with_extension("colors"))
                    .ok()
                    .and_then(|s| ron::from_str::<Vec<[u8; 4]>>(&s).ok())
                    .unwrap_or_default();
                self.pending_layout = Some(PendingLayout {
                    saved,
                    arrived: std::collections::HashMap::new(),
                    expected: spawn_ords.len(),
                    fonts,
                    names,
                    colors,
                    browser_panes,
                    sftp_panes,
                });
            }
        }
        for (i, s) in tree.sessions.into_iter().enumerate() {
            // 이 스폰의 레이아웃 ordinal(=i)과 로컬 백로그를 spawn_ctx로 전달 → register_spawn이 seq에 묶는다.
            let backlog = matches!(s.kind, SessionKind::Local { .. }).then(|| {
                self.workspace_path
                    .parent()
                    .map(|d| d.join(format!("scroll_{i}.txt")))
                    .and_then(|p| std::fs::read(p).ok())
                    .unwrap_or_default()
            });
            self.spawn_ctx = Some((Some(i), backlog, None));
            self.connect_saved(s);
        }
        self.restore_floating(); // 분리 OS 창(torn-off)도 위치·크기와 함께 복원(P10).
        self.spawn_ctx = None; // 복원 종료 — 이후 일반 스폰은 컨텍스트 없음.
        restored
    }
}

#[cfg(test)]
mod tests {
    use super::{shell_from_str, shell_to_str};

    #[test]
    fn strip_uri_slash_windows() {
        assert_eq!(super::strip_uri_slash("/C:/Users/x"), "C:/Users/x");
        assert_eq!(super::strip_uri_slash("/home/u"), "/home/u");
        assert_eq!(super::strip_uri_slash("C:/already"), "C:/already");
    }

    #[test]
    fn shell_str_roundtrip() {
        for s in ["powershell", "pwsh", "cmd", "wsl", "gitbash"] {
            assert_eq!(shell_to_str(&shell_from_str(s)), s);
        }
        // 알 수 없는 값은 기본 powershell로 폴백.
        assert_eq!(shell_to_str(&shell_from_str("zzz")), "powershell");
    }
}
