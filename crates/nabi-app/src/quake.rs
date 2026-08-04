//! Quake 모드: OS 전역 핫키(Ctrl+`)로 창을 어디서나 보이기/숨기기 토글.
//!
//! `global-hotkey`가 자체 스레드로 핫키를 받고, 이벤트는 프로세스 전역 채널로 온다.
//! 매 프레임 폴링해 `ViewportCommand::Visible`로 창을 토글한다(드롭다운 콘솔 동작).

use crate::app::NabiApp;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};

pub(crate) struct QuakeState {
    _manager: GlobalHotKeyManager,
    id: u32,
    visible: bool,
}

/// 설정 문자열(global-hotkey 형식)로 전역 핫키를 등록한다. 파싱 실패는 Ctrl+`로 폴백,
/// 등록 실패(이미 점유 등)는 None → 기능 비활성.
pub(crate) fn init(spec: &str) -> Option<QuakeState> {
    let manager = GlobalHotKeyManager::new().ok()?;
    let hk = spec
        .parse::<HotKey>()
        .unwrap_or_else(|_| HotKey::new(Some(Modifiers::CONTROL), Code::Backquote));
    manager.register(hk).ok()?;
    Some(QuakeState {
        _manager: manager,
        id: hk.id(),
        visible: true,
    })
}

impl NabiApp {
    /// 전역 핫키 이벤트를 폴링해 창 표시/숨김을 토글한다(매 프레임 호출).
    pub(crate) fn poll_quake(&mut self, ctx: &egui::Context) {
        let Some(q) = self.quake.as_mut() else {
            return;
        };
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.id == q.id && ev.state == HotKeyState::Pressed {
                q.visible = !q.visible;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(q.visible));
                if q.visible {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }
}
