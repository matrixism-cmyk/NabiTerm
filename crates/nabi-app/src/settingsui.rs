//! 설정 다이얼로그 본문 — 카테고리별 페이지(좌측 내비게이션에서 선택). show_settings에서 호출.

use nabi_config::{AppConfig, EditorConfig};
use nabi_i18n::{tr, Lang};

/// 좌측 내비게이션 항목(i18n 키). 인덱스가 `page()`의 페이지 번호.
//
// 종류별로 묶는다(사용자 요청 2026-08-19·08-21):
//   일반 → 모양 → 터미널 → 원격 연결 → 규칙 → 자동화.
// • 모양: 글꼴·색상·커서·테마 가져오기를 한 페이지로(모두 [appearance] 한 덩어리라 나눌 양이 아니다).
// • 원격 연결: SSH와 전송(SFTP)을 한 페이지로. SSH는 항목이 둘뿐이라 탭 하나를 가질 양이 아니었다.
// • AI 터미널(프로필)은 **여기서 뺐다** — 전용 창(터미널 메뉴 ▸ 프로필 관리)이 이미 있고,
//   저장된 세션 목록처럼 '설정'이 아니라 '목록 관리'다.
// • 접근성: 색·크기·움직임에 관한 것을 한자리에 모은다. 흩어 두면 정작 필요한 사람이
//   못 찾는다 — 여기 있는 값 일부는 다른 페이지에도 그대로 있다(같은 값을 가리킨다).
pub(crate) const PAGE_KEYS: [&str; 7] = [
    "settings.sec.general",
    "settings.sec.appearance",
    "settings.sec.terminal",
    "settings.sec.remote",
    "settings.sec.rules",
    "settings.sec.automation",
    "settings.sec.a11y",
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
/// (nabiPad 설정은 편집기 창 메뉴로 이동 — 여기서는 편집기 설정을 다루지 않는다.)
pub(crate) fn page(ui: &mut egui::Ui, cfg: &mut AppConfig, _editor: &mut EditorConfig, lang: Lang, idx: usize, cx: &PageCtx) {
    match idx {
        // 일반(0): 언어·단축키·복원·승인 정책.
        // 이 페이지는 표를 **스스로** 조각내 연다 — 설명 줄이 두 칸을 가로질러야 해서다.
        0 => {
            crate::settingsui2::behavior_rows(ui, cfg, lang);
            grid(ui, "sec_general_approvals", |ui| {
                crate::controlui::approvals_ui(ui, cx.policy, lang);
            });
        }
        // 모양(1): 글꼴 → 색상 → 커서 → 테마 가져오기.
        1 => {
            group(ui, lang, "settings.sec.font");
            grid(ui, "sec_font", |ui| font_rows(ui, cfg, lang, cx));
            group(ui, lang, "settings.sec.colors");
            grid(ui, "sec_colors", |ui| color_rows(ui, cfg, lang));
            group(ui, lang, "settings.sec.cursor");
            grid(ui, "sec_cursor", |ui| cursor_rows(ui, cfg, lang));
            group(ui, lang, "settings.sec.import");
            crate::themeimport::import_section(ui, cfg, lang);
        }
        2 => grid(ui, "sec_terminal", |ui| terminal_rows(ui, cfg, lang)),
        // 원격 연결(3): SSH(접속 유지·통계) → 전송·SFTP(속도·병렬·무결성·다운로드 폴더).
        3 => {
            group(ui, lang, "settings.sec.ssh"); grid(ui, "sec_ssh", |ui| crate::settingsui2::ssh_rows(ui, cfg, lang));
            group(ui, lang, "settings.sec.transfer"); grid(ui, "sec_transfer", |ui| crate::settingsxfer::transfer_rows(ui, cfg, lang));
        }
        // 사용자 규칙(4): 키워드 강조 + 명령 스니펫(둘 다 직접 관리하는 목록).
        // 사용자 규칙(4): 한 장에 네 가지가 쌓여 있어 무엇이 무엇인지 읽기 어려웠다.
        // 하는 일별로 소제목을 달아 나눈다(2026-08-25 IA 정리).
        4 => {
            group(ui, lang, "settings.sec.highlights"); crate::settingslists::highlight_rows(ui, cfg, lang);
            group(ui, lang, "settings.sec.triggers"); crate::settingslists::alert_rows(ui, cfg, lang);
            group(ui, lang, "settings.sec.linkrules"); crate::settingslists::link_rule_rows(ui, cfg, lang);
            group(ui, lang, "settings.sec.snippets"); crate::settingslists::snippet_rows(ui, cfg, lang);
        }
        // 자동화(5): 내장 스케줄러 + 텔레그램 브리지.
        5 => {
            group(ui, lang, "settings.sec.schedule");
            crate::schedui::schedule_rows(ui, lang, cx.sched, cx.sched_path);
            group(ui, lang, "settings.sec.telegram");
            grid(ui, "sec_telegram", |ui| crate::settingstelegram::telegram_rows(ui, cfg, lang, cx.tg_pending));
        }
        // 접근성 — 색·크기·움직임. 일부는 다른 페이지에도 있다(같은 값을 가리킨다).
        _ => grid(ui, "sec_a11y", |ui| crate::settingsa11y::a11y_rows(ui, cfg, lang)),
    }
}

/// 카테고리 키 → 좌측 내비 인덱스. 팔레트/메뉴가 특정 설정 페이지로 바로 뛸 때 쓴다
/// (인덱스 하드코딩 금지 — 페이지를 통합·분리해도 어긋나지 않는다).
pub(crate) fn page_index(key: &str) -> usize {
    PAGE_KEYS.iter().position(|k| *k == key).unwrap_or(0)
}

/// 한 페이지에 둘 이상의 그룹을 담을 때 쓰는 소제목(통합 페이지의 경계).
fn group(ui: &mut egui::Ui, lang: Lang, key: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tr(lang, key)).strong());
    ui.separator();
}

