//! 웹 화면을 **탭 안에** 둔다(배치 AZ).
//!
//! ## 왜 별도 창이 아니라 탭인가
//!
//! 별도 창은 임시 형태였다. 탭에 있어야 다른 것과 나란히 놓고, 분할해서 보고, 작업 공간에
//! 함께 저장된다(사용자 요청 2026-08-29).
//!
//! ## 자식 창이라 지켜야 하는 것
//!
//! WebView2 는 운영체제가 자기 창에 그린다. 그 창은 우리가 egui 로 그리는 것보다 **늘 위에**
//! 온다. 그래서 우리가 직접 숨기고 옮겨 줘야 한다.
//!
//! * **안 보이는 탭이면 숨긴다.** 안 숨기면 다른 탭 위에 웹 화면이 그대로 남는다.
//! * **자리는 그 탭이 그려질 때 받아 둔다.** 매 프레임 맞춘다 — 탭을 옮기거나 창 크기가
//!   바뀌면 따라와야 한다.
//!
//! ## 만드는 시점
//!
//! 탭을 열 때 바로 만들지 않는다. 그때는 **아직 자리를 모른다**(화면에 그려져 봐야 안다).
//! 처음 그려질 때 만든다.

use nabi_types::PaneId;

/// 탭 하나가 들고 있는 것.
pub(crate) struct WebTab {
    /// 아직 안 만들었으면 `None`. 처음 그려질 때 만든다.
    pub view: Option<nabi_web::embed::Embedded>,
    /// 열려고 하는 주소. 만들 때 쓰고, 주소 칸에도 보여 준다.
    pub url: String,
    /// 만들다 실패했으면 그 이유. 화면에 그대로 보여 준다 — 빈 탭만 두지 않는다.
    pub failed: Option<String>,
}

impl WebTab {
    pub(crate) fn new(url: &str) -> Self {
        Self { view: None, url: url.to_string(), failed: None }
    }
}

impl crate::app::NabiApp {
    /// 웹 탭을 하나 연다.
    pub(crate) fn open_web_tab(&mut self, url: &str) -> PaneId {
        let p = nabi_types::next_pane_id(); // 오케스트레이터 pane 없는 UI 전용 id.
        self.web_tabs.insert(p, WebTab::new(url));
        self.add_pane(p);
        p
    }

    /// 안 보이는 웹 탭을 전부 숨긴다.
    ///
    /// 매 프레임 부른다. 그려진 탭은 자기가 다시 보이게 하므로, 여기서 한 번 다 숨겨도
    /// 깜빡이지 않는다 — 같은 값이면 아무 일도 하지 않기 때문이다.
    pub(crate) fn hide_unseen_web_tabs(&mut self, seen: &std::collections::HashSet<PaneId>) {
        for (id, tab) in self.web_tabs.iter_mut() {
            if !seen.contains(id) {
                if let Some(v) = &mut tab.view {
                    v.show(false);
                }
            }
        }
    }
}
