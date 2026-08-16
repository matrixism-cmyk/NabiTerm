//! SFTP 전송 히스토리(S6-60) — 완료/실패한 전송의 세션 내 기록 + 목록 창.
//!
//! rclone/WinSCP처럼 "방금 뭘 옮겼고 뭐가 실패했나"를 큐가 비워진 뒤에도 되짚을 수 있다.
//! 기록은 메모리 전용(세션 한정) — 파일 로그는 후속(개인정보·경로 노출 고려).

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 전송 1건의 결과 기록.
pub struct XferRecord {
    pub name: String,
    /// true=업로드(↑), false=다운로드(↓).
    pub up: bool,
    pub ok: bool,
    pub size: u64,
    pub secs: f64,
    /// 실패 사유(성공이면 빈 문자열).
    pub err: String,
    /// 기록 시각(epoch 초) — humanfmt::human_age와 짝.
    pub when: u64,
}

/// 현재 epoch 초.
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 기록 상한 — 오래된 것부터 버린다(메모리 상한, 순수 로직).
pub const HISTORY_CAP: usize = 200;

/// 최신이 앞에 오도록 넣고 상한을 넘으면 꼬리를 자른다.
pub fn push_record(list: &mut Vec<XferRecord>, rec: XferRecord) {
    list.insert(0, rec);
    list.truncate(HISTORY_CAP);
}

impl NabiApp {
    /// 전송 완료 훅에서 호출 — 큐 항목의 크기·소요시간으로 기록을 남긴다.
    pub(crate) fn record_xfer(&mut self, name: &str, up: bool, ok: bool, size: u64, secs: f64, err: &str) {
        push_record(&mut self.xfer_history, XferRecord {
            name: name.to_string(), up, ok, size, secs, err: err.to_string(), when: now_epoch(),
        });
    }

    /// 전송 히스토리 창(팔레트/도구 메뉴에서 열기).
    pub(crate) fn show_xfer_history(&mut self, ctx: &egui::Context) {
        if !self.xfer_history_open {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        egui::Window::new(tr(lang, "sftp.history"))
            .open(&mut open).collapsible(false).resizable(true).default_size([560.0, 300.0])
            .show(ctx, |ui| {
                if self.xfer_history.is_empty() {
                    ui.weak(tr(lang, "sftp.history.empty"));
                    return;
                }
                if ui.small_button(tr(lang, "sftp.history.clear")).clicked() {
                    self.xfer_history.clear();
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("xfer_hist").num_columns(5).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                        for r in &self.xfer_history {
                            let (mark, color) = if r.ok {
                                ("\u{2713}", crate::theme_ui::OK)
                            } else {
                                ("\u{2717}", crate::theme_ui::ERR)
                            };
                            let row = ui.colored_label(color, format!("{mark} {}", if r.up { "\u{2191}" } else { "\u{2193}" }));
                            if !r.err.is_empty() { row.on_hover_text(&r.err); }
                            ui.label(&r.name);
                            ui.label(crate::humanfmt::human(r.size));
                            ui.label(format!("{:.1}s", r.secs));
                            // 상대 시각(n분 전) — 정렬은 최신순이라 대략만 있으면 된다.
                            ui.weak(crate::humanfmt::human_age(r.when, now_epoch()));
                            ui.end_row();
                        }
                    });
                });
            });
        if !open {
            self.xfer_history_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_caps_and_orders() {
        let mut v = Vec::new();
        for i in 0..(HISTORY_CAP + 5) {
            push_record(&mut v, XferRecord {
                name: format!("f{i}"), up: false, ok: true, size: 1, secs: 0.1,
                err: String::new(), when: 0,
            });
        }
        assert_eq!(v.len(), HISTORY_CAP, "상한 유지");
        assert_eq!(v[0].name, format!("f{}", HISTORY_CAP + 4), "최신이 맨 앞");
    }
}
