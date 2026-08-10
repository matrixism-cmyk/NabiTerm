//! 도움말 페이지 본문(정보/단축키/AI 제어) — help.rs의 좌측 내비에서 선택해 그린다.

use nabi_i18n::{tr, Lang};
use std::path::Path;

/// 좌측 내비 카테고리(i18n 키). 인덱스가 페이지 번호.
pub(crate) const HELP_CATS: [&str; 5] =
    ["help.cat.about", "help.cat.keys", "help.cat.features", "help.cat.agent", "help.cat.licenses"];

/// 기능 안내 — 우클릭 컨텍스트 메뉴·팔레트에 숨은 주요 기능을 발견하도록 요약.
pub(crate) fn features_page(ui: &mut egui::Ui, lang: Lang) {
    ui.heading(tr(lang, "help.cat.features"));
    ui.label(tr(lang, "help.feat.intro"));
    ui.add_space(6.0);
    for key in ["help.feat.editor", "help.feat.browser", "help.feat.sftp", "help.feat.terminal", "help.feat.sessions", "help.feat.palette"] {
        ui.label(tr(lang, key));
        ui.add_space(2.0);
    }
}

/// 사용한 주요 오픈소스 (이름, 용도, 라이선스). 전이 의존성 660여 개는 대부분 MIT/Apache-2.0.
const LICENSES: [(&str, &str, &str); 22] = [
    ("alacritty_terminal", "terminal core / VT", "Apache-2.0"),
    ("egui · eframe · epaint", "GUI framework", "MIT OR Apache-2.0"),
    ("wgpu", "GPU rendering backend", "MIT OR Apache-2.0"),
    ("Mesa 3D (llvmpipe, 동봉)", "software OpenGL fallback", "MIT"),
    ("egui_extras", "table widgets", "MIT OR Apache-2.0"),
    ("egui_dock", "docking tabs", "MIT"),
    ("epaint_default_fonts", "bundled UI fonts", "OFL-1.1, UFL-1.0"),
    ("image", "PNG/JPEG/GIF decode", "MIT OR Apache-2.0"),
    ("portable-pty", "local PTY (ConPTY)", "MIT"),
    ("russh · russh-sftp", "SSH / SFTP", "Apache-2.0"),
    ("suppaftp", "FTP client", "MIT OR Apache-2.0"),
    ("tokio", "async runtime", "MIT"),
    ("encoding_rs · chardetng", "text encoding", "Apache-2.0/MIT, BSD-3"),
    ("syntect · fancy-regex", "syntax highlight", "MIT"),
    ("ttf-parser", "font enumeration", "MIT OR Apache-2.0"),
    ("memmap2", "large-file viewer", "MIT OR Apache-2.0"),
    ("arboard", "clipboard", "MIT OR Apache-2.0"),
    ("rfd", "native file dialog", "MIT"),
    ("argon2 · aes-gcm · zeroize", "vault crypto", "MIT OR Apache-2.0"),
    ("serde · serde_json · toml", "config/serialize", "MIT OR Apache-2.0"),
    ("chrono · directories", "time / paths", "MIT OR Apache-2.0"),
    ("option-ext (via directories)", "transitive", "MPL-2.0"),
];

