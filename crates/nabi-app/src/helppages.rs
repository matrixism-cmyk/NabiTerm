//! 도움말 페이지 본문(정보/단축키/AI 제어) — help.rs의 좌측 내비에서 선택해 그린다.

use nabi_i18n::{tr, Lang};
use std::path::Path;

/// 좌측 내비 카테고리(i18n 키). 인덱스가 페이지 번호.
pub(crate) const HELP_CATS: [&str; 5] = [
    "help.cat.about",
    "help.cat.keys",
    "help.cat.features",
    "help.cat.agent",
    "help.cat.licenses",
];

/// 기능 안내 — 우클릭 컨텍스트 메뉴·팔레트에 숨은 주요 기능을 발견하도록 요약.
pub(crate) fn features_page(ui: &mut egui::Ui, lang: Lang) {
    ui.heading(tr(lang, "help.cat.features"));
    ui.label(tr(lang, "help.feat.intro"));
    ui.add_space(6.0);
    for key in [
        "help.feat.editor",
        "help.feat.browser",
        "help.feat.sftp",
        "help.feat.terminal",
        "help.feat.sessions",
        "help.feat.palette",
    ] {
        ui.label(tr(lang, key));
        ui.add_space(2.0);
    }
}

/// 사용한 주요 오픈소스 (이름, 용도, 라이선스). 전이 의존성 660여 개는 대부분 MIT/Apache-2.0.
const LICENSES: [(&str, &str, &str); 22] = [
    ("alacritty_terminal", "terminal core / VT", "Apache-2.0"),
    (
        "egui · eframe · epaint",
        "GUI framework",
        "MIT OR Apache-2.0",
    ),
    ("wgpu", "GPU rendering backend", "MIT OR Apache-2.0"),
    (
        "Mesa 3D (llvmpipe, 동봉)",
        "software OpenGL fallback",
        "MIT",
    ),
    ("egui_extras", "table widgets", "MIT OR Apache-2.0"),
    ("egui_dock", "docking tabs", "MIT"),
    (
        "epaint_default_fonts",
        "bundled UI fonts",
        "OFL-1.1, UFL-1.0",
    ),
    ("image", "PNG/JPEG/GIF decode", "MIT OR Apache-2.0"),
    ("portable-pty", "local PTY (ConPTY)", "MIT"),
    ("russh · russh-sftp", "SSH / SFTP", "Apache-2.0"),
    ("suppaftp", "FTP client", "MIT OR Apache-2.0"),
    ("tokio", "async runtime", "MIT"),
    (
        "encoding_rs · chardetng",
        "text encoding",
        "Apache-2.0/MIT, BSD-3",
    ),
    ("syntect · fancy-regex", "syntax highlight", "MIT"),
    ("ttf-parser", "font enumeration", "MIT OR Apache-2.0"),
    ("memmap2", "large-file viewer", "MIT OR Apache-2.0"),
    ("arboard", "clipboard", "MIT OR Apache-2.0"),
    ("rfd", "native file dialog", "MIT"),
    (
        "argon2 · aes-gcm · zeroize",
        "vault crypto",
        "MIT OR Apache-2.0",
    ),
    (
        "serde · serde_json · toml",
        "config/serialize",
        "MIT OR Apache-2.0",
    ),
    ("chrono · directories", "time / paths", "MIT OR Apache-2.0"),
    ("option-ext (via directories)", "transitive", "MPL-2.0"),
];

