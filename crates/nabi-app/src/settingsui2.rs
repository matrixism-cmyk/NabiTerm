//! 설정 — 동작(언어·핫키·복원·셸통합·제어평면 등) 페이지. settingsui에서 분리(파일 크기 규율).

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

pub(crate) fn behavior_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.language"));
    egui::ComboBox::from_id_salt("set_lang")
        .selected_text(cfg.appearance.language.clone())
        .show_ui(ui, |ui| {
            for (code, label) in lang_choices() {
                ui.selectable_value(&mut cfg.appearance.language, code.to_owned(), label);
            }
        });
    ui.end_row();

    ui.label(tr(lang, "settings.quakehotkey"));
    ui.add(
        egui::TextEdit::singleline(&mut cfg.appearance.quake_hotkey).hint_text("Control+Backquote"),
    );
    ui.end_row();

    let chk = |ui: &mut egui::Ui, label: &str, v: &mut bool| {
        ui.label(label);
        ui.checkbox(v, "");
        ui.end_row();
    };
    chk_help(
        ui,
        tr(lang, "settings.restorews"),
        tr(lang, "settings.restorews.help"),
        &mut cfg.terminal.restore_workspace,
        true,
    );
    chk_help(
        ui,
        tr(lang, "settings.restorecmd"),
        tr(lang, "settings.restorecmd.help"),
        &mut cfg.terminal.restore_running_command,
        cfg.terminal.restore_workspace,
    );
    chk_help(
        ui,
        tr(lang, "settings.restoreshaai"),
        tr(lang, "settings.restoreshaai.help"),
        &mut cfg.terminal.restore_ssh_ai_command,
        cfg.terminal.restore_workspace,
    );
    ui.label(tr(lang, "settings.restoreai"));
    ui.add(
        egui::Label::new(egui::RichText::new(tr(lang, "settings.restoreai.help")).weak()).wrap(),
    );
    ui.end_row();
    chk(
        ui,
        tr(lang, "settings.builtineditor"),
        &mut cfg.terminal.editor_builtin,
    );
    chk(
        ui,
        tr(lang, "update.autocheck"),
        &mut cfg.terminal.auto_check_update,
    );
    chk(
        ui,
        tr(lang, "settings.confirmclose"),
        &mut cfg.terminal.confirm_close,
    );
    chk(
        ui,
        tr(lang, "settings.autoreconnect"),
        &mut cfg.terminal.auto_reconnect,
    );
    chk(
        ui,
        tr(lang, "settings.copyonselect"),
        &mut cfg.appearance.copy_on_select,
    );
    chk(
        ui,
        tr(lang, "settings.visualbell"),
        &mut cfg.appearance.visual_bell,
    );
    chk(
        ui,
        tr(lang, "settings.agentsound"),
        &mut cfg.terminal.agent_sound,
    );
    chk(
        ui,
        tr(lang, "menu.ontop"),
        &mut cfg.appearance.always_on_top,
    );
    chk(
        ui,
        tr(lang, "settings.statusbar"),
        &mut cfg.appearance.show_statusbar,
    );
    chk(
        ui,
        tr(lang, "settings.clock"),
        &mut cfg.appearance.show_clock,
    );
    chk(
        ui,
        tr(lang, "settings.warnpaste"),
        &mut cfg.terminal.warn_paste_newline,
    );

    // 셸 통합: PowerShell 프로필에 OSC 133/7 스니펫 설치(명령 경계·종료코드·cwd).
    ui.label(tr(lang, "settings.shellinteg"));
    ui.horizontal(|ui| {
        let id = egui::Id::new("shellinteg_msg");
        if ui.button(tr(lang, "settings.shellinteg.install")).clicked() {
            let msg = match crate::shellinteg::install() {
                Ok(m) => format!("\u{2713} {m}"),
                Err(e) => format!("\u{2715} {e}"),
            };
            ui.data_mut(|d| d.insert_temp(id, msg));
        }
        if let Some(msg) = ui.data(|d| d.get_temp::<String>(id)) {
            ui.weak(msg);
        }
    });
    ui.end_row();

    // 에이전트 제어 평면: pane 내 프로세스가 nabiTerm을 제어(off/ask/on).
    ui.label(tr(lang, "settings.control"));
    ui.horizontal(|ui| {
        for (val, key) in [
            ("off", "settings.control.off"),
            ("ask", "settings.control.ask"),
            ("on", "settings.control.on"),
        ] {
            ui.radio_value(
                &mut cfg.terminal.control_mode,
                val.to_string(),
                tr(lang, key),
            );
        }
    });
    ui.end_row();
    ui.label("OSC 7771");
    ui.label(tr(lang, "settings.control.osc"));
    ui.checkbox(&mut cfg.terminal.control_allow_osc, "");
    ui.end_row();
}

fn chk_help(ui: &mut egui::Ui, label: &str, help: &str, value: &mut bool, enabled: bool) {
    ui.vertical(|ui| {
        ui.label(label);
        ui.add(egui::Label::new(egui::RichText::new(help).weak().small()).wrap());
    });
    ui.add_enabled(enabled, egui::Checkbox::without_text(value));
    ui.end_row();
}

fn lang_choices() -> [(&'static str, &'static str); 4] {
    [
        ("system", "System"),
        ("en", "English"),
        ("ko", "한국어"),
        ("ja", "日本語"),
    ]
}
