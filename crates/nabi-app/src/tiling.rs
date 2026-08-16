//! 탭 타일/합치기 재배치 — workspace.rs에서 분리(라인 한도).

use crate::app::NabiApp;

impl NabiApp {
    /// 탭 그룹(leaf)들을 분할 타일로 재배치한다 — 한 탭바에 묶인 탭은 묶음 유지.
    pub(crate) fn tile_tabs(&mut self) {
        use egui_dock::NodeIndex;
        let focused = self.focused_pane();
        // 0.19: Leaf가 LeafNode 구조체 — tabs는 게터로 꺼낸다.
        let groups: Vec<Vec<nabi_types::PaneId>> = self
            .dock
            .main_surface()
            .iter()
            .filter_map(|n| n.tabs().filter(|t| !t.is_empty()).map(|t| t.to_vec()))
            .collect();
        if groups.len() < 2 {
            return;
        }
        self.dock = egui_dock::DockState::new(groups[0].clone());
        let tree = self.dock.main_surface_mut();
        let mut node = NodeIndex::root();
        for (i, g) in groups[1..].iter().enumerate() {
            let res = if i % 2 == 0 {
                tree.split_right(node, 0.5, g.clone())
            } else {
                tree.split_below(node, 0.5, g.clone())
            };
            node = res[1];
        }
        // 재배치 후 포커스를 원래 탭으로(이후 새 탭이 엉뚱한 타일에 붙지 않도록).
        if let Some(fp) = focused {
            if let Some(loc) = self.dock.find_tab(&fp) {
                self.dock.set_focused_node_and_surface(egui_dock::NodePath { surface: loc.surface, node: loc.node });
                let _ = self.dock.set_active_tab(loc);
            }
        }
    }

    /// 분할된 pane들을 다시 한 탭 묶음으로 합친다(타일 해제).
    pub(crate) fn tabify_tabs(&mut self) {
        let panes: Vec<nabi_types::PaneId> =
            self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
        if !panes.is_empty() {
            self.dock = egui_dock::DockState::new(panes);
        }
    }

}