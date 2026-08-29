//! 탭바 빈 공간 우클릭 메뉴 — 새 탭/파일 브라우저 탭/빠른 연결.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 각 탭 그룹(leaf)의 탭바 띠 안 우클릭을 감지해 메뉴를 띄우고 그 그룹을 포커스한다
    /// (이후 새 탭이 그 그룹에 생기도록). 탭 자체의 컨텍스트 메뉴가 열렸으면 양보.
    /// 비활성 pane 우클릭으로 들어온 포커스 요청을 dock에 적용한다(dock.show 직후 호출).
    pub(crate) fn apply_pending_focus(&mut self) {
        if let Some(tp) = self.focus_req.take().and_then(|p| self.dock.find_tab(&p)) {
            self.dock.set_focused_node_and_surface(egui_dock::NodePath { surface: tp.surface, node: tp.node });
        }
    }

    pub(crate) fn detect_tabbar_menu(&mut self, ctx: &egui::Context) {
        // 탭 위 우클릭은 **그 탭의 메뉴가 주인**이므로 양보한다(#3). egui_dock이 탭별 위치를
        // 알려주지 않아 "탭 위인지"를 좌표로 판정할 수 없어서, 탭 메뉴가 열려 있으면 물러선다.
        if self.tab_ctx_tab.is_some() {
            return;
        }
        // 일반 팝업(상단 메뉴·드롭다운·툴팁)은 **이 클릭이 연 경우에만** 양보한다.
        //
        // 예전에는 "지금 열려 있으면" 무조건 물러섰다. 그래서 메뉴를 한 번 쓰거나 툴팁이 떠 있으면
        // 다음 우클릭이 그 팝업을 닫는 데만 쓰이고 **우리 메뉴는 안 떴다** — 한 번 더 눌러야 떴다
        // (사용자 보고 2026-08-21 "빈 공간 우클릭이 잘 안 뜬다").
        let popup_now = egui::Popup::is_any_open(ctx);
        let popup_before = std::mem::replace(&mut self.popup_was_open, popup_now);
        if popup_now && !popup_before {
            return;
        }
        let Some(pos) = ctx.input(|i| {
            i.pointer
                .interact_pos()
                .filter(|_| i.pointer.secondary_clicked())
        }) else {
            return;
        };
        // 우클릭 위치가 어느 leaf의 탭바 띠(상단 32px)에 있는지 찾는다.
        let mut hit: Option<egui_dock::NodeIndex> = None;
        for (i, n) in self.dock.main_surface().iter().enumerate() {
            // 0.19: Leaf가 구조체 변형(LeafNode)이 됐고 rect는 Option 게터다.
            if let Some(rect) = n.get_leaf().map(|l| l.rect) {
                let band = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 32.0));
                if band.contains(pos) {
                    hit = Some(egui_dock::NodeIndex(i));
                    break;
                }
            }
        }
        if let Some(node) = hit {
            self.dock.set_focused_node_and_surface(egui_dock::NodePath { surface: egui_dock::SurfaceIndex::main(), node });
            self.tabbar_menu = Some(pos);
            self.tabbar_menu_fresh = true; // 이 프레임의 우클릭은 '메뉴 밖 클릭'이 아니다.
        }
    }

    /// 기록된 위치에 메뉴를 그린다(바깥 클릭/Esc로 닫힘).
    pub(crate) fn show_tabbar_menu(&mut self, ctx: &egui::Context) {
        let Some(pos) = self.tabbar_menu else { return };
        let lang = self.lang;
        let mut act: Option<u8> = None;
        let resp = egui::Area::new(egui::Id::new("tabbar_menu"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(150.0);
                    if ui.button(tr(lang, "central.newtab")).clicked() {
                        act = Some(0);
                    }
                    if ui.button(tr(lang, "menu.browsertab")).clicked() {
                        act = Some(1);
                    }
                    // 웹 탭도 여기서 연다 — 이제 다른 탭과 같은 것이니 같은 자리에 있어야 한다
                    // (사용자 요청 2026-08-29). 메뉴·팔레트와 표현을 맞춰 "웹 브라우저"로 적는다.
                    if ui.button(tr(lang, "web.title")).clicked() {
                        act = Some(3);
                    }
                    if ui.button(tr(lang, "qc.title")).clicked() {
                        act = Some(2);
                    }
                });
            })
            .response;
        // 닫기: 항목 선택, 메뉴 밖 클릭, Esc.
        //
        // **메뉴를 연 그 프레임에는 밖 클릭 판정을 하지 않는다.** egui가 Area를 화면 안에 맞추려
        // 클릭 위치가 아닌 곳에 배치하는 경우가 있는데(실측: 클릭 x=792인데 Area는 600~768),
        // 그러면 여는 우클릭이 곧바로 '밖 클릭'으로 잡혀 **뜨자마자 닫혔다**. 이게 "빈 공간
        // 우클릭이 잘 안 뜬다"의 정체였다(사용자 보고 2026-08-21).
        let fresh = std::mem::take(&mut self.tabbar_menu_fresh);
        let clicked_out = !fresh
            && ctx.input(|i| {
                (i.pointer.primary_clicked() || i.pointer.secondary_clicked())
                    && i.pointer.interact_pos().is_some_and(|p| !resp.rect.contains(p))
            });
        if act.is_some() || clicked_out || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.tabbar_menu = None;
        }
        match act {
            Some(0) => {
                let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
                self.spawn_local(shell);
            }
            Some(1) => {
                self.open_browser_tab();
            }
            Some(2) => self.open_quick_connect(),
            Some(3) => {
                self.open_web_tab(crate::webopen::HOME);
            }
            _ => {}
        }
    }
}
