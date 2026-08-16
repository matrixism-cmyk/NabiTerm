//! 전역 egui 크롬 테마 — "Slate + Cyan"(쿨 다크 + 단일 시안 강조).
//!
//! egui 크롬(패널·메뉴·탭·버튼·창·상태바)만 스타일링한다. 터미널 셀 영역은
//! `nabi_render::paint(.. &self.theme ..)`가 `nabi_vt::Theme`로 그리므로 영향 없음.

use egui::{Color32, CornerRadius, Stroke};

// --- 공유 팔레트(크롬 전용; 호출부 재사용 위해 pub) ----------------------
/// 강조색(시안-애저) — 링크·선택·포커스·활성 탭. 유일한 액센트.
pub const ACCENT: Color32 = Color32::from_rgb(64, 180, 230); // #40B4E6
/// 눌림/활성 위젯 채움(어두운 강조).
pub const ACCENT_DIM: Color32 = Color32::from_rgb(42, 110, 150); // #2A6E96
/// 의미색: 브로드캐스트/경고(앰버). 강조색 아님.
pub const BROADCAST: Color32 = Color32::from_rgb(255, 176, 32); // #FFB020
/// 의미색: 성공(녹색).
pub const OK: Color32 = Color32::from_rgb(74, 200, 120); // #4AC878
/// 의미색: 오류(빨강).
pub const ERR: Color32 = Color32::from_rgb(232, 90, 90); // #E85A5A
/// 메뉴 띠 채움(또렷한 슬레이트 — 너무 어두워 검게 보이지 않도록).
pub const MENU_FILL: Color32 = Color32::from_rgb(60, 72, 96);
/// 메뉴/툴바 강제 텍스트색(밝게 — 어두운 띠에서 확실히 보이도록).
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(238, 244, 250);
/// 상태바를 잡아주는 네이비 띠.
pub const STATUS_FILL: Color32 = Color32::from_rgb(28, 50, 74); // #1C324A

// --- 컬러 액센트(세션 종류·아이콘 — nabidrive식 타입별 색) ------------------
/// 로컬 셸 세션(녹색).
pub const SESS_LOCAL: Color32 = Color32::from_rgb(90, 200, 120); // #5AC878
/// SSH 세션(시안 — 강조색 계열).
pub const SESS_SSH: Color32 = Color32::from_rgb(70, 180, 235); // #46B4EB
/// FTP 세션(앰버).
pub const SESS_FTP: Color32 = Color32::from_rgb(232, 165, 70); // #E8A546
/// 폴더(금빛 — 탐색기식).
pub const FOLDER: Color32 = Color32::from_rgb(232, 190, 90); // #E8BE5A

/// 저장 세션의 종류별 강조색.
pub fn session_color(is_ftp: bool, ssh: bool) -> Color32 {
    if is_ftp {
        SESS_FTP
    } else if ssh {
        SESS_SSH
    } else {
        SESS_LOCAL
    }
}

// --- 내부 표면 ------------------------------------------------------------
const PANEL: Color32 = Color32::from_rgb(30, 36, 48); // 작업영역
// 메뉴/팝업/창 배경 — 띠/패널보다 더 밝게 해서 위로 또렷이 떠 보이게(검은 함몰 방지).
const WINDOW: Color32 = Color32::from_rgb(68, 81, 106);
const FAINT: Color32 = Color32::from_rgb(38, 46, 60); // 지브라 행
const EXTREME: Color32 = Color32::from_rgb(18, 22, 30); // 입력란
const SELECTION: Color32 = Color32::from_rgb(40, 96, 140); // 선택 배경
const W_INACTIVE: Color32 = Color32::from_rgb(44, 53, 70);
const W_HOVERED: Color32 = Color32::from_rgb(58, 72, 96);
const HAIRLINE: Color32 = Color32::from_rgb(46, 55, 72); // 테두리
const TEXT: Color32 = Color32::from_rgb(212, 222, 234); // 본문
const TEXT_STRONG: Color32 = Color32::from_rgb(236, 242, 248); // 강조 텍스트