/// (단축키, 설명 i18n 키). 설명을 함께 보여 무슨 기능인지 알 수 있게 한다.
const KEYS: [(&str, &str); 24] = [
    ("Ctrl+Shift+T", "help.key.newtab"),
    ("Ctrl+Shift+N", "help.key.connect"),
    ("Ctrl+Shift+W", "help.key.close"),
    ("Ctrl+Shift+Q", "help.key.quit"),
    ("Ctrl+Shift+D", "help.key.dup"),
    ("Ctrl+Shift+E", "help.key.browser"),
    ("Ctrl+Shift+P", "help.key.palette"),
    ("Ctrl+Shift+\\  /  -", "help.key.split"),
    ("Ctrl+Shift+Z", "help.key.zoom"),
    ("Ctrl+Shift+M", "help.key.broadcast"),
    ("Ctrl+Shift+K", "help.key.clear"),
    ("Ctrl+Shift+B", "help.key.statusbar"),
    ("Ctrl+Shift+A", "help.key.selectall"),
    ("Ctrl+Shift+C  /  Ctrl+Insert", "help.key.copy"),
    ("Ctrl+Shift+V  /  Shift+Insert", "help.key.paste"),
    ("Ctrl + =  /  -  /  0", "help.key.fontsize"),
    ("Ctrl+Shift+0", "help.key.fontreset"),
    ("Ctrl+F  /  F3", "help.key.find"),
    ("F11", "help.key.fullscreen"),
    ("Ctrl+`", "help.key.quake"),
    ("Alt+1~9", "help.key.tabn"),
    ("Ctrl+PgUp  /  PgDn", "help.key.tabcycle"),
    ("Shift+PgUp  /  PgDn", "help.key.scroll"),
    ("Shift+Home  /  End", "help.key.scrollends"),
];

/// 정보 페이지: 앱 이름·버전·설명 + 업데이트 확인/적용 + 설정 폴더 링크.
pub(crate) fn about_page(
    ui: &mut egui::Ui,
    lang: Lang,
    cfg_dir: Option<&Path>,
    updater: &nabi_release::UpdateChecker,
    update_quit: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    ui.heading("\u{1f98b} nabiTerm (나비텀)");
    ui.label(tr(lang, "help.desc"));
    ui.label(concat!("v", env!("CARGO_PKG_VERSION")));
    ui.add_space(8.0);
    crate::updateui::update_section(ui, lang, updater, update_quit); // 업데이트(설정에서 이동).
    ui.add_space(8.0);
    ui.separator();
    ui.label(tr(lang, "help.privacy")); // 폐쇄망/보안 환경 도입 요건 명시.
    ui.add_space(8.0);
    ui.label(tr(lang, "help.cfgfolder"));
    if let Some(dir) = cfg_dir {
        if ui.link(dir.to_string_lossy()).clicked() {
            let _ = std::process::Command::new("explorer").arg(dir).spawn();
        }
        // 진단 로그(crash.log·회전 로그) 폴더 — 사후 문제 분석용(G1).
        ui.add_space(4.0);
        if ui.button(tr(lang, "help.diaglogs")).clicked() {
            let logs = dir.join("logs");
            let _ = std::fs::create_dir_all(&logs);
            let _ = std::process::Command::new("explorer").arg(&logs).spawn();
        }
    }
}

/// 단축키 페이지: (키, 설명) 2열 표.
pub(crate) fn shortcut_page(ui: &mut egui::Ui, lang: Lang) {
    egui::Grid::new("help_keys")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (combo, key) in KEYS {
                ui.monospace(combo);
                ui.label(tr(lang, key));
                ui.end_row();
            }
        });
    ui.add_space(10.0);
    ui.strong(tr(lang, "help.mousetips"));
    for k in ["help.tip.urlopen", "help.tip.pathjump", "help.tip.runterm", "help.tip.mpaste"] {
        ui.label(format!("\u{2022} {}", tr(lang, k)));
    }
}

/// 주요 명령(짧은 표기, 설명 i18n 키) — 화면에서 무슨 명령인지 바로 알 수 있게.
const CMDS: [(&str, &str); 7] = [
    ("nabi cli list", "help.cmd.list"),
    ("nabi cli spawn …", "help.cmd.spawn"),
    ("nabi cli send …", "help.cmd.send"),
    ("nabi cli capture …", "help.cmd.capture"),
    ("nabi cli wait …", "help.cmd.wait"),
    ("nabi cli notify …", "help.cmd.notify"),
    ("nabi cli kill …", "help.cmd.kill"),
];