/// nabiPad 자체 설정 창 본문(메인 설정의 에디터 페이지와 동일 UI 재사용, DRY).
pub(crate) fn editor_settings_body(ui: &mut egui::Ui, e: &mut EditorConfig, lang: Lang) {
    grid(ui, "nabipad_settings", |ui| editor_rows(ui, e, lang));
    // 언어 서버는 **깔려 있지 않은 것이 기본**이라 아무 일도 안 일어나는 것이 정상이다.
    // 그런데 사용자에게는 고장과 구별되지 않으므로, 무엇이 없어서 안 되는지 보여 준다.
    crate::settingslsp::lsp_group(ui, lang);
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
    // '줄바꿈' 변환이 쓸 폭 — 예전엔 80으로 박혀 있었다.
    ui.label(tr(lang, "editor.wrapcol"));
    ui.add(egui::Slider::new(&mut e.wrap_col, 40..=200));
    ui.end_row();
    // 세로 눈금 — 접지 않고 규약 폭을 보여 준다(줄바꿈 폭과 다른 것이라 나란히 둔다).
    ui.label(tr(lang, "editor.rulers")); ui.add(egui::TextEdit::singleline(&mut e.rulers).hint_text("80,100").desired_width(120.0))
        .on_hover_text(tr(lang, "editor.rulers.hint"));
    ui.end_row();
    // 안내선과 눈금은 둘 다 "보조선"이라 나란히 둔다 — 흩어 놓으면 하나만 찾게 된다.
    ui.label(tr(lang, "editor.guides")); ui.checkbox(&mut e.guides, "").on_hover_text(tr(lang, "editor.guides.hint"));
    ui.end_row();
    // 미니맵 — 예전에는 문서마다 꺼진 채 시작해 켜도 다음 파일에서 다시 꺼졌다.
    ui.label(tr(lang, "nabipad.minimap")); ui.checkbox(&mut e.minimap, "").on_hover_text(tr(lang, "editor.minimap.hint"));
    ui.end_row();
    // 긴 파일에서 "지금 어느 함수 안인가"를 맨 위에 붙여 둔다(VS Code 의 sticky scroll).
    ui.label(tr(lang, "nabipad.sticky")); ui.checkbox(&mut e.sticky, "").on_hover_text(tr(lang, "editor.sticky.hint"));
    ui.end_row();
    crate::editorsyntax::settings_ui(ui, e, lang);
}

/// 라벨 칸의 폭. 표를 여러 조각으로 나눠도 이 값이 같으면 줄이 어긋나 보이지 않는다.
const LABEL_W: f32 = 200.0;

/// 2열 그리드 한 페이지(라벨 열 고정폭 + 여유 간격 — 현대식 설정 폼).
fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).min_col_width(LABEL_W).spacing([28.0, 12.0]).show(ui, rows);
}

/// 표를 한 조각 그린다 — 설명 줄을 사이에 끼우려고 나눌 때 쓴다.
pub(crate) fn grid_seg(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    grid(ui, id, rows);
}

/// 표 왼쪽 칸의 라벨 — **폭을 정확히 고정한다.**
///
/// `min_col_width` 만으로는 부족했다. 그것은 최소일 뿐이라, 라벨이 긴 조각은 그만큼
/// 넓어지고 짧은 조각은 안 넓어진다. 표를 조각내 놓으니 조각마다 칸 폭이 달라져
/// **체크박스가 들쭉날쭉해 보였다**(사용자 보고 2026-09-05).
///
/// 폭을 못 박으면 어느 조각이든 같은 자리에서 시작한다. 라벨이 길면 줄이 접힌다 —
/// 접히는 편이 어긋나는 것보다 낫다.
pub(crate) fn label_cell(ui: &mut egui::Ui, text: &str) {
    ui.scope(|ui| {
        ui.set_width(LABEL_W);
        ui.add(egui::Label::new(text).wrap());
    });
}

/// **두 칸을 가로지르는 설명 한 줄.**
///
/// 설명을 표의 칸 안에 넣었더니 그 칸이 설명 길이만큼 넓어져 설정 창 폭이 흐트러졌다
/// (사용자 보고 2026-09-05). 칸 폭은 그 칸에 들어간 것 중 가장 넓은 것이 정하므로,
/// 긴 설명 하나가 창 전체를 끌고 다닌 것이다.
///
/// 그래서 표 **밖에** 그린다. 표가 아니니 칸 폭에 영향을 주지 않고, 페이지 폭에 맞춰
/// 스스로 줄을 접는다.
pub(crate) fn help_line(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).weak().small()).wrap());
    ui.add_space(8.0);
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
    crate::settingsshell::shell_row(ui, cfg, lang);
    // 세션 기록 — 원격 연결 페이지에 있던 것을 여기로 옮겼다(배치 AM). 로컬 셸도 남는다.
    crate::settingslog::log_rows(ui, cfg, lang);

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
