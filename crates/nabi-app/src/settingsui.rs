//! 설정 다이얼로그 본문 — 카테고리별 페이지(좌측 내비게이션에서 선택). show_settings에서 호출.

use nabi_config::{AppConfig, EditorConfig};
use nabi_i18n::{tr, Lang};

/// 좌측 내비게이션 항목(i18n 키). 인덱스가 `page()`의 페이지 번호.
// 외관 그룹(글꼴·색상·커서·테마)을 앞쪽에 인접 배치 → 그 뒤 터미널·동작·에디터·강조·스니펫.
pub(crate) const PAGE_KEYS: [&str; 12] = [
    "settings.sec.font",
    "settings.sec.colors",
    "settings.sec.cursor",
    "settings.sec.import",
    "settings.sec.terminal",
    "settings.sec.behavior",
    "settings.sec.editor",
    "settings.sec.highlights",
    "settings.sec.snippets",
    "settings.sec.aiprof",
    "settings.sec.schedule",
    "settings.sec.telegram",
];

/// 페이지에 필요한 앱 핸들(설정 외 상태) 묶음 — 승인 정책 + 폰트 설치기.
pub(crate) struct PageCtx<'a> {
    pub policy: &'a nabi_control::policy::ControlPolicy,
    pub font_installer: &'a crate::fontinstall::FontInstaller,
    /// 텔레그램 DM 페어링 대기(chat, 코드, 만료) — 승인/거부 버튼이 직접 편집(C1).
    pub tg_pending: &'a std::cell::RefCell<Vec<(i64, String, std::time::Instant)>>,
    /// 스케줄 잡 목록(C3) + 영속 경로 — 페이지가 직접 편집·저장.
    pub sched: &'a std::cell::RefCell<Vec<crate::scheduler::Job>>,
    pub sched_path: &'a std::path::Path,
}

/// 선택된 카테고리 페이지 하나를 그린다(cfg 직접 편집; 적용은 apply_settings).
pub(crate) fn page(ui: &mut egui::Ui, cfg: &mut AppConfig, editor: &mut EditorConfig, lang: Lang, idx: usize, cx: &PageCtx) {
    match idx {
        // 외관 그룹: 글꼴(0)·색상(1)·커서(2)·테마(3).
        0 => grid(ui, "sec_font", |ui| font_rows(ui, cfg, lang, cx)),
        1 => grid(ui, "sec_colors", |ui| color_rows(ui, cfg, lang)),
        2 => grid(ui, "sec_cursor", |ui| cursor_rows(ui, cfg, lang)),
        3 => crate::themeimport::import_section(ui, cfg, lang),
        4 => grid(ui, "sec_terminal", |ui| terminal_rows(ui, cfg, lang)),
        5 => grid(ui, "sec_behavior", |ui| {
            crate::settingsui2::behavior_rows(ui, cfg, lang);
            crate::controlui::approvals_ui(ui, cx.policy, lang);
        }),
        6 => grid(ui, "sec_editor", |ui| editor_rows(ui, editor, lang)),
        7 => crate::settingslists::highlight_rows(ui, cfg, lang),
        8 => crate::settingslists::snippet_rows(ui, cfg, lang),
        9 => crate::aiprofileui::ai_profile_rows(ui, cfg, lang),
        10 => crate::schedui::schedule_rows(ui, lang, cx.sched, cx.sched_path),
        _ => grid(ui, "sec_telegram", |ui| crate::settingstelegram::telegram_rows(ui, cfg, lang, cx.tg_pending)),
    }
}

/// nabiPad 자체 설정 창 본문(메인 설정의 에디터 페이지와 동일 UI 재사용, DRY).
pub(crate) fn editor_settings_body(ui: &mut egui::Ui, e: &mut EditorConfig, lang: Lang) {
    grid(ui, "nabipad_settings", |ui| editor_rows(ui, e, lang));
}

/// nabiPad(내장 에디터) 설정 — 별도 파일(nabipad.toml)에 저장. 새로 여는 문서에 적용된다.
fn editor_rows(ui: &mut egui::Ui, e: &mut EditorConfig, lang: Lang) {
    let row = |ui: &mut egui::Ui, key: &str, v: &mut bool| { ui.label(tr(lang, key)); ui.checkbox(v, ""); ui.end_row(); };
    row(ui, "nabipad.openinwindow", &mut e.open_in_window);
    row(ui, "nabipad.menu.show", &mut e.show_menu_bar);
    // 아래 셋은 새로 여는 문서에 적용된다.
    row(ui, "editor.highlight", &mut e.syntax_highlight);
    row(ui, "editor.wrap", &mut e.word_wrap); row(ui, "editor.showws", &mut e.show_whitespace); row(ui, "editor.trimonsave", &mut e.trim_on_save); row(ui, "editor.finalnl", &mut e.final_newline); row(ui, "editor.autosave", &mut e.autosave);
    row(ui, "editor.indentspaces", &mut e.indent_spaces);
    ui.label(tr(lang, "editor.tabsize"));
    ui.add(egui::Slider::new(&mut e.tab_size, 1..=8));
    ui.end_row();
    crate::editorsyntax::settings_ui(ui, e, lang);
}