/// AI 제어 페이지: 요약 + 주요 명령(설명 포함) + 사용설명 복사/저장 버튼(out 플래그로 신호).
pub(crate) fn agent_page(ui: &mut egui::Ui, lang: Lang, copy: &mut bool, save: &mut bool) {
    ui.heading(tr(lang, "help.agent.title"));
    ui.label(tr(lang, "help.agent.intro")); // 폭 제한된 본문이라 자동 줄바꿈.
    ui.add_space(8.0);
    ai_cli_manager(ui, lang);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.strong(tr(lang, "help.agent.examples"));
    ui.add_space(2.0);
    // 명령 ↔ 설명 2열 표(가로로 뻗지 않게 짧은 표기 사용 — 전체 문법은 복사본 MD에).
    egui::Grid::new("help_agent_cmds").num_columns(2).spacing([14.0, 6.0]).striped(true).show(
        ui,
        |ui| {
            for (cmd, key) in CMDS {
                ui.monospace(cmd);
                ui.label(tr(lang, key));
                ui.end_row();
            }
        },
    );
    ui.add_space(8.0);
    ui.label(tr(lang, "help.agent.perm"));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button(format!("\u{1f4cb} {}", tr(lang, "help.agent.copy"))).clicked() {
            *copy = true;
        }
        if ui.button(format!("\u{1f4be} {}", tr(lang, "help.agent.save"))).clicked() {
            *save = true;
        }
    });
    ui.add_space(4.0);
    ui.weak(tr(lang, "help.agent.hint"));
}

/// 설치된 AI CLI와 버전을 보여주고 공식 설치 관리자를 연다.
fn ai_cli_manager(ui: &mut egui::Ui, lang: Lang) {
    let words = match lang {
        Lang::Ko => ("AI CLI 관리", "미설치", "새로고침", "설치", "제거"),
        Lang::Ja => ("AI CLI 管理", "未インストール", "更新", "インストール", "削除"),
        Lang::En => ("AI CLI manager", "Not installed", "Refresh", "Install", "Remove"),
    };
    let key = egui::Id::new("ai_cli_status");
    let mut statuses = ui.ctx().data(|d| d.get_temp::<Vec<crate::aicli::CliStatus>>(key));
    ui.horizontal(|ui| {
        ui.strong(words.0);
        if ui.small_button(format!("↻ {}", words.2)).clicked() { statuses = None; }
    });
    let statuses = statuses.unwrap_or_else(|| {
        let v = crate::aicli::detect_all();
        ui.ctx().data_mut(|d| d.insert_temp(key, v.clone()));
        v
    });
    egui::Grid::new("ai_cli_manager").num_columns(4).spacing([12.0, 5.0]).striped(true).show(ui, |ui| {
        for cli in statuses {
            ui.strong(cli.name);
            if cli.installed() {
                ui.colored_label(crate::theme_ui::OK, cli.version.as_deref().unwrap_or("Installed"))
                    .on_hover_text(cli.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
                ui.monospace(cli.command);
                if ui.small_button(words.4).clicked() { let _ = crate::aicli::launch_action(cli.id, true); }
            } else {
                ui.weak(words.1); ui.monospace(cli.command);
                if ui.small_button(words.3).clicked() { let _ = crate::aicli::launch_action(cli.id, false); }
            }
            ui.end_row();
        }
    });
}

/// 오픈소스 라이선스 페이지: 주요 구성요소(이름·용도·라이선스) 표 + 안내.
pub(crate) fn licenses_page(ui: &mut egui::Ui, lang: Lang) {
    ui.heading(tr(lang, "help.cat.licenses"));
    ui.label(tr(lang, "help.lic.intro"));
    ui.add_space(8.0);
    egui::Grid::new("help_licenses").num_columns(3).spacing([14.0, 5.0]).striped(true).show(
        ui,
        |ui| {
            for (name, use_, lic) in LICENSES {
                ui.strong(name);
                ui.label(use_);
                ui.monospace(lic);
                ui.end_row();
            }
        },
    );
    ui.add_space(8.0);
    ui.weak(tr(lang, "help.lic.note"));
}