/// (단축키, 설명 i18n 키). 설명을 함께 보여 무슨 기능인지 알 수 있게 한다.
pub(crate) const KEYS: [(&str, &str); 24] = [
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
    open_logs: &mut bool,
) {
    ui.heading("\u{1f98b} nabiTerm (나비텀)");
    ui.label(tr(lang, "help.desc"));
    ui.label(concat!("v", env!("CARGO_PKG_VERSION")));
    ui.add_space(6.0);
    // 프로그램 소개 + 공개 저장소/연락처(오픈소스 전환 후 문의 창구를 앱 안에서 안내).
    ui.label(tr(lang, "help.about.intro"));
    ui.add_space(6.0);
    ui.label(tr(lang, "help.about.oss"));
    ui.horizontal(|ui| {
        if ui.link("github.com/matrixism-cmyk/NabiTerm").clicked() {
            crate::paneurl::os_open("https://github.com/matrixism-cmyk/NabiTerm");
        }
        ui.label("\u{b7}");
        if ui.link(tr(lang, "help.about.issues")).clicked() {
            crate::paneurl::os_open("https://github.com/matrixism-cmyk/NabiTerm/issues");
        }
    });
    ui.horizontal(|ui| {
        ui.label(format!("{}:", tr(lang, "help.about.contact")));
        if ui.link("matrixism@gmail.com").clicked() {
            crate::paneurl::os_open("mailto:matrixism@gmail.com");
        }
    });
    ui.weak(tr(lang, "help.about.security"));
    // 만든 곳·문의처 — 둘 다 실제로 눌러 갈 수 있게 링크로 둔다(글자만 적어 두면 찾아가야 한다).
    ui.horizontal(|ui| {
        ui.label(format!("{}:", tr(lang, "help.about.madeby")));
        if ui.link("나비소리 (nabisori.kr)").clicked() {
            crate::paneurl::os_open("https://nabisori.kr");
        }
    });
    ui.horizontal(|ui| {
        ui.label(format!("{}:", tr(lang, "help.about.inquiry")));
        if ui.link("AI메타버스센터 (metahubs.kr)").clicked() {
            crate::paneurl::os_open("https://metahubs.kr");
        }
    });
    ui.add_space(4.0);
    ui.weak(tr(lang, "help.about.funding"));
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
        // 진단 로그 — 앱 안에서 바로 보여 준다. 폴더만 열어 주면 어느 파일인지, 어디부터가
        // 문제인지 사용자가 골라야 한다. 원격 지원에서 그건 또 하나의 숙제다.
        ui.add_space(4.0);
        if ui.button(tr(lang, "help.diaglogs")).clicked() {
            *open_logs = true;
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
    for k in [
        "help.tip.urlopen",
        "help.tip.pathjump",
        "help.tip.runterm",
        "help.tip.mpaste",
    ] {
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
///
/// `auto_update`는 AI CLI 자동 업데이트 설정 — 바뀌면 `saved`를 세워 호출부가 저장하게 한다.
pub(crate) fn agent_page(
    ui: &mut egui::Ui,
    lang: Lang,
    copy: &mut bool,
    save: &mut bool,
    auto_update: &mut bool,
    saved: &mut bool,
) {
    ui.heading(tr(lang, "help.agent.title"));
    ui.label(tr(lang, "help.agent.intro")); // 폭 제한된 본문이라 자동 줄바꿈.
    ui.add_space(8.0);
    *saved |= crate::aiclipage::ai_cli_manager(ui, lang, auto_update);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.strong(tr(lang, "help.agent.examples"));
    ui.add_space(2.0);
    // 명령 ↔ 설명 2열 표(가로로 뻗지 않게 짧은 표기 사용 — 전체 문법은 복사본 MD에).
    egui::Grid::new("help_agent_cmds")
        .num_columns(2)
        .spacing([14.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (cmd, key) in CMDS {
                ui.monospace(cmd);
                ui.label(tr(lang, key));
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    ui.label(tr(lang, "help.agent.perm"));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .button(format!("\u{1f4cb} {}", tr(lang, "help.agent.copy")))
            .clicked()
        {
            *copy = true;
        }
        if ui
            .button(format!("\u{1f4be} {}", tr(lang, "help.agent.save")))
            .clicked()
        {
            *save = true;
        }
    });
    ui.add_space(4.0);
    ui.weak(tr(lang, "help.agent.hint"));
}

/// 오픈소스 라이선스 페이지: 주요 구성요소(이름·용도·라이선스) 표 + 안내.
pub(crate) fn licenses_page(ui: &mut egui::Ui, lang: Lang) {
    ui.heading(tr(lang, "help.cat.licenses"));
    ui.label(tr(lang, "help.lic.intro"));
    ui.add_space(8.0);
    egui::Grid::new("help_licenses")
        .num_columns(3)
        .spacing([14.0, 5.0])
        .striped(true)
        .show(ui, |ui| {
            for (name, use_, lic) in LICENSES {
                ui.strong(name);
                ui.label(use_);
                ui.monospace(lic);
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    ui.weak(tr(lang, "help.lic.note"));
}
