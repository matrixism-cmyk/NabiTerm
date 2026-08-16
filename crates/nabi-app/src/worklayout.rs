//! 워크스페이스 분할 레이아웃 + pane 사이드카(글꼴·이름·색) 저장/복원(workspace.rs에서 분리).

use crate::app::NabiApp;
use crate::workspace::PendingSpawn;
use nabi_session::{SavedSession, SessionKind};
use nabi_types::PaneId;

/// 분리 OS 창 1개의 저장 항목(floating.ron) — 출처 + 마지막 cwd/명령 + 창 기하[x,y,w,h].
#[derive(serde::Serialize, serde::Deserialize)]
struct FloatSave {
    kind: SessionKind,
    on_connect: Option<String>,
    cwd: Option<String>,
    geom: [f32; 4],
}

impl NabiApp {
    /// 분리 OS 창(torn-off) pane들의 출처+기하를 floating.ron에 저장한다(정상 종료 시, P10).
    /// 에디터/브라우저/SFTP 분리창은 출처 모델이 달라 제외 — 터미널/SSH 셸만.
    /// 로컬 분리창은 도크와 동일하게 스크롤백을 fscroll_{i}.txt로 저장해 복원 시 화면을 되살린다.
    pub(crate) fn save_floating(&self) {
        let path = self.workspace_path.with_extension("floating");
        let dir = self.workspace_path.parent();
        // 이전 분리창 백로그 정리(도크 scroll_ 정리와 네임스페이스 분리).
        if let Some(d) = dir {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().starts_with("fscroll_") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        // 인덱스 정합을 위해 먼저 대상 pane을 확정(저장 Vec과 fscroll_ 인덱스가 일치).
        let panes: Vec<PaneId> = self
            .floating
            .iter()
            .copied()
            .filter(|p| !self.editors.contains_key(p) && !self.browser_tabs.contains_key(p))
            .filter(|p| self.pane_origins.contains_key(p) && self.floating_geom.contains_key(p))
            .collect();
        let saves: Vec<FloatSave> = panes
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let kind = self.pane_origins.get(p).cloned()?;
                let geom = self.floating_geom.get(p).copied()?;
                let (cwd, on_connect) = match kind {
                    SessionKind::Local { .. } => {
                        self.save_float_backlog(dir, *p, i); // 로컬 스크롤백 백로그.
                        self.saved_local_state(*p)
                    }
                    SessionKind::Ssh { .. } => (None, self.saved_ssh_ai_command(*p)),
                };
                Some(FloatSave {
                    kind,
                    on_connect,
                    cwd,
                    geom,
                })
            })
            .collect();
        if saves.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else if let Ok(s) = ron::to_string(&saves) {
            let _ = std::fs::write(path, s);
        }
    }

    /// 분리 로컬 pane의 마지막 2000줄을 fscroll_{i}.txt로 저장한다(복원 시 inject_restore_backlog).
    fn save_float_backlog(&self, dir: Option<&std::path::Path>, pane: PaneId, idx: usize) {
        let Some(d) = dir else { return };
        if let Some(v) = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).cloned())
        {
            if let Ok(md) = v.model.lock() {
                let txt = md.dump_text(2000);
                if !txt.is_empty() {
                    let _ = std::fs::write(d.join(format!("fscroll_{idx}.txt")), txt);
                }
            }
        }
    }

    /// floating.ron을 읽어 분리 OS 창을 위치·크기와 함께 다시 연다(P10). 복원 루프 끝에서 호출.
    pub(crate) fn restore_floating(&mut self) {
        let path = self.workspace_path.with_extension("floating");
        let Ok(txt) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(saves) = ron::from_str::<Vec<FloatSave>>(&txt) else {
            return;
        };
        let dir = self.workspace_path.parent().map(|d| d.to_path_buf());
        for (i, f) in saves.into_iter().enumerate() {
            // 로컬 분리창은 저장해 둔 스크롤백(fscroll_{i})을 backlog로 주입 — 도크와 동일.
            let backlog = matches!(f.kind, SessionKind::Local { .. }).then(|| {
                dir.as_ref()
                    .and_then(|d| std::fs::read(d.join(format!("fscroll_{i}.txt"))).ok())
                    .unwrap_or_default()
            });
            self.spawn_ctx = Some((None, backlog, Some(f.geom))); // float_geom → PaneSpawned가 floating으로.
            self.connect_saved(SavedSession {
                name: "workspace".into(),
                folder: None,
                kind: f.kind,
                on_connect: f.on_connect,
                cwd: f.cwd,
                is_ftp: false,
                open_sftp: false,
            });
        }
    }

    /// 앱 발급 스폰 seq를 하나 발급하고, 그 seq로 미완료 스폰 정보를 등록한다.
    /// 복원 컨텍스트(spawn_ctx)가 있으면 ordinal/백로그를 흡수한다. 스폰 명령의 reply_seq로 전달한다.
    pub(crate) fn register_spawn(&mut self, origin: SessionKind, oncmd: Option<String>) -> u64 {
        let (ordinal, backlog, float_geom) = self.spawn_ctx.take().unwrap_or((None, None, None));
        self.next_spawn_seq += 1;
        let seq = self.next_spawn_seq;
        self.pending_spawns.insert(
            seq,
            PendingSpawn {
                origin,
                oncmd,
                backlog,
                ordinal,
                float_geom,
            },
        );
        seq
    }

    /// 복원 백로그를 pane 모델에 주입한다(표시 전용 — 위로 스크롤하면 이전 세션 기록).
    pub(crate) fn inject_restore_backlog(&self, pane: PaneId, b: &[u8]) {
        if b.is_empty() {
            return;
        }
        if let Some(v) = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).cloned())
        {
            if let Ok(mut md) = v.model.lock() {
                md.process(b);
                md.process("\r\n\u{1b}[90m\u{2500}\u{2500} 이전 세션 기록(위로 스크롤) \u{2500}\u{2500}\u{1b}[0m".as_bytes());
                // 백로그를 히스토리로 밀어 ConPTY 초기 화면 재그리기(2J)에 지워지지 않게 한다.
                let rows = md.size().rows() as usize;
                md.process("\r\n".repeat(rows).as_bytes());
            }
        }
    }

    /// 도착한 pane을 ordinal로 레이아웃에 등록하고, 모두 도착하면 분할 트리·사이드카(글꼴/이름/색)를 적용한다.
    pub(crate) fn layout_arrive(&mut self, ordinal: usize, pane: PaneId) {
        let Some(pl) = self.pending_layout.as_mut() else {
            return;
        };
        pl.arrived.insert(ordinal, pane);
        if pl.arrived.len() < pl.expected {
            return;
        }
        let Some(pl) = self.pending_layout.take() else { return }; // 위에서 확인했지만 unwrap 금지(T4-1).
        // ordinal → 도착 pane. 로그인 필요 칸은 없으므로 None → filter_map_tabs가 트리에서 제거(분할 보존).
        // 같은 pane이 두 탭으로 배치되지 않도록 dedup 가드(복원 시 동일 #번호 탭 2개 방지, 사용자 보고 #1).
        let mut placed = std::collections::HashSet::new();
        self.dock = pl.saved.filter_map_tabs(|&ord| {
            let pane = match ord {
                o if o >= 1000 => pl.browser_panes.get(o - 1000).copied(),
                o => pl.arrived.get(&o).copied(),
            }?;
            placed.insert(pane).then_some(pane)
        });
        for (&ord, p) in pl.arrived.iter() {
            if let Some(f) = pl.fonts.get(ord) {
                self.pane_font.insert(*p, *f);
            }
            if let Some(n) = pl.names.get(ord).filter(|n| !n.is_empty()) {
                self.tab_names.insert(*p, n.clone());
            }
            if let Some(c) = pl.colors.get(ord).filter(|c| c[3] != 0) {
                self.tab_colors.insert(
                    *p,
                    egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]),
                );
            }
        }
    }

    /// 분할 레이아웃과 pane별 사이드카(글꼴/이름/색)를 RON으로 저장한다.
    /// 모든 터미널 탭에 출처가 있어 레이아웃을 신뢰할 수 있을 때만 기록하고,
    /// 아니면(브라우저 탭 포함 등) 낡은 사이드카를 정리해 다음 복원이 도크를 덮어쓰지 않게 한다.
    pub(crate) fn save_layout_sidecars(
        &self,
        ordered: &[PaneId],
        term_ordered: &[PaneId],
        count: usize,
    ) {
        if count != term_ordered.len() || ordered.is_empty() {
            for ext in ["layout", "fonts", "names", "colors"] {
                let _ = std::fs::remove_file(self.workspace_path.with_extension(ext));
            }
            return;
        }
        let mut index: std::collections::HashMap<PaneId, usize> = term_ordered
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, i))
            .collect();
        let mut bi = 0usize;
        for p in ordered {
            if self.browser_tabs.contains_key(p) {
                index.insert(*p, 1000 + bi); // save_browser_tabs와 같은 도크 순서.
                bi += 1;
            }
        }
        let layout = self.dock.map_tabs(|p| *index.get(p).unwrap_or(&0));
        self.write_sidecar("layout", &layout);
        let fonts: Vec<f32> = term_ordered
            .iter()
            .map(|p| self.pane_font.get(p).copied().unwrap_or(self.font_size))
            .collect();
        self.write_sidecar("fonts", &fonts);
        let names: Vec<String> = term_ordered
            .iter()
            .map(|p| self.tab_names.get(p).cloned().unwrap_or_default())
            .collect();
        self.write_sidecar("names", &names);
        let colors: Vec<[u8; 4]> = term_ordered
            .iter()
            .map(|p| {
                self.tab_colors
                    .get(p)
                    .map(|c| c.to_array())
                    .unwrap_or([0; 4])
            })
            .collect();
        self.write_sidecar("colors", &colors);
    }

    /// 값을 RON으로 직렬화해 workspace 경로의 `.<ext>` 사이드카에 쓴다(실패는 무시).
    fn write_sidecar<T: serde::Serialize>(&self, ext: &str, value: &T) {
        if let Ok(s) = ron::to_string(value) {
            let _ = std::fs::write(self.workspace_path.with_extension(ext), s);
        }
    }
}

