//! **직렬 콘솔 열기** 창 — 포트와 속도를 고른다.
//!
//! ## 왜 목록을 열 때마다 다시 읽나
//!
//! USB 직렬 어댑터는 꽂았다 뺐다 한다. 창을 여는 그 순간이 사용자가 방금 케이블을 꽂고
//! 온 순간일 때가 많다 — 캐시해 두면 "분명히 꽂았는데 목록에 없다"가 된다.
//!
//! ## 왜 속도를 크게 물어보나
//!
//! 콘솔에서 속도를 잘못 잡으면 연결은 되는데 **글자가 깨져 나온다.** 그때 사용자는 대개
//! 케이블이나 장비를 의심하지 대화상자에서 고른 숫자를 의심하지 않는다. 그래서 기본을
//! 장비 콘솔의 사실상 표준(9600 8N1)으로 두고, 지금 무엇으로 붙었는지 pane 제목에 적는다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_serial::{PortInfo, SerialCfg, BAUDS};

/// 창이 들고 있는 것.
pub(crate) struct SerialOpen {
    /// 창을 열 때 읽은 포트 목록.
    pub ports: Vec<PortInfo>,
    /// 고른 포트 이름(목록이 비었으면 사용자가 직접 적을 수 있다).
    pub port: String,
    pub baud: u32,
    /// `8N1`.
    pub frame: String,
    /// 세션 목록에 남길 이름(비우면 남기지 않는다).
    ///
    /// 콘솔은 **같은 장비에 반복해서** 붙는다. 매번 포트와 속도를 다시 고르게 하면
    /// 그 반복이 그대로 비용이 된다 — 그런데 세션 저장을 다른 화면으로 미루면 아무도
    /// 안 한다. 그래서 붙는 자리에서 함께 남긴다.
    pub save_as: String,
}

impl NabiApp {
    /// 창을 연다 — 그때마다 포트를 다시 읽는다.
    pub(crate) fn open_serial_dialog(&mut self) {
        let ports = nabi_serial::ports();
        let port = ports.first().map(|p| p.name.clone()).unwrap_or_default();
        let cfg = SerialCfg::default();
        self.serial_open =
            Some(SerialOpen { ports, port, baud: cfg.baud, frame: cfg.frame(), save_as: String::new() });
    }

    /// 창을 그린다.
    pub(crate) fn show_serial_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.serial_open.take() else { return };
        let lang = self.lang;
        let (mut open, mut go) = (true, false);
        egui::Window::new(tr(lang, "serial.title"))
            .open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(tr(lang, "serial.about"));
                ui.add_space(6.0);
                egui::Grid::new("serial_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(tr(lang, "serial.port"));
                    port_picker(ui, lang, &mut st);
                    ui.end_row();

                    ui.label(tr(lang, "serial.baud"));
                    egui::ComboBox::from_id_salt("serial_baud")
                        .selected_text(st.baud.to_string())
                        .show_ui(ui, |ui| {
                            for b in BAUDS {
                                ui.selectable_value(&mut st.baud, *b, b.to_string());
                            }
                        });
                    ui.end_row();

                    ui.label(tr(lang, "serial.frame"));
                    ui.add(egui::TextEdit::singleline(&mut st.frame).desired_width(70.0))
                        .on_hover_text(tr(lang, "serial.frame.hint"));
                    ui.end_row();

                    ui.label(tr(lang, "serial.saveas"));
                    ui.add(
                        egui::TextEdit::singleline(&mut st.save_as)
                            .hint_text(tr(lang, "serial.saveas.hint"))
                            .desired_width(220.0),
                    );
                    ui.end_row();
                });
                // 표기가 이상하면 **누르기 전에** 알려 준다. 눌러 놓고 실패를 보는 것보다
                // 낫다 — 직렬은 실패해도 원인이 여럿이라 하나라도 미리 지워 주는 편이 좋다.
                let frame_ok = SerialCfg::parse_frame(&st.frame).is_some();
                if !frame_ok {
                    ui.colored_label(crate::theme_ui::ERR, tr(lang, "serial.badframe"));
                }
                if st.ports.is_empty() {
                    ui.weak(tr(lang, "serial.noports"));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = frame_ok && !st.port.trim().is_empty();
                    go = ui.add_enabled(ok, egui::Button::new(tr(lang, "serial.connect"))).clicked();
                });
            });
        if go {
            self.save_serial_session(&st);
            self.spawn_serial(st.port.trim().to_string(), st.baud, st.frame.clone());
            return; // 창은 닫는다(상태를 되돌려 놓지 않는다).
        }
        if open {
            self.serial_open = Some(st);
        }
    }

    /// 이름을 적었으면 세션 목록에 남긴다.
    ///
    /// 같은 이름이 있으면 덮어쓴다(빠른 연결의 저장과 같은 규칙) — 고쳐 저장하는 것이
    /// 흔한 일이라 매번 지우고 다시 적게 하면 번거롭다.
    fn save_serial_session(&mut self, st: &SerialOpen) {
        let name = st.save_as.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.sessions.remove(&name);
        self.sessions.add(nabi_session::SavedSession {
            name: name.clone(),
            folder: None,
            kind: nabi_session::SessionKind::Serial {
                port: st.port.trim().to_string(),
                baud: st.baud,
                frame: st.frame.clone(),
            },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        });
        self.save_sessions();
        self.notify = Some((
            format!("{} {name}", tr(self.lang, "qc.save")),
            std::time::Instant::now(),
        ));
    }
}

/// 포트 고르기 — 목록이 있으면 고르고, 없으면 직접 적는다.
///
/// 목록이 비었다고 입력을 막지 않는다. 드라이버가 열거에 안 잡히는 어댑터가 있고,
/// 그때도 이름을 알면 열 수 있다.
fn port_picker(ui: &mut egui::Ui, lang: nabi_i18n::Lang, st: &mut SerialOpen) {
    if st.ports.is_empty() {
        ui.add(egui::TextEdit::singleline(&mut st.port).hint_text("COM1").desired_width(220.0));
        return;
    }
    egui::ComboBox::from_id_salt("serial_port")
        .selected_text(st.port.clone())
        .width(220.0)
        .show_ui(ui, |ui| {
            for p in &st.ports {
                // 무엇이 꽂혀 있는지 함께 보여 준다 — 어댑터가 둘이면 이름만으로는 못 고른다.
                let label = match p.detail.is_empty() {
                    true => p.name.clone(),
                    false => format!("{} — {}", p.name, p.detail),
                };
                ui.selectable_value(&mut st.port, p.name.clone(), label);
            }
        });
    let _ = lang;
}
