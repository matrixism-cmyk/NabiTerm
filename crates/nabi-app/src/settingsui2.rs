//! 설정 — 동작(언어·핫키·복원·셸통합·제어평면 등) 페이지. settingsui에서 분리(파일 크기 규율).

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

/// 동작 페이지.
///
/// 이 페이지만 표(그리드)를 **스스로 조각내 연다.** 설명이 붙는 줄은 설명을 두 칸에
/// 걸쳐 적어야 하는데, 표 안에서는 그렇게 할 수 없어서다 — 칸에 넣으면 그 칸이 설명
/// 길이만큼 넓어져 창 폭이 흐트러진다(사용자 보고 2026-09-05).
///
/// 조각마다 라벨 칸 폭이 같아서(`settingsui::LABEL_W`) 나눠도 줄이 어긋나 보이지 않는다.
pub(crate) fn behavior_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    use crate::settingsui::{grid_seg, help_line};

    grid_seg(ui, "beh_lang", |ui| {
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
            egui::TextEdit::singleline(&mut cfg.appearance.quake_hotkey)
                .hint_text("Control+Backquote"),
        );
        ui.end_row();
    });

    // ── 복원 무리 ────────────────────────────────────────────────
    chk_help(ui, "beh_rws", tr(lang, "settings.restorews"), tr(lang, "settings.restorews.help"),
        &mut cfg.terminal.restore_workspace, true);
    let restore = cfg.terminal.restore_workspace;
    chk_help(ui, "beh_rcmd", tr(lang, "settings.restorecmd"), tr(lang, "settings.restorecmd.help"),
        &mut cfg.terminal.restore_running_command, restore);
    chk_help(ui, "beh_rai", tr(lang, "settings.restoreshaai"), tr(lang, "settings.restoreshaai.help"),
        &mut cfg.terminal.restore_ssh_ai_command, restore);
    // 스위치가 아니라 안내다 — 라벨만 두고 설명은 아래 줄에.
    grid_seg(ui, "beh_rainfo", |ui| {
        ui.label(tr(lang, "settings.restoreai"));
        ui.label("");
        ui.end_row();
    });
    help_line(ui, tr(lang, "settings.restoreai.help"));

    grid_seg(ui, "beh_misc1", |ui| {
        chk(ui, tr(lang, "settings.builtineditor"), &mut cfg.terminal.editor_builtin);
        chk(ui, tr(lang, "update.autocheck"), &mut cfg.terminal.auto_check_update);
        // 확인이 필요한 것들을 한자리에 모은다 — 흩어 두면 "어디서 끄지?"가 된다.
        chk(ui, tr(lang, "settings.confirmclose"), &mut cfg.terminal.confirm_close);
    });
    chk_help(ui, "beh_guard", tr(lang, "settings.guarddangerous"), tr(lang, "settings.guarddangerous.hint"),
        &mut cfg.terminal.guard_dangerous, true);

    grid_seg(ui, "beh_misc2", |ui| {
        chk(ui, tr(lang, "settings.autoreconnect"), &mut cfg.terminal.auto_reconnect);
        // 진단 로그 보관 일수 — 0이면 정리하지 않는다(끄는 길을 화면에도 둔다).
        ui.label(tr(lang, "settings.logkeep"));
        ui.add(
            egui::DragValue::new(&mut cfg.terminal.log_keep_days)
                .range(0..=365)
                .suffix(tr(lang, "settings.logkeepunithint")),
        )
        .on_hover_text(tr(lang, "settings.logkeep.hint"));
        ui.end_row();
        chk(ui, tr(lang, "settings.copyonselect"), &mut cfg.appearance.copy_on_select);
        chk(ui, tr(lang, "settings.visualbell"), &mut cfg.appearance.visual_bell);
        chk(ui, tr(lang, "settings.agentsound"), &mut cfg.terminal.agent_sound);
        chk(ui, tr(lang, "menu.ontop"), &mut cfg.appearance.always_on_top);
        chk(ui, tr(lang, "settings.statusbar"), &mut cfg.appearance.show_statusbar);
        chk(ui, tr(lang, "settings.splash"), &mut cfg.appearance.splash);
        chk(ui, tr(lang, "settings.clock"), &mut cfg.appearance.show_clock);
    });

    // 탭에 pane 번호(#N)를 붙인다. **켤 방법이 없었다** — 설정에는 있는데 화면에
    // 스위치가 없어서 파일을 직접 고쳐야 했다(config-keys 검사로 찾았다).
    // AI 제어에서 `--pane <N>` 을 쓰라고 안내하면서 정작 그 번호를 못 켜고 있었다.
    chk_help(ui, "beh_paneids", tr(lang, "settings.showpaneids"), tr(lang, "settings.showpaneids.hint"),
        &mut cfg.appearance.show_pane_ids, true);

    grid_seg(ui, "beh_paste", |ui| {
        chk(ui, tr(lang, "settings.warnpaste"), &mut cfg.terminal.warn_paste_newline);
        // 개행 경고와 별개 스위치 — 눈에 보이지 않는 문자는 개행 확인을 꺼 둔 사람도 봐야 한다.
        chk(ui, tr(lang, "settings.warnpasteunicode"), &mut cfg.terminal.warn_paste_unicode);
        // 화면을 덮어 그리는 TUI 가 다시 그리기 전에 "스크롤백을 지워라"를 보내는 일이 잦다.
        // 그러면 사람이 올려 보려던 것이 그 순간 사라진다(사용자 보고 2026-08-31).
        chk(ui, tr(lang, "settings.protectscrollback"), &mut cfg.terminal.protect_scrollback);

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
                ui.radio_value(&mut cfg.terminal.control_mode, val.to_string(), tr(lang, key));
            }
        });
        ui.end_row();
        ui.label("OSC 7771");
        ui.checkbox(&mut cfg.terminal.control_allow_osc, "");
        ui.end_row();
    });
    help_line(ui, tr(lang, "settings.control.osc"));
}

/// 표 한 줄짜리 스위치.
fn chk(ui: &mut egui::Ui, label: &str, v: &mut bool) {
    ui.label(label);
    ui.checkbox(v, "");
    ui.end_row();
}

/// 스위치 한 줄 + **두 칸을 가로지르는 설명.**
///
/// 표를 한 조각만 열고 닫은 뒤 설명을 표 밖에 적는다. 설명을 칸 안에 두면 그 칸이
/// 설명 길이만큼 넓어져 창 폭이 흐트러진다(사용자 보고 2026-09-05).
fn chk_help(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    help: &str,
    value: &mut bool,
    enabled: bool,
) {
    crate::settingsui::grid_seg(ui, id, |ui| {
        ui.label(label);
        ui.add_enabled(enabled, egui::Checkbox::without_text(value));
        ui.end_row();
    });
    crate::settingsui::help_line(ui, help);
}

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
    // 양자내성 연결 정책 — 협상 결과를 보고 알리거나 끊는다.
    // 배지 옆이 아니라 여기 두는 까닭: 배지는 지나간 일을 보여 주고, 이것은 앞으로를 정한다.
    ui.label(tr(lang, "settings.kexpolicy"));
    ui.horizontal(|ui| {
        for (val, key) in [
            ("auto", "settings.kexpolicy.auto"),
            ("warn", "settings.kexpolicy.warn"),
            ("require", "settings.kexpolicy.require"),
        ] {
            ui.radio_value(&mut cfg.terminal.ssh_kex_policy, val.to_string(), tr(lang, key))
                .on_hover_text(tr(lang, "settings.kexpolicyhint"));
        }
    });
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
