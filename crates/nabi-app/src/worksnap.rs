//! 워크스페이스 명명 스냅샷(T7-2) — 현재 작업 배치를 이름으로 저장하고 전환한다.
//!
//! SecureCRT Snapshots·Termius Workspaces 대응. 스냅샷 = workspace 파일 세트
//! (toml + layout/fonts/names/colors/btabs 사이드카)의 사본. 전환은
//! "현재 상태 저장 → 로컬 탭 정리 → 스냅샷을 현재 슬롯에 복사 → 복원" 순서라
//! 어느 시점에도 파일이 유실되지 않는다. 원격(SFTP) 탭은 스냅샷 대상이 아니므로 유지된다.

use crate::app::NabiApp;
use std::path::PathBuf;

/// 스냅샷을 구성하는 사이드카 확장자(workspace.toml 기준).
const EXTS: &[&str] = &["toml", "layout", "fonts", "names", "colors", "btabs"];

impl NabiApp {
    fn snapshot_dir(&self) -> PathBuf {
        self.workspace_path.parent().map(|p| p.join("snapshots")).unwrap_or_else(|| PathBuf::from("snapshots"))
    }

    /// 저장된 스냅샷 이름 목록(가나다순).
    pub(crate) fn list_snapshots(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.snapshot_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension().and_then(|x| x.to_str()) == Some("toml"))
                    .then(|| p.file_stem()?.to_str().map(str::to_string))
                    .flatten()
            })
            .collect();
        v.sort();
        v
    }

    /// 현재 워크스페이스를 `name` 스냅샷으로 저장한다(같은 이름은 덮어씀).
    pub(crate) fn save_snapshot(&mut self, name: &str) {
        let name = sanitize(name);
        if name.is_empty() {
            return;
        }
        self.save_workspace(); // 현재 상태를 먼저 파일로.
        let dir = self.snapshot_dir();
        let _ = std::fs::create_dir_all(&dir);
        for ext in EXTS {
            let src = self.workspace_path.with_extension(ext);
            let dst = dir.join(format!("{name}.{ext}"));
            if src.exists() {
                let _ = std::fs::copy(&src, &dst);
            } else {
                let _ = std::fs::remove_file(&dst); // 사이드카가 없어졌으면 사본도 정리.
            }
        }
        self.notify = Some((format!("\u{1f4f7} {name}"), std::time::Instant::now()));
    }

    /// `name` 스냅샷으로 전환한다. 미저장 편집 문서가 있으면 중단(데이터 보호).
    pub(crate) fn open_snapshot(&mut self, name: &str) -> bool {
        let dir = self.snapshot_dir();
        if !dir.join(format!("{name}.toml")).exists() {
            return false;
        }
        if self.editors.values().any(|e| e.dirty) {
            self.notify = Some((nabi_i18n::tr(self.lang, "snap.dirty").to_string(), std::time::Instant::now()));
            return false;
        }
        self.save_workspace(); // 떠나는 상태 보존(마지막 자동 슬롯).
        // 로컬 탭 정리: 터미널은 ClosePane(이벤트가 dock 정리), 에디터/브라우저 탭은 UI 전용이라 직접 제거.
        let panes: Vec<nabi_types::PaneId> = self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
        for p in panes {
            if self.editors.remove(&p).is_some() || self.browser_tabs.remove(&p).is_some() {
                if let Some(loc) = self.dock.find_tab(&p) {
                    self.dock.remove_tab(loc);
                }
            } else if Some(p) != self.sftp_pane && !self.sftp_bg.contains_key(&p) {
                self.orch.send(nabi_proto::Command::ClosePane { pane: p });
            }
        }
        // 스냅샷 파일을 현재 슬롯으로 복사 후 복원.
        for ext in EXTS {
            let src = dir.join(format!("{name}.{ext}"));
            let dst = self.workspace_path.with_extension(ext);
            if src.exists() {
                let _ = std::fs::copy(&src, &dst);
            } else {
                let _ = std::fs::remove_file(&dst);
            }
        }
        let b = self.restore_browser_tabs();
        self.restore_workspace(b);
        self.notify = Some((format!("\u{21c4} {name}"), std::time::Instant::now()));
        true
    }

    /// 스냅샷을 삭제한다.
    pub(crate) fn delete_snapshot(&self, name: &str) {
        let dir = self.snapshot_dir();
        for ext in EXTS {
            let _ = std::fs::remove_file(dir.join(format!("{name}.{ext}")));
        }
    }
}

/// 파일명으로 안전한 이름만 남긴다(경로 탈출·예약문자 제거).
fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .chars()
        .take(60)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn sanitize_strips_path_hazards() {
        assert_eq!(sanitize("../../evil"), "....evil");
        assert_eq!(sanitize("dev: 서버 A?"), "dev 서버 A");
        assert_eq!(sanitize("  이름  "), "이름");
        assert!(sanitize("///").is_empty());
    }
}