/// 2열 그리드 한 페이지(라벨 열 고정폭 + 여유 간격 — 현대식 설정 폼).
fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).min_col_width(150.0).spacing([28.0, 12.0]).show(ui, rows);
}

/// 글꼴·테마·UI 배율 그룹.
fn font_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang, cx: &PageCtx) {
    ui.label(tr(lang, "settings.fontsize"));
    // 슬라이더 0.5px 스냅 + 버튼 0.1px + 직접 입력(6~40px) — 정밀 제어 요청으로 확장.
    let spec = crate::settingsfont::FineSpec {
        coarse: 8.0..=32.0, full: 6.0..=40.0, snap: 0.5, fine: 0.1,
        decimals: 1, suffix: " px", default: nabi_config::DEFAULT_FONT_SIZE,
    };
    crate::settingsfont::fine_row(ui, &mut cfg.appearance.font_size, &spec, tr(lang, "settings.resetdefault"));

    ui.label(tr(lang, "settings.fontfamily"));
    ui.vertical(|ui| {
        // 설치된 등폭 글꼴 드롭다운(경로를 직접 타이핑하지 않아도 선택 가능) + 커스텀 경로.
        let fonts = crate::fonts::list_monospace_fonts();
        if !fonts.is_empty() {
            let cur = std::path::Path::new(&cfg.appearance.font_family)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| tr(lang, "settings.colordefault").to_string());
            egui::ComboBox::from_id_salt("set_font").selected_text(cur).show_ui(ui, |ui| {
                for (label, path) in &fonts {
                    if ui.selectable_label(&cfg.appearance.font_family == path, label).clicked() {
                        cfg.appearance.font_family = path.clone();
                    }
                }
            });
        }
        ui.add(
            egui::TextEdit::singleline(&mut cfg.appearance.font_family)
                .hint_text("C:\\...\\font.ttf")
                .desired_width(220.0),
        );
    });
    ui.end_row();

    // 인기 코딩 폰트 클릭 다운로드(미설치 시 GitHub에서 받아 자동 적용).
    ui.label(tr(lang, "font.install"));
    crate::settingsfont::font_get_row(ui, cfg, lang, cx.font_installer);
    ui.end_row();

    crate::settingsfont::ui_scale_row(ui, cfg, lang);
}

/// 커서 모양·깜빡임 그룹.
fn cursor_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.cursorshape"));
    egui::ComboBox::from_id_salt("set_cursorshape")
        .selected_text(cfg.appearance.cursor_shape.clone())
        .show_ui(ui, |ui| {
            for s in ["block", "bar", "underline"] {
                ui.selectable_value(&mut cfg.appearance.cursor_shape, s.to_owned(), s);
            }
        });
    ui.end_row();
    ui.label(tr(lang, "settings.cursorblink"));
    ui.checkbox(&mut cfg.appearance.cursor_blink, "");
    ui.end_row();
    ui.label(tr(lang, "settings.blinkms"));
    ui.add(egui::Slider::new(&mut cfg.appearance.blink_ms, 100..=1000).suffix(" ms"));
    ui.end_row();
}

/// 색상 그룹 + 라이브 미리보기.
/// 색상 테마 프리셋 드롭다운(스와치 미리보기). 색상 페이지 상단에 둬 개별 색상과 함께 보이게 한다(#9 테마+색상 통합).
fn theme_combo(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.theme"));
    egui::ComboBox::from_id_salt("set_theme")
        .selected_text(nabi_vt::Theme::preset_label(&cfg.appearance.theme))
        .show_ui(ui, |ui| {
            for name in nabi_vt::Theme::preset_names() {
                let t = nabi_vt::Theme::preset(name);
                let label = format!("  Aa  {}  ", nabi_vt::Theme::preset_label(name));
                let item = egui::RichText::new(label)
                    .monospace()
                    .color(egui::Color32::from_rgb(t.fg.r, t.fg.g, t.fg.b))
                    .background_color(egui::Color32::from_rgb(t.bg.r, t.bg.g, t.bg.b));
                if ui.selectable_label(cfg.appearance.theme == *name, item).clicked() { cfg.appearance.theme = (*name).to_owned(); }
            }
        });
    ui.end_row();
}

