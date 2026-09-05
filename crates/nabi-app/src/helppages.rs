//! 도움말 페이지 본문(정보/단축키/AI 제어) — help.rs의 좌측 내비에서 선택해 그린다.

use nabi_i18n::{tr, Lang};
use std::path::Path;

/// 좌측 내비 카테고리(i18n 키). 인덱스가 페이지 번호.
pub(crate) const HELP_CATS: [&str; 6] = [
    "help.cat.about",
    "help.cat.keys",
    "help.cat.features",
    "help.cat.agent",
    "help.cat.network",
    "help.cat.licenses",
];

/// **이 프로그램이 연결하는 곳** — 보안 검토에서 반드시 나오는 질문에 대한 답.
///
/// 목록은 `egress::ALL`에서 읽는다. 문서에만 적어 두면 코드가 바뀔 때 **조용히 틀린
/// 답**이 되기 때문이다. 새 호출을 넣는 사람은 그 표에 한 줄을 더해야 한다.
pub(crate) fn network_page(ui: &mut egui::Ui, lang: Lang, offline: bool) {
    ui.heading(tr(lang, "help.cat.network"));
    ui.label(tr(lang, "help.net.intro"));
    ui.add_space(6.0);
    if offline {
        ui.colored_label(crate::theme_ui::OK, tr(lang, "help.net.offlineon"));
        ui.add_space(6.0);
    }
    egui::Grid::new("help_egress").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
        ui.strong(tr(lang, "help.net.host"));
        ui.strong(tr(lang, "help.net.why"));
        ui.strong(tr(lang, "help.net.when"));
        ui.end_row();
        for e in crate::egress::ALL {
            ui.monospace(e.host);
            ui.label(tr(lang, e.why));
            // 시키지 않은 호출은 오프라인 모드가 막는다 — 그 사실이 표에서 보여야 한다.
            match e.unattended {
                true => ui.label(tr(lang, "help.net.auto")),
                false => ui.weak(tr(lang, "help.net.manual")),
            };
            ui.end_row();
        }
    });
}

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
    // 무인 설치 — 여러 대에 밀어 넣는 사람만 찾는 정보라 접어 둔다.
    ui.add_space(8.0);
    ui.collapsing(tr(lang, "help.silent.title"), |ui| {
        ui.label(tr(lang, "help.silent.intro"));
        ui.add_space(4.0);
        for line in [
            "nabiTerm-setup.exe /VERYSILENT /NOLAUNCH",
            "nabiTerm-setup.exe /VERYSILENT /ALLUSERS /NOLAUNCH",
            r#"nabiTerm-setup.exe /VERYSILENT /DIR="D:\Apps\nabiTerm" /TASKS="desktopicon""#,
            r#"nabiTerm-setup.exe /VERYSILENT /LOG="C:\temp\nabi-install.log""#,
        ] {
            if ui.add(egui::Label::new(egui::RichText::new(line).monospace()).sense(egui::Sense::click())).clicked() {
                ui.ctx().copy_text(line.to_string());
            }
        }
        ui.add_space(4.0);
        ui.label(tr(lang, "help.silent.note"));
    });
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
    // 문의처는 두 기관이다 — 한 줄에 몰아 적으면 어느 주소가 어디인지 알 수 없다.
    ui.horizontal(|ui| {
        ui.label(format!("{}:", tr(lang, "help.about.inquiry")));
        if ui.link("AI메타버스센터 (aimeta.or.kr)").clicked() {
            crate::paneurl::os_open("https://aimeta.or.kr");
        }
    });
    ui.horizontal(|ui| {
        ui.label(" "); // 위 줄과 세로로 맞춘다(라벨을 두 번 적지 않는다).
        if ui.link("서남권가상융합산업허브센터 (metahubs.kr)").clicked() {
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
/// 도움말에 보여 줄 주요 명령 — **전부가 아니라 자주 쓰는 것들**이다.
/// 전체 목록과 문법은 아래 "복사"·"MD로 저장" 이 주는 설명서에 있다(`agentguide.rs`).
///
/// 왼쪽 낱말은 실제 동사여야 한다 — 아래 시험이 대조한다. 없는 명령을 적어 두면
/// AI 가 그것을 부르고 실패하고, 실패한 AI 는 우리 프로그램이 고장 났다고 판단한다.
/// 명령 한 칸 — 누르면 true. 눌러서 복사한다는 것을 알 수 있게 손 모양으로 바꾼다.
fn cmd_button(ui: &mut egui::Ui, lang: Lang, cmd: &str) -> bool {
    let r = ui.add(
        egui::Label::new(egui::RichText::new(cmd).monospace())
            .sense(egui::Sense::click()),
    );
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r.on_hover_text(tr(lang, "help.agent.clickcopy"))
        .clicked()
}

const CMDS: &[(&str, &str)] = &[
    // pane 을 다루는 기본 고리 — 열고, 보내고, 읽고, 기다린다.
    ("nabi cli list", "help.cmd.list"),
    ("nabi cli spawn …", "help.cmd.spawn"),
    ("nabi cli send …", "help.cmd.send"),
    ("nabi cli capture …", "help.cmd.capture"),
    ("nabi cli wait …", "help.cmd.wait"),
    ("nabi cli notify …", "help.cmd.notify"),
    ("nabi cli kill …", "help.cmd.kill"),
    // 화면으로 확인하는 길 — 글로는 알 수 없는 것을 본다.
    ("nabi cli screenshot …", "help.cmd.screenshot"),
    ("nabi cli scroll …", "help.cmd.scroll"),
    ("nabi cli history …", "help.cmd.history"),
    // 프로그램 자체를 다루는 것 — 되돌릴 수 없으니 뜻을 밝혀 둔다.
    ("nabi cli quit", "help.cmd.quit"),
    ("nabi cli restart", "help.cmd.restart"),
    ("nabi cli update …", "help.cmd.update"),
];

/// AI 제어 페이지: 요약 + 주요 명령(설명 포함) + 사용설명 복사/저장 버튼(out 플래그로 신호).
///
/// **설치 기능은 여기 없다.** AI CLI를 깔고 지우는 일은 환경 관리자(도구 메뉴)로 옮겼다 —
/// 도움말은 읽는 곳이라, 반년 뒤에 다시 깔려고 이곳을 뒤지는 사람은 없기 때문이다
/// (사용자와 2026-08-25에 내린 결론). 대신 읽다가 바로 갈 수 있게 버튼 하나를 둔다.
pub(crate) fn agent_page(
    ui: &mut egui::Ui,
    lang: Lang,
    copy: &mut bool,
    save: &mut bool,
    open_env: &mut bool,
    copy_cmd: &mut Option<String>,
) {
    ui.heading(tr(lang, "help.agent.title"));
    ui.label(tr(lang, "help.agent.intro")); // 폭 제한된 본문이라 자동 줄바꿈.
    ui.add_space(8.0);
    if ui.button(tr(lang, "help.agent.openenv")).clicked() {
        *open_env = true;
    }
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.strong(tr(lang, "help.agent.examples"));
    ui.add_space(2.0);
    // 명령 ↔ 설명 2열 표. **명령을 누르면 그 명령만 클립보드로 간다** — 읽고 나서
    // 바로 쓰려면 옮겨 적어야 했다(사용자 요청 2026-09-05).
    //
    // 설명은 오른쪽 칸에 들어가는데, 길면 그 칸이 그만큼 넓어져 창이 늘어난다.
    // 그래서 설명은 **한 줄로 끝나는 길이**만 적는다(자세한 것은 복사본 MD에).
    egui::Grid::new("help_agent_cmds")
        .num_columns(2)
        .spacing([14.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (cmd, key) in CMDS {
                if cmd_button(ui, lang, cmd) {
                    *copy_cmd = Some(cmd.trim_end_matches(" \u{2026}").to_string());
                }
                ui.label(tr(lang, key));
                ui.end_row();
            }
        });
    ui.add_space(10.0);
    // 전체 목록 — **설명서에서 뽑는다.** 여기에 또 손으로 적으면 그 순간부터 어긋난다
    // (agentverbs). 누르면 그 명령이 클립보드로 간다.
    egui::CollapsingHeader::new(tr(lang, "help.agent.allcmds")).show(ui, |ui| {
        ui.weak(tr(lang, "help.agent.allcmds.hint"));
        ui.add_space(4.0);
        egui::Grid::new("help_agent_all").num_columns(2).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
            for (kind, cmd) in crate::agentverbs::all_verbs() {
                ui.label(egui::RichText::new(kind).weak().small());
                if cmd_button(ui, lang, cmd) {
                    *copy_cmd = Some(cmd.to_string());
                }
                ui.end_row();
            }
        });
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

#[cfg(test)]
mod cmds_tests {
    /// 도움말 표의 명령이 **실제로 있는 동사인지** 대조한다.
    ///
    /// 이 표는 사람이 골라 적는다(전부를 적지 않는다). 그래서 자동으로 채울 수는 없지만,
    /// 적힌 것이 실제와 맞는지는 셀 수 있다. 없는 명령을 적어 두면 그것을 읽은 AI 가
    /// 부르고 실패한다 — 그리고 실패한 AI 는 우리 프로그램이 고장 났다고 판단한다.
    #[test]
    fn 도움말에_적힌_명령은_실제로_있다() {
        let src = [
            include_str!("../../nabi-control/src/clientverbs.rs"),
            include_str!("../../nabi-control/src/client.rs"),
            include_str!("../../nabi-control/src/clientagent.rs"),
        ]
        .concat();
        let known: Vec<String> = src
            .split("Some(\"")
            .skip(1)
            .filter_map(|p| p.split('"').next().map(str::to_string))
            .collect();
        let mut missing = Vec::new();
        for (cmd, _) in super::CMDS {
            let Some(rest) = cmd.strip_prefix("nabi cli ") else {
                missing.push(cmd.to_string());
                continue;
            };
            let verb = rest.split_whitespace().next().unwrap_or("");
            if !known.iter().any(|k| k == verb) {
                missing.push(verb.to_string());
            }
        }
        assert!(missing.is_empty(), "도움말에만 있고 실제로는 없는 명령: {missing:?}");
    }
}
