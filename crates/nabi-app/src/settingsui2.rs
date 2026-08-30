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
    // 확인이 필요한 것들을 한자리에 모은다 — 흩어 두면 "어디서 끄지?"가 된다.
    chk(
        ui,
        tr(lang, "settings.confirmclose"),
        &mut cfg.terminal.confirm_close,
    );
    chk_help(
        ui,
        tr(lang, "settings.guarddangerous"),
        tr(lang, "settings.guarddangerous.hint"),
        &mut cfg.terminal.guard_dangerous,
        true,
    );
    chk(
        ui,
        tr(lang, "settings.autoreconnect"),
        &mut cfg.terminal.auto_reconnect,
    );
    // 진단 로그 보관 일수 — 0이면 정리하지 않는다(끄는 길을 화면에도 둔다).
    ui.label(tr(lang, "settings.logkeep")); ui.add(egui::DragValue::new(&mut cfg.terminal.log_keep_days).range(0..=365).suffix(tr(lang, "settings.logkeepunithint")))
        .on_hover_text(tr(lang, "settings.logkeep.hint"));
    ui.end_row();
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
        tr(lang, "settings.splash"),
        &mut cfg.appearance.splash,
    );
    chk(
        ui,
        tr(lang, "settings.clock"),
        &mut cfg.appearance.show_clock,
    );
    // 탭에 pane 번호(#N)를 붙인다. **켤 방법이 없었다** — 설정에는 있는데 화면에
    // 스위치가 없어서 파일을 직접 고쳐야 했다(config-keys 검사로 찾았다).
    // AI 제어에서 `--pane <N>` 을 쓰라고 안내하면서 정작 그 번호를 못 켜고 있었다.
    chk_help(
        ui,
        tr(lang, "settings.showpaneids"),
        tr(lang, "settings.showpaneids.hint"),
        &mut cfg.appearance.show_pane_ids,
        true,
    );
    chk(
        ui,
        tr(lang, "settings.warnpaste"),
        &mut cfg.terminal.warn_paste_newline,
    );
    // 개행 경고와 별개 스위치 — 눈에 보이지 않는 문자는 개행 확인을 꺼 둔 사람도 봐야 한다.
    chk(
        ui,
        tr(lang, "settings.warnpasteunicode"),
        &mut cfg.terminal.warn_paste_unicode,
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

/// 언어 고르기 목록.
///
/// "System" 은 그것만 봐서는 **무엇으로 정해졌는지 알 수 없다.** 한국어로 나오는데
/// 목록에는 "System" 이라고만 적혀 있으면, 이게 지금 한국어인지 영어인지 확인하려고
/// 굳이 골라 봐야 한다. 그래서 실제로 정해진 언어를 괄호에 적는다.
fn lang_choices() -> [(&'static str, String); 4] {
    // `is_explicit` 은 "이 코드가 언어를 못 박는가"를 답한다. 못 박지 않는 값(system·빈 값)
    // 이면 실제로 무엇이 뽑혔는지 보여 줘야 한다.
    let auto = match nabi_i18n::Lang::is_explicit("system") {
        true => String::new(), // 있을 수 없는 일이지만, 그러면 굳이 덧붙이지 않는다.
        false => format!(" ({})", lang_name(nabi_i18n::Lang::from_code("system"))),
    };
    [
        ("system", format!("System{auto}")),
        ("en", "English".to_string()),
        ("ko", "한국어".to_string()),
        ("ja", "日本語".to_string()),
    ]
}

/// 그 언어를 그 언어로 적은 이름.
fn lang_name(l: Lang) -> &'static str {
    match l {
        Lang::Ko => "한국어",
        Lang::Ja => "日本語",
        _ => "English",
    }
}

/// SSH 페이지 — 접속 유지·통계 경고처럼 "연결 자체"에 걸리는 설정만 모은다
/// (예전에는 만물상 '터미널' 페이지에 섞여 있었다 — 사용자 요청 2026-08-19로 분리).
pub(crate) fn ssh_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.offline"));
    ui.checkbox(&mut cfg.terminal.offline_mode, "")
        .on_hover_text(tr(lang, "settings.offlinehint"));
    ui.end_row();
    ui.label(tr(lang, "settings.publicip"));
    ui.checkbox(&mut cfg.terminal.public_ip_lookup, "")
        .on_hover_text(tr(lang, "settings.publiciphint"));
    ui.end_row();
    ui.label(tr(lang, "settings.redacthist"));
    ui.checkbox(&mut cfg.terminal.redact_history, "")
        .on_hover_text(tr(lang, "settings.redacthisthint"));
    ui.end_row();
    ui.label(tr(lang, "settings.sshtimeout"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.ssh_connect_timeout_secs).range(0..=600).suffix(" s"))
        .on_hover_text(tr(lang, "settings.sshtimeouthint"));
    ui.end_row();
    ui.label(tr(lang, "settings.sshkeepalive"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.ssh_keepalive_secs).range(0..=3600).suffix(" s"))
        .on_hover_text(tr(lang, "settings.sshkeepalivehint"));
    ui.end_row();
    // 서버 상태를 몇 초마다 물을 것인가. 0 이면 묻지 않는다.
    // 스키마 주석에 "기본 3, 0=비활성"이라고 적어 두고도 화면에 없었다.
    ui.label(tr(lang, "settings.statssecs"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.ssh_stats_secs).range(0..=60).suffix(" s"))
        .on_hover_text(tr(lang, "settings.statssecshint"));
    ui.end_row();
    ui.label(tr(lang, "settings.statsalert"));
    ui.add(egui::Slider::new(&mut cfg.terminal.ssh_stats_alert_pct, 50..=100).suffix("%"));
    ui.end_row();
    ui.label(tr(lang, "settings.slowcmd"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.slow_command_secs).range(0..=3600).suffix(" s"))
        .on_hover_text(tr(lang, "settings.slowcmdhint"));
    ui.end_row();
    ui.label(tr(lang, "settings.connhist"));
    ui.checkbox(&mut cfg.terminal.keep_conn_history, "").on_hover_text(tr(lang, "connhist.what"));
    ui.end_row();
}

/// 영문 팁 한글 오버레이 설정(터미널 페이지) — 사전 기반 + 선택적 AI 번역.
pub(crate) fn tip_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.tipoverlay"));
    ui.checkbox(&mut cfg.terminal.tip_overlay, "")
        .on_hover_text(tr(lang, "settings.tipoverlayhint"));
    ui.end_row();
    ui.label(tr(lang, "settings.tipai"));
    ui.add_enabled(cfg.terminal.tip_overlay, egui::Checkbox::new(&mut cfg.terminal.tip_translate_ai, ""))
        .on_hover_text(tr(lang, "settings.tipaihint"));
    ui.end_row();
    // 캐시 파일 경로: 공유 폴더/서버 경로를 지정하면 여러 PC의 번역이 한 곳에 누적된다.
    ui.label(tr(lang, "settings.tipcachepath"));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut cfg.terminal.tip_cache_path)
                .desired_width(240.0)
                .hint_text(r"\\server\share\tipcache.json"),
        )
        .on_hover_text(tr(lang, "settings.tipcachepathhint"));
        if ui.button("\u{1f4c1}").clicked() {
            if let Some(f) = rfd::FileDialog::new().add_filter("json", &["json"]).save_file() {
                cfg.terminal.tip_cache_path = f.to_string_lossy().into_owned();
            }
        }
    });
    ui.end_row();
}