impl NabiApp {
    /// 현재 레이아웃을 JSON으로(B4). `panes`=apply가 소비하는 순서 목록(cwd·명령·제목),
    /// `tree`=egui_dock 분할 트리(정확한 토폴로지 — 참고용, apply v1은 순서만 쓴다).
    pub(crate) fn layout_export_json(&self) -> String {
        let mut panes = Vec::new();
        for (_, p) in self.dock.iter_all_tabs() {
            let kind = self.pane_origins.get(p);
            let (cwd, cmd) = self.saved_local_state(*p);
            let title = self.orch.panes.read().ok()
                .and_then(|m| m.get(p).map(|v| v.title.clone()))
                .unwrap_or_default();
            let kind_s = match kind {
                Some(nabi_session::SessionKind::Ssh { .. }) => "ssh",
                _ => "local",
            };
            panes.push(serde_json::json!({
                "title": title, "kind": kind_s, "cwd": cwd, "command": cmd,
            }));
        }
        let ordinals: std::collections::HashMap<nabi_types::PaneId, usize> = self
            .dock.iter_all_tabs().enumerate().map(|(i, (_, p))| (*p, i)).collect();
        let tree = self.dock.map_tabs(|p| *ordinals.get(p).unwrap_or(&0));
        serde_json::json!({ "panes": panes, "tree": tree }).to_string()
    }
}

