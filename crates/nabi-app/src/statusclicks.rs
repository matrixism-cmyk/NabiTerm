//! 상태 표시줄에서 **무엇을 눌렀는지** 받아 적용한다 — statusbar.rs 에서 갈라 왔다(줄 한도).
//!
//! 그리는 동안에는 `self` 를 이미 빌리고 있어서 그 자리에서 처리할 수 없다. 누른 것을
//! 자루에 담아 두고 그린 뒤에 한꺼번에 적용한다. 칩이 늘 때마다 이 자루에 한 칸 붙는다.

use crate::app::NabiApp;

/// 상태 표시줄에서 누른 것들 — 그린 뒤에 한꺼번에 적용한다.
pub(crate) struct StatusClicks {
    pub want_dims: Option<(u16, u16)>,
    pub goto_tab: Option<nabi_types::PaneId>,
    pub clip_pick: Option<String>,
    pub clip_windows: bool,
    pub jump_fail: bool,
    pub focus_sftp: bool,
}

impl NabiApp {
    /// 상태 표시줄에서 무엇을 눌렀는지 받아 적용한다.
    ///
    /// 그리는 동안에는 `self` 를 이미 빌리고 있어서 그 자리에서 처리할 수 없다. 그래서
    /// 누른 것을 자루에 담아 두고 그린 뒤에 한꺼번에 적용한다.
    pub(crate) fn apply_status_clicks(&mut self, ctx: &egui::Context, c: StatusClicks) {
        if let Some(want) = c.want_dims {
            self.resize_window_for_grid(ctx, want);
        }
        if let Some(p) = c.goto_tab {
            self.focus_tab(p);
        }
        if let Some(t) = c.clip_pick {
            self.paste_text_to_focused(t);
        }
        if c.clip_windows {
            crate::statusclip::open_windows_clipboard();
        }
        if c.jump_fail {
            self.jump_failed(true); // 칩을 누르면 다음 실패한 명령으로.
        }
        if c.focus_sftp {
            if let Some(p) = self.sftp_pane {
                if let Some(loc) = self.dock.find_tab(&p) {
                    let _ = self.dock.set_active_tab(loc);
                }
            }
        }
    }
}