fn color_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    theme_combo(ui, cfg, lang); // 테마 프리셋(베이스) + 아래 개별 색상(오버라이드).
    {
        let a = &mut cfg.appearance;
        color_field(ui, tr(lang, "settings.cursorcolor"), &mut a.cursor_color, lang);
        color_field(ui, tr(lang, "settings.selectioncolor"), &mut a.selection_color, lang);
        color_field(ui, tr(lang, "settings.matchcolor"), &mut a.match_color, lang);
        color_field(ui, tr(lang, "settings.fgcolor"), &mut a.fg_color, lang);
        color_field(ui, tr(lang, "settings.bgcolor"), &mut a.bg_color, lang);
    }
    // 라이브 미리보기: 현재 색/글꼴이 적용된 터미널 한 줄.
    ui.label(tr(lang, "settings.preview"));
    crate::settingsprev::appearance_preview(ui, &cfg.appearance);
    ui.end_row();
}

fn terminal_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.shell"));
    egui::ComboBox::from_id_salt("set_shell")
        .selected_text(cfg.terminal.default_shell.clone())
        .show_ui(ui, |ui| {
            for s in ["powershell", "pwsh", "cmd", "wsl", "gitbash"] { ui.selectable_value(&mut cfg.terminal.default_shell, s.to_owned(), s); }
        });
    ui.end_row();

    // 새 터미널 기본 시작 디렉터리(비우면 포커스 셸 cwd 상속). 찾아보기 버튼 포함.
    ui.label(tr(lang, "settings.defaultcwd"));
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut cfg.terminal.default_cwd)
            .desired_width(220.0)
            .hint_text(tr(lang, "settings.defaultcwdhint"));
        ui.add(edit);
        if ui.button("\u{1f4c1}").clicked() {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                cfg.terminal.default_cwd = d.to_string_lossy().into_owned();
            }
        }
    });
    ui.end_row();

    ui.label(tr(lang, "settings.encoding"));
    egui::ComboBox::from_id_salt("set_encoding")
        .selected_text(cfg.terminal.encoding.clone())
        .show_ui(ui, |ui| {
            for e in [
                "UTF-8",
                "EUC-KR",
                "Shift_JIS",
                "GBK",
                "Big5",
                "ISO-8859-1",
                "windows-1252",
                "windows-1251",
            ] {
                ui.selectable_value(&mut cfg.terminal.encoding, e.to_owned(), e);
            }
        });
    ui.end_row();

    ui.label(tr(lang, "settings.scrollback"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.scrollback).range(0..=1_000_000).suffix(tr(lang, "settings.lines"))); ui.end_row();
    ui.label(tr(lang, "settings.searchlimit"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.search_limit).range(0..=1_000_000).suffix(tr(lang, "settings.lines"))); ui.end_row();
    ui.label(tr(lang, "settings.sshkeepalive")); ui.add(egui::DragValue::new(&mut cfg.terminal.ssh_keepalive_secs).range(0..=3600).suffix(" s")).on_hover_text(tr(lang, "settings.sshkeepalivehint")); ui.end_row();
    crate::settingsui2::sftp_rows(ui, cfg, lang); // SFTP 전송·인코딩 그룹(분리 — 라인 한도).
    crate::settingsui2::tip_rows(ui, cfg, lang); // 영문 팁 한글 오버레이.
    // 원격이 로컬 클립보드에 쓰는 것(OSC 52) — 차단/알림/조용히 허용.
    ui.label(tr(lang, "settings.osc52"));
    ui.horizontal(|ui| {
        for (v, key) in [(0u8, "settings.osc52.block"), (1, "settings.osc52.notify"), (2, "settings.osc52.allow")] {
            ui.selectable_value(&mut cfg.terminal.osc52_mode, v, tr(lang, key));
        }
    })
    .response
    .on_hover_text(tr(lang, "settings.osc52hint"));
    ui.end_row();
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
    ui.label(tr(lang, "settings.downloadask"));
    ui.checkbox(&mut cfg.terminal.download_ask, tr(lang, "settings.downloadaskhint")); ui.end_row();
    ui.label(tr(lang, "settings.statsalert"));
    ui.add(egui::Slider::new(&mut cfg.terminal.ssh_stats_alert_pct, 50..=100).suffix("%")); ui.end_row();
}

// behavior_rows·lang_choices는 settingsui2.rs로 분리(파일 크기 규율).

/// 색 입력 한 줄: 색 선택기(스와치) + 16진 텍스트(정밀) + 기본값 되돌리기.
/// 빈 값은 "테마 기본"을 뜻하며, 선택기로 고르면 #RRGGBB로 채워진다.
fn color_field(ui: &mut egui::Ui, label: &str, value: &mut String, lang: Lang) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut rgb = nabi_types::Rgba::from_hex(value)
            .map(|c| [c.r, c.g, c.b])
            .unwrap_or([0x88; 3]);
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *value = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        }
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(80.0)
                .hint_text(tr(lang, "settings.colordefault")),
        );
        if !value.is_empty()
            && ui.small_button("\u{21ba}").on_hover_text(tr(lang, "settings.colordefault")).clicked()
        {
            value.clear(); // 테마 기본으로 되돌림.
        }
    });
    ui.end_row();
}

