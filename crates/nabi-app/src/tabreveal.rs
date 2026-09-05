//! 탭이 많아 **오른쪽 밖으로 나간 탭**을 다시 손 닿게 한다.
//!
//! ## 무슨 일이 있었나
//!
//! 탭을 여러 개 열면 탭 줄이 꽉 찬다. 그다음에 새 탭을 열면 그 탭은 오른쪽 끝에 붙는데,
//! 붙을 자리가 없어서 **화면 밖에 생긴다.** 사용자는 새로 연 것이 아예 안 열린 줄 안다
//! (사용자 보고 2026-09-05).
//!
//! egui_dock 은 탭 줄을 옆으로 굴릴 수 있고 넘치면 막대도 그린다. 다만 **새 탭을 따라
//! 가지는 않는다** — 굴리는 것은 사람 몫이고, 사람은 거기 뭐가 생겼는지 모른다.
//!
//! ## 두 가지로 푼다
//!
//! **하나, 새 탭이 생기면 탭 줄을 맨 오른쪽까지 굴린다.** 새 탭은 늘 끝에 붙으므로
//! 끝까지 굴리면 반드시 보인다. 탭 폭을 재서 계산할 필요가 없다 — 그 계산은
//! egui_dock 안쪽 사정이라 우리가 흉내 내면 판이 바뀔 때마다 어긋난다.
//!
//! **둘, 탭 목록을 연다.** 굴려서 찾는 것은 탭이 스무 개쯤 되면 이미 불편하다.
//! 목록에서 고르면 폭과 상관없이 어느 탭이든 한 번에 간다.

use nabi_types::PaneId;

/// 굴림 값은 0(왼쪽 끝)에서 -넘침(오른쪽 끝) 사이로 잘린다. 큰 음수를 넣으면 오른쪽 끝이다.
const FAR_RIGHT: f32 = -1.0e6;

impl crate::app::NabiApp {
    /// 방금 붙인 탭이 보이도록 탭 줄을 맨 오른쪽까지 굴린다.
    ///
    /// 탭을 붙이는 모든 길에서 부른다. 한 곳이라도 빠지면 그 길로 연 탭만 안 보인다.
    pub(crate) fn reveal_new_tab(&mut self) {
        let Some(path) = self.dock.focused_leaf() else { return };
        if let Some(leaf) = self.dock[path.surface][path.node].get_leaf_mut() {
            leaf.scroll = FAR_RIGHT;
        }
    }

    /// 이 pane 이 있는 탭 줄을 그 탭 쪽으로 굴린다.
    ///
    /// 목록에서 골랐을 때 쓴다. 고른 탭이 어디 있는지 모르니 양 끝 중 가까운 쪽으로
    /// 굴리는 대신, **활성 탭으로 만들고 끝까지 굴려 본다** — 끝에 있으면 보이고,
    /// 앞쪽에 있으면 왼쪽 끝으로 굴려야 보인다. 어느 쪽인지는 탭 차례로 안다.
    pub(crate) fn focus_tab(&mut self, pane: PaneId) {
        let Some(loc) = self.dock.find_tab(&pane) else { return };
        let _ = self.dock.set_active_tab(loc);
        // 앞쪽 절반이면 왼쪽 끝, 뒤쪽 절반이면 오른쪽 끝으로 굴린다. 정확한 자리를
        // 맞추려면 탭 폭을 알아야 하는데 그것은 egui_dock 안쪽 사정이다. 끝으로
        // 굴리면 적어도 **보이기는 한다.**
        let node = &mut self.dock[loc.surface][loc.node];
        let Some(leaf) = node.get_leaf_mut() else { return };
        let n = leaf.tabs.len();
        let i = leaf.tabs.iter().position(|t| *t == pane).unwrap_or(0);
        leaf.scroll = match i * 2 >= n {
            true => FAR_RIGHT,
            false => 0.0,
        };
    }

    /// 지금 열려 있는 탭 전부 — 목록에 그릴 (pane, 이름) 짝.
    ///
    /// 이름은 탭에 붙은 것과 같은 규칙으로 짓는다. 목록에만 다른 이름이 뜨면 같은 탭을
    /// 두 이름으로 부르게 된다.
    pub(crate) fn all_tab_names(&self) -> Vec<(PaneId, String)> {
        self.dock
            .iter_all_tabs()
            .map(|(_, p)| (*p, self.tab_list_name(*p)))
            .collect()
    }

    /// 목록에 적을 탭 이름.
    fn tab_list_name(&self, pane: PaneId) -> String {
        if let Some(w) = self.web_tabs.get(&pane) {
            return format!("\u{1f310} {}", crate::webtab::tab_name(&w.title, &w.url));
        }
        if let Some(b) = self.browser_tabs.get(&pane) {
            let name = b
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| b.path.display().to_string());
            return format!("\u{1f4c1} {name}");
        }
        if let Some(e) = self.editors.get(&pane) {
            return format!("\u{270e} {}", e.title);
        }
        if Some(pane) == self.sftp_pane {
            return format!("\u{1f5a7} {}", self.sftp.host);
        }
        if let Some(p) = self.sftp_bg.get(&pane) {
            return format!("\u{1f5a7} {}", p.host);
        }
        self.tab_names.get(&pane).cloned().unwrap_or_else(|| {
            self.orch
                .panes
                .read()
                .ok()
                .and_then(|m| m.get(&pane).map(|v| v.title.clone()))
                .unwrap_or_else(|| format!("#{}", pane.get()))
        })
    }
}
