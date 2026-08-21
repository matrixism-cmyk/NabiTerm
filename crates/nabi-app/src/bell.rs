//! 비주얼 벨: 터미널이 BEL을 울리면 화면을 잠깐 번쩍인다(소리 대신 시각 피드백).

use crate::app::NabiApp;

/// 시스템 알림음(Windows MessageBeep) — 비동기·논블로킹, 실패해도 무해.
pub(crate) fn system_beep() {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBeep(u_type: u32) -> i32;
    }
    // SAFETY: 인자 없는 Win32 호출. MessageBeep은 상수 하나만 받고 포인터를 다루지 않으며,
    // 실패해도 0을 돌려줄 뿐 프로세스 상태를 건드리지 않는다.
    unsafe {
        let _ = MessageBeep(0x30); // MB_ICONEXCLAMATION — 주의 환기 톤.
    }
}
use nabi_types::PaneId;
use std::time::{Duration, Instant};

impl NabiApp {
    pub(crate) fn visual_bell(&mut self, ctx: &egui::Context) {
        // 1) 각 pane의 벨 카운트 증가를 감지(가드 잡은 채 self를 변경하지 않도록 먼저 수집).
        let counts: Vec<(PaneId, usize)> = match self.orch.panes.read() {
            Ok(panes) => panes
                .iter()
                .filter_map(|(id, v)| v.model.lock().ok().map(|m| (*id, m.bell_count())))
                .collect(),
            Err(_) => return,
        };
        for (id, c) in counts {
            let last = self.last_bell.get(&id).copied().unwrap_or(c);
            if c > last {
                if self.config.appearance.visual_bell {
                    self.bell_flash = Some(Instant::now());
                }
                // 벨이 울렸는데 창이 비포커스면 작업표시줄로 주의 환기(벨의 본래 목적).
                if !ctx.input(|i| i.focused) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
                }
            }
            self.last_bell.insert(id, c);
        }

        // 2) 플래시 렌더(짧게).
        if let Some(t) = self.bell_flash {
            if t.elapsed() < Duration::from_millis(120) {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("nabi-bell"),
                ));
                painter.rect_filled(
                    ctx.content_rect(),
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_white_alpha(48),
                );
                ctx.request_repaint();
            } else {
                self.bell_flash = None;
            }
        }
    }
}