/// 시작 시 한 번 적용(NabiApp::new에서 fonts 설치 옆에 호출).
pub fn apply_theme(ctx: &egui::Context) {
    let round_w = CornerRadius::same(5);
    let round_win = CornerRadius::same(8);
    let mut v = egui::Visuals::dark();

    // 배경(쿨 슬레이트-네이비)
    v.panel_fill = PANEL;
    v.window_fill = WINDOW;
    v.faint_bg_color = FAINT;
    v.extreme_bg_color = EXTREME;
    v.code_bg_color = EXTREME;

    // 강조: 링크 + 선택
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = SELECTION;
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    // 창/팝업 테두리
    v.window_corner_radius = round_win;
    v.window_stroke = Stroke::new(1.0, W_HOVERED);
    v.menu_corner_radius = round_w;
    v.window_shadow.color = Color32::from_black_alpha(120);
    v.popup_shadow.color = Color32::from_black_alpha(96);

    // 의미 전경색
    v.warn_fg_color = BROADCAST;
    v.error_fg_color = ERR;

    let w = &mut v.widgets;
    // 비상호작용: 라벨/구분선/패널 텍스트
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = round_w;
    // 비활성: 유휴 버튼/콤보
    w.inactive.bg_fill = W_INACTIVE;
    w.inactive.weak_bg_fill = W_INACTIVE;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = round_w;
    // 호버: 시안 테두리 + 약간 부상
    w.hovered.bg_fill = W_HOVERED;
    w.hovered.weak_bg_fill = W_HOVERED;
    w.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    w.hovered.fg_stroke = Stroke::new(1.5, TEXT_STRONG);
    w.hovered.corner_radius = round_w;
    w.hovered.expansion = 1.0;
    // 활성: 눌림/선택 — 어두운 강조 채움
    w.active.bg_fill = ACCENT_DIM;
    w.active.weak_bg_fill = ACCENT_DIM;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.5, TEXT_STRONG);
    w.active.corner_radius = round_w;
    w.active.expansion = 1.0;
    // 열림: 콤보/메뉴 펼침 — 호버 계열
    w.open.bg_fill = W_HOVERED;
    w.open.weak_bg_fill = W_HOVERED;
    w.open.bg_stroke = Stroke::new(1.0, ACCENT);
    w.open.fg_stroke = Stroke::new(1.0, TEXT_STRONG);
    w.open.corner_radius = round_w;

    // OS 라이트 모드가 테마를 뒤집지 못하게 다크 고정 + 양쪽 테마 슬롯 모두에
    // 우리 팔레트 주입(시스템 테마 이벤트가 set_visuals 한쪽만 덮는 사고 방지).
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);

    // 간격 현대화: egui 기본은 빽빽함 — 여백을 넓혀 시원한 레이아웃으로.
    // egui 자체 줌(Ctrl+휠/±) 비활성 — 글꼴 확대는 앱이 직접 관리. zoom_factor가
    // 영속되면 다음 실행의 초기 창 크기가 (모니터/줌)으로 클램프되어 창이 작아진다.
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    ctx.set_zoom_factor(1.0);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 24.0; // 버튼/콤보 최소 높이(클릭 타깃 확대).
        s.spacing.menu_margin = egui::Margin::same(8);
        s.spacing.window_margin = egui::Margin::same(10);
    });
}

/// egui_dock(탭바·분할) 스타일 — 크롬 테마와 일관된 현대식 탭.
/// 탭바는 어두운 띠, 활성 탭은 패널색으로 떠오르고 포커스 탭은 시안 윤곽.
pub fn dock_style(base: &egui::Style) -> egui_dock::Style {
    let round_tab = CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 };
    let mut s = egui_dock::Style::from_egui(base);
    s.tab_bar.bg_fill = EXTREME; // 탭 띠는 작업영역보다 깊게.
    s.tab_bar.height = 30.0;
    s.tab_bar.hline_color = HAIRLINE;
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    // 활성/포커스 탭: 패널색으로 본문과 이어지게, 포커스는 시안 윤곽.
    s.tab.active.bg_fill = PANEL;
    s.tab.active.outline_color = HAIRLINE;
    s.tab.active.corner_radius = round_tab;
    s.tab.active.text_color = TEXT;
    s.tab.focused.bg_fill = PANEL;
    s.tab.focused.outline_color = ACCENT;
    s.tab.focused.corner_radius = round_tab;
    s.tab.focused.text_color = TEXT_BRIGHT;
    s.tab.hovered.bg_fill = W_HOVERED;
    s.tab.hovered.corner_radius = round_tab;
    s.tab.hovered.text_color = TEXT_BRIGHT;
    s.tab.inactive.bg_fill = W_INACTIVE;
    s.tab.inactive.corner_radius = round_tab;
    s.tab.inactive.text_color = TEXT;
    s.tab.tab_body.bg_fill = PANEL;
    s.tab.tab_body.stroke = Stroke::NONE;
    // 분할 핸들: 평소엔 헤어라인, 잡으면 시안.
    s.separator.color_idle = HAIRLINE;
    s.separator.color_hovered = ACCENT;
    s.separator.color_dragged = ACCENT;
    // +/✕ 버튼과 드롭 오버레이도 강조색 계열로.
    s.buttons.add_tab_color = TEXT;
    s.buttons.add_tab_active_color = TEXT_BRIGHT;
    s.buttons.close_tab_color = TEXT;
    s.buttons.close_tab_active_color = TEXT_BRIGHT;
    s.overlay.selection_color = Color32::from_rgba_unmultiplied(64, 180, 230, 70);
    s
}
