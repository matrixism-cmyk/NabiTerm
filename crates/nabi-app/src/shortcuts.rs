//! 전역 키보드 단축키. consume_key로 처리해 터미널 입력으로 새지 않게 한다.

use crate::app::NabiApp;
use egui::{Key, Modifiers};

impl NabiApp {
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // 키보드 직접 붙여넣기(Ctrl+V 등)가 여러 줄이면 확인 경로로 가로챈다(터미널 렌더 전에).
        self.intercept_keyboard_paste(ctx);
        let cs = Modifiers {
            command: true,
            shift: true,
            ..Modifiers::NONE
        };
        // Ctrl+Shift+T: 새 로컬 터미널(기본 셸).
        if ctx.input_mut(|i| i.consume_key(cs, Key::T)) {
            let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
            self.spawn_local(shell);
        }
        // Ctrl+Shift+W: 포커스된 pane 닫기.
        if ctx.input_mut(|i| i.consume_key(cs, Key::W)) {
            if let Some(p) = self.focused_pane() {
                self.orch.send(nabi_proto::Command::ClosePane { pane: p });
            }
        }
        // Ctrl+Shift+E: 파일 브라우저 토글(열 때 현재 작업 디렉터리에서).
        if ctx.input_mut(|i| i.consume_key(cs, Key::E)) {
            self.toggle_browser();
        }
        // Ctrl+Shift+K: 화면 지우기(셸에 Ctrl+L 전송).
        if ctx.input_mut(|i| i.consume_key(cs, Key::K)) {
            if let Some(p) = self.focused_pane() {
                self.orch.send(nabi_proto::Command::WriteInput {
                    pane: p,
                    data: bytes::Bytes::from_static(&[0x0c]),
                });
            }
        }
        // Ctrl+C/X 처리: egui-winit은 Ctrl+C/X를 Event::Copy/Cut로 바꾸며 Key 이벤트를
        // 내지 않으므로(키 시퀀스로 못 감) 여기서 직접 처리한다.
        //  · Shift 동반(Ctrl+Shift+C) → 선택 복사(터미널 관례).
        //  · Shift 없음(Ctrl+C/X) → 포커스 pane에 제어 바이트 전송(C=0x03 브레이크, X=0x18).
        // 위젯 포커스/팝업 중에는 건너뛰어(=blocked) TextEdit 자체 복사를 방해하지 않는다.
        // ⚠️ 터미널 포커스 싱크(Tab 수정용)는 항상 포커스를 잡으므로 단순 focused().is_some()로
        // 판정하면 Ctrl+C가 영구 차단된다 → 싱크를 제외하는 term_input_blocked를 쓴다.
        let blocked = crate::paneio::term_input_blocked(ctx);
        if !blocked {
            let (copy_ev, cut_ev, shift) = ctx.input(|i| {
                (
                    i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                    i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                    i.modifiers.shift,
                )
            });
            if shift {
                if copy_ev {
                    self.copy_selection(ctx);
                }
            } else if copy_ev || cut_ev {
                if let Some(p) = self.focused_pane() {
                    let b: u8 = if copy_ev { 0x03 } else { 0x18 };
                    self.orch.send(nabi_proto::Command::WriteInput {
                        pane: p,
                        data: bytes::Bytes::copy_from_slice(&[b]),
                    });
                }
            }
        }
        // Ctrl+Shift+V: 클립보드 붙여넣기.
        if ctx.input_mut(|i| i.consume_key(cs, Key::V)) {
            self.paste_to_focused();
        }
        // Ctrl+Shift+D: 현재 탭 복제.
        if ctx.input_mut(|i| i.consume_key(cs, Key::D)) {
            self.duplicate_focused();
        }
        // Ctrl+Shift+M: MultiExec(브로드캐스트) 토글.
        if ctx.input_mut(|i| i.consume_key(cs, Key::M)) {
            self.broadcast = !self.broadcast;
        }
        // Ctrl+Shift+Q: 종료(여러 탭이면 확인 대화상자).
        if ctx.input_mut(|i| i.consume_key(cs, Key::Q)) {
            if self.config.terminal.confirm_close && self.dock.iter_all_tabs().count() > 1 {
                self.confirm_close = true;
            } else {
                self.quit();
            }
        }
        // Ctrl+Shift+B: 상태바 표시 토글.
        if ctx.input_mut(|i| i.consume_key(cs, Key::B)) {
            self.config.appearance.show_statusbar = !self.config.appearance.show_statusbar;
        }
        // Ctrl+Shift+N: Quick Connect(새 SSH 연결).
        if ctx.input_mut(|i| i.consume_key(cs, Key::N)) {
            self.open_quick_connect();
        }
        // Ctrl+Shift+Z: 포커스 터미널 pane 최대화(줌) 토글(tmux식).
        if ctx.input_mut(|i| i.consume_key(cs, Key::Z)) {
            self.toggle_pane_zoom();
        }
        // Ctrl+Shift+A: 화면 전체 선택 + 클립보드 복사.
        if ctx.input_mut(|i| i.consume_key(cs, Key::A)) {
            if let Some(t) = self.select_all_focused() {
                ctx.copy_text(t);
            }
        }
        // Ctrl+Shift+\ : 오른쪽 분할, Ctrl+Shift+- : 아래로 분할.
        if ctx.input_mut(|i| i.consume_key(cs, Key::Backslash)) {
            self.split_shell(true);
        }
        if ctx.input_mut(|i| i.consume_key(cs, Key::Minus)) {
            self.split_shell(false);
        }
        // Ctrl+Shift+0 : 포커스된 pane의 확대/축소를 전역 기본으로 초기화.
        if ctx.input_mut(|i| i.consume_key(cs, Key::Num0)) {
            if let Some(p) = self.focused_pane() {
                self.pane_font.remove(&p);
            }
        }
        // Shift+Insert: 붙여넣기(X11 표준).
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        if ctx.input_mut(|i| i.consume_key(shift, Key::Insert)) {
            self.paste_to_focused();
        }
        // Ctrl+Insert: 선택 복사(X11 표준).
        let ctrl_only = Modifiers {
            command: true,
            ..Modifiers::NONE
        };
        if ctx.input_mut(|i| i.consume_key(ctrl_only, Key::Insert)) {
            self.copy_selection(ctx);
        }
        // Ctrl+F: 찾기 토글. F3/Shift+F3: 다음/이전. consume로 셸에 ^F·함수키 시퀀스 누수 방지.
        if ctx.input_mut(|i| i.consume_key(ctrl_only, Key::F)) {
            self.toggle_find();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F3)) {
            self.find_nav(true);
        }
        if ctx.input_mut(|i| i.consume_key(shift, Key::F3)) {
            self.find_nav(false);
        }

        // 줌: Ctrl+= / Ctrl++ 확대, Ctrl+- 축소, Ctrl+0 기본(14).
        let ctrl = Modifiers {
            command: true,
            ..Modifiers::NONE
        };
        let zoom_in = ctx.input_mut(|i| {
            i.consume_key(ctrl, Key::Equals)
                || i.consume_key(ctrl, Key::Plus)
                || i.consume_key(cs, Key::Plus)
        });
        if zoom_in {
            self.set_font_size(self.font_size + 1.0);
        }
        if ctx.input_mut(|i| i.consume_key(ctrl, Key::Minus)) {
            self.set_font_size(self.font_size - 1.0);
        }
        if ctx.input_mut(|i| i.consume_key(ctrl, Key::Num0)) {
            self.set_font_size(nabi_config::DEFAULT_FONT_SIZE); // 설정 기본값과 동일(SSOT).
        }

        // F11: 전체화면 토글.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            self.fullscreen = !self.fullscreen;
            self.pending_fullscreen = Some(self.fullscreen);
        }

        // Alt+1..9: N번째 탭으로 전환.
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        const NUMS: [Key; 9] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        for (idx, key) in NUMS.iter().enumerate() {
            if ctx.input_mut(|i| i.consume_key(alt, *key)) {
                let pane = self.dock.iter_all_tabs().nth(idx).map(|(_, p)| *p);
                if let Some(loc) = pane.and_then(|p| self.dock.find_tab(&p)) {
                    let _ = self.dock.set_active_tab(loc);
                }
            }
        }

        // Ctrl+PageUp/PageDown: 이전/다음 탭으로 순환.
        let prev = ctx.input_mut(|i| i.consume_key(ctrl, Key::PageUp));
        let next = ctx.input_mut(|i| i.consume_key(ctrl, Key::PageDown));
        if prev || next {
            let tabs: Vec<_> = self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
            if !tabs.is_empty() {
                let cur = self
                    .focused_pane()
                    .and_then(|f| tabs.iter().position(|p| *p == f))
                    .unwrap_or(0);
                let n = tabs.len();
                let idx = if next { (cur + 1) % n } else { (cur + n - 1) % n };
                if let Some(loc) = self.dock.find_tab(&tabs[idx]) {
                    let _ = self.dock.set_active_tab(loc);
                }
            }
        }
    }
}
