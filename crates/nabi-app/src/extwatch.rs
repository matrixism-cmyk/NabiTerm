//! 외부 파일 변경 감지(VS Code식) — 열린 nabiPad 문서의 디스크 mtime을 주기적으로 확인해
//! 미수정이면 자동 다시 불러오고, 수정 중이면 1회 경고한다. EditorDoc 변경 없이 앱 맵으로 관리.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

impl NabiApp {
    /// 파일 mtime을 기록한다(열기·저장 후). 자동 감지의 기준점.
    pub(crate) fn record_editor_mtime(&mut self, pane: PaneId, path: &Path) {
        if let Ok(m) = std::fs::metadata(path).and_then(|m| m.modified()) {
            self.editor_mtimes.insert(pane, m);
        }
    }

    /// 변경된 '경로 있는 일반 텍스트' 문서를 모두 정상 저장한다(인코딩·EOL·trim·mtime 갱신). 저장한 개수 반환.
    pub(crate) fn save_all_docs(&mut self) -> usize {
        let panes: Vec<PaneId> = self.editors.iter()
            .filter(|(_, d)| d.dirty && d.hex.is_none() && d.big.is_none() && d.edit.is_none() && d.remote.is_none() && !d.path.as_os_str().is_empty())
            .map(|(p, _)| *p).collect();
        let n = panes.len();
        for p in panes {
            self.save_editor_doc(p);
        }
        n
    }

    /// 자동 저장(VS Code files.autoSave): 15초마다 변경된 문서를 저장한다(설정 켜짐 시).
    pub(crate) fn autosave_tick(&mut self) {
        if !self.editor_config.autosave || self.autosave_at.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.autosave_at = Instant::now();
        self.save_all_docs();
    }

    /// 2초마다 열린 문서의 디스크 변경을 확인 → 미수정이면 리로드, 수정 중이면 경고(저장 시 덮어씀).
    pub(crate) fn check_external_changes(&mut self, ctx: &egui::Context) {
        if self.editor_extcheck.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.editor_extcheck = Instant::now();
        // 대상 수집(빌림 분리) — plain 로컬 텍스트 문서만.
        let checks: Vec<(PaneId, std::path::PathBuf, SystemTime, bool)> = self.editors.iter()
            .filter(|(_, d)| d.hex.is_none() && d.edit.is_none() && d.big.is_none() && d.remote.is_none() && !d.path.as_os_str().is_empty())
            .filter_map(|(p, d)| self.editor_mtimes.get(p).map(|m| (*p, d.path.clone(), *m, d.dirty)))
            .collect();
        for (pane, path, known, dirty) in checks {
            let Ok(cur) = std::fs::metadata(&path).and_then(|m| m.modified()) else { continue };
            if cur <= known {
                continue;
            }
            self.editor_mtimes.insert(pane, cur);
            if dirty {
                self.notify = Some((tr(self.lang, "editor.extchanged").to_string(), Instant::now()));
            } else {
                self.reload_editor_doc(pane); // 미수정 → 조용히 최신 내용으로 갱신.
                self.notify = Some((tr(self.lang, "editor.extreloaded").to_string(), Instant::now()));
                ctx.request_repaint();
            }
        }
    }
}
