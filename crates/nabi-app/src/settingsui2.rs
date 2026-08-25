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

fn lang_choices() -> [(&'static str, &'static str); 4] {
    [
        ("system", "System"),
        ("en", "English"),
        ("ko", "한국어"),
        ("ja", "日本語"),
    ]
}

/// SSH 페이지 — 접속 유지·통계 경고처럼 "연결 자체"에 걸리는 설정만 모은다
/// (예전에는 만물상 '터미널' 페이지에 섞여 있었다 — 사용자 요청 2026-08-19로 분리).
pub(crate) fn ssh_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.sshkeepalive"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.ssh_keepalive_secs).range(0..=3600).suffix(" s"))
        .on_hover_text(tr(lang, "settings.sshkeepalivehint"));
    ui.end_row();
    ui.label(tr(lang, "settings.statsalert"));
    ui.add(egui::Slider::new(&mut cfg.terminal.ssh_stats_alert_pct, 50..=100).suffix("%"));
    ui.end_row();
    ui.label(tr(lang, "settings.slowcmd"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.slow_command_secs).range(0..=3600).suffix(" s"))
        .on_hover_text(tr(lang, "settings.slowcmdhint"));
    ui.end_row();
}

/// 전송·SFTP 페이지 — 속도/병렬/무결성/파일명 인코딩 + 다운로드 폴더를 한자리에.
pub(crate) fn transfer_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    sftp_rows(ui, cfg, lang);
    // SFTP 다운로드 기본 폴더(비우면 로컬 창/홈) + 매번 물어보기 여부.
    ui.label(tr(lang, "settings.downloaddir"));
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut cfg.terminal.download_dir)
            .desired_width(220.0)
            .hint_text(tr(lang, "settings.downloaddirhint"));
        ui.add(edit);
        if ui.button("\u{1f4c1}").clicked() {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                cfg.terminal.download_dir = d.to_string_lossy().into_owned();
            }
        }
    });
    ui.end_row();
    // 업로드 권한 정규화 — 빈 값=끄기(기본). auto면 스크립트에 실행 비트.
    ui.label(tr(lang, "settings.uploadmode"));
    ui.add(egui::TextEdit::singleline(&mut cfg.terminal.sftp_upload_mode).desired_width(120.0).hint_text("off / auto / 644"))
        .on_hover_text(tr(lang, "settings.uploadmodehint"));
    ui.end_row();
    ui.label(tr(lang, "settings.downloadask"));
    ui.checkbox(&mut cfg.terminal.download_ask, tr(lang, "settings.downloadaskhint"));
    ui.end_row();
}

/// SFTP 전송·파일명 인코딩 그룹(전송 페이지에서 호출 — settingsui.rs 라인 한도로 분리).
fn sftp_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    // 외부 편집기 — 원격 파일을 밖에서 열 때. 비우면 기존처럼 OS 기본 앱.
    ui.label(tr(lang, "settings.exteditor")); ui.add(egui::TextEdit::singleline(&mut cfg.terminal.external_editor).hint_text("code").desired_width(160.0))
        .on_hover_text(tr(lang, "settings.exteditorhint"));
    ui.end_row();
    ui.label(tr(lang, "settings.speedlimit"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.speed_limit_kbps).suffix(" KB/s")); ui.end_row();
    ui.label(tr(lang, "settings.maxparallel"));
    ui.add(egui::Slider::new(&mut cfg.terminal.max_parallel_transfers, 1..=4));
    ui.end_row();
    ui.label(tr(lang, "settings.verifyhash"));
    ui.checkbox(&mut cfg.terminal.sftp_verify_hash, "").on_hover_text(tr(lang, "settings.verifyhashhint"));
    ui.end_row();
    // SFTP 파일명 인코딩 — v3 서버가 로컬 인코딩 raw 바이트로 보낼 때(한국 서버 CP949 등).
    ui.label(tr(lang, "settings.sftpcharset"));
    egui::ComboBox::from_id_salt("sftp_name_charset")
        .selected_text(if cfg.terminal.sftp_name_charset == "auto" { tr(lang, "settings.sftpcharset.auto").to_string() } else { cfg.terminal.sftp_name_charset.clone() })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.terminal.sftp_name_charset, "auto".into(), tr(lang, "settings.sftpcharset.auto"));
            for e in ["utf8", "euc-kr", "shift_jis", "gbk"] {
                ui.selectable_value(&mut cfg.terminal.sftp_name_charset, e.into(), e);
            }
        })
        .response
        .on_hover_text(tr(lang, "settings.sftpcharsethint"));
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
