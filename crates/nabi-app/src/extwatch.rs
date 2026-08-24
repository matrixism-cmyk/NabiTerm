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

    /// 설정·복구본이 놓이는 폴더(config.toml이 있는 곳).
    pub(crate) fn cfg_dir(&self) -> std::path::PathBuf {
        self.config_path.parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }

    /// 자동 저장 + **미저장 문서 복구본 떨구기**를 15초마다.
    ///
    /// 자동 저장은 설정에 달렸지만 복구본은 **늘 남긴다**. 설정을 켜 둔 사람만 보호받는
    /// 안전장치는 안전장치가 아니다 — 잃고 나서야 그 설정을 찾게 된다.
    pub(crate) fn autosave_tick(&mut self) {
        if self.autosave_at.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.autosave_at = Instant::now();
        if self.editor_config.autosave {
            self.save_all_docs();
        }
        self.stash_unsaved();
    }

    /// 아직 저장한 적 없는(경로 없는) 수정된 문서를 복구 폴더에 떨군다.
    ///
    /// 경로가 있는 문서는 다루지 않는다 — 그쪽은 원본이 디스크에 있어 최악이라도 마지막
    /// 저장 시점으로 돌아갈 뿐이다. 잃을 것이 전부인 쪽은 제목 없는 새 문서다.
    fn stash_unsaved(&mut self) {
        let dir = self.cfg_dir();
        for (pane, d) in self.editors.iter() {
            if !d.path.as_os_str().is_empty() || !d.dirty || d.text.is_empty() {
                continue;
            }
            // 흘려 읽기·HEX 문서는 제목 없이 열릴 수 없다(파일에서만 온다) — text 문서만.
            if d.hex.is_some() || d.big.is_some() || d.huge.is_some() {
                continue;
            }
            let _ = crate::padrecover::stash(&dir, pane.0, &d.title, &d.text);
        }
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
