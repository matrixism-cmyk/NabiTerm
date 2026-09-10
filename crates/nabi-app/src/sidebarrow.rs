//! 사이드바 **세션 한 줄**을 그린다 — 목록·필터·그룹은 `sidebar` 에 있다.
//!
//! `sidebar` 가 하드 한도(400줄)에 닿아 나눴다. 자르는 자리는 자연스럽다:
//! 저쪽은 "무엇을 어떤 차례로 보여 줄까", 이쪽은 "한 줄을 어떻게 그릴까"다.

use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
use nabi_session::{SavedSession, SessionKind};

/// 세션 종류 아이콘(로컬/SSH/FTP).
fn kind_icon(s: &SavedSession) -> &'static str {
    if s.is_ftp {
        "\u{1f310}" // 🌐 FTP
    } else {
        match s.kind {
            SessionKind::Local { .. } => "\u{1f4bb}", // 💻 로컬 셸
            SessionKind::Ssh { .. } => "\u{1f5a5}",   // 🖥 SSH
            SessionKind::Serial { .. } => "\u{1f50c}", // 🔌 직렬 콘솔
        }
    }
}

/// 한 줄의 살아 있는 상태 — 인자를 하나 더 늘리는 대신 묶었다(이 함수는 이미 인자가 많다).
#[derive(Clone, Default)]
pub(crate) struct RowState {
    /// 지금 연결돼 있는가.
    pub live: bool,
    /// 붙어 있다면 어느 pane 인가 — 종류 아이콘을 누르면 그리로 간다.
    pub live_pane: Option<nabi_types::PaneId>,
    /// 색 말고 **기호로도** 알려 줄 것인가(접근성 설정 `symbol_cues`).
    ///
    /// 이 줄에는 색으로만 말하던 것이 둘 있었다 — 연결 표시(초록 아이콘)와
    /// 표식 띠(운영/스테이징/개발). 켜면 둘 다 글자를 함께 그린다.
    pub cues: bool,
    /// 마지막 일괄 확인 결과(안 훑었으면 None).
    pub reach: Option<crate::reachall::Reach>,
    /// 마지막 연결 실패(성공하면 지워진다).
    pub fail: Option<crate::lastfail::LastFail>,
}

/// 사이드바 세션 한 줄: 클릭=연결(SSH 열기), 드래그=그룹 이동, 우클릭=메뉴, 우측 아이콘(✎편집·✕삭제·🖧SFTP·⋯더보기).
#[allow(clippy::too_many_arguments)]
pub(crate) fn side_row(
    ui: &mut egui::Ui,
    lang: Lang,
    s: &SavedSession,
    cur_sel: Option<&str>,
    new_sel: &mut Option<String>,
    folders: &[String],
    notes: &std::collections::BTreeMap<String, String>,
    st: RowState,
    last: Option<i64>,
    now: i64,
    marked: bool,
    click_out: &mut Option<(String, bool, bool)>,
    // 직전 프레임에 이 행의 ⋯ 메뉴가 열려 있었는가.
    menu_was_open: bool,
    // 이번 프레임에 이 행의 ⋯ 메뉴가 열려 있으면 이름을 담는다.
    menu_open_out: &mut Option<String>,
) -> Option<MenuAction> {
    let mut action = None;
    // 초록 아이콘을 눌렀을 때 갈 곳. 그리는 도중에 정해지므로 따로 받아 둔다.
    let mut jump: Option<nabi_types::PaneId> = None;
    let is_ssh = matches!(s.kind, SessionKind::Ssh { .. }) && !s.is_ftp;
    let selected = cur_sel == Some(s.name.as_str()) || marked;
    // 행 전체 사각형을 **먼저** 잡는다. 배경을 이름 영역에만 칠하면 강조 막대가 오른쪽
    // 아이콘 자리에서 뚝 끊겨 지저분해 보이고, 호버 판정도 이름 위에서만 되어
    // 아이콘 쪽으로 마우스를 옮기면 강조가 꺼진다.
    let row_h = (ui.spacing().interact_size.y + 4.0).max(22.0);
    let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), row_h));
    let row_hot = ui.rect_contains_pointer(full);
    // 동작 아이콘은 **가리키거나 선택했을 때만** 보여 준다. 늘 띄워 두면 세션 10개에
    // 아이콘이 40개라 목록이 아이콘 밭이 된다(오클릭 위험도 있다 — 특히 ✕ 삭제).
    // 평소엔 이름만 보이고, 우클릭 메뉴는 행 어디서나 그대로 열린다.
    // ⋯ 메뉴가 열려 있는 동안에는 아이콘을 계속 그린다. 예전엔 행에서 마우스가 벗어나면
    // 버튼 자체가 사라졌고, **버튼에 매인 메뉴도 같이 닫혔다** — 항목을 고르려고 아래로
    // 내려가는 순간 닫혀 쓸 수가 없었다(사용자 보고 2026-08-21). 우클릭 메뉴는 행 응답에
    // 매여 있어 같은 문제가 없었다.
    let show_icons = row_hot || selected || menu_was_open;
    let rounding = egui::CornerRadius::same(4);
    if selected {
        ui.painter().rect_filled(full, rounding, ui.visuals().selection.bg_fill);
    } else if row_hot {
        ui.painter().rect_filled(full, rounding, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let resp = ui.horizontal(|ui| {
        // 아이콘 자리는 **행마다 같은 폭**으로 예약한다. 예전엔 SSH 여부로 90/68을 갈라서
        // 종류가 섞이면 이름 끝나는 위치가 22px씩 어긋나 목록이 들쭉날쭉해 보였다.
        const ICON_W: f32 = 92.0;
        let bw = (ui.available_width() - ICON_W).max(48.0);
        let (rect, r) = ui.allocate_exact_size(egui::vec2(bw, row_h), egui::Sense::click_and_drag());
        let vis = ui.style().interact_selectable(&r, selected);
        let font = egui::TextStyle::Button.resolve(ui.style());
        // 연결 중이면 종류 아이콘을 강조색(초록)으로 — 🟢 점 대신 선두 아이콘 색으로 표시.
        // 표식 띠(운영/스테이징/개발) — 행 왼쪽 가장자리. 목록을 훑을 때 이름보다 먼저 보인다.
        if s.tag != nabi_session::SessionTag::None {
            let (r8, g8, b8) = s.tag.rgb();
            let col = egui::Color32::from_rgb(r8, g8, b8);
            // 기호를 함께 낼 때는 띠를 글자가 들어갈 만큼 넓힌다.
            let w = if st.cues { 11.0 } else { 3.0 };
            let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(w, rect.height()));
            ui.painter().rect_filled(bar, egui::CornerRadius::ZERO, col);
            // **"이건 운영 서버다"를 색으로만 말하고 있었다.** 같은 표식을 상태 표시줄·
            // 우클릭 메뉴·확인 창은 글자와 함께 보여 주는데 목록만 아니었다(2026-09-10).
            if st.cues {
                ui.painter().text(
                    bar.center(),
                    egui::Align2::CENTER_CENTER,
                    crate::cues::tag_letter(s.tag),
                    egui::FontId::proportional(9.0),
                    // 띠 위에 얹으므로 띠와 대비되는 색이라야 읽힌다.
                    crate::theme_ui::on_color(col),
                );
            }
        }
        let kcolor = if st.live { crate::theme_ui::OK } else { crate::theme_ui::session_color(s.is_ftp, is_ssh) };
        ui.painter().text(egui::pos2(rect.left() + 5.0, rect.center().y), egui::Align2::LEFT_CENTER, kind_icon(s), font.clone(), kcolor);
        // **초록 아이콘을 누르면 그 탭으로 간다**(같은 데를 또 여는 대신).
        //
        // 행 전체의 클릭은 그대로 "연결"이다 — 오래 쓰던 동작을 바꾸면, 붙어 있는 줄
        // 모르고 누른 사람이 갑자기 다른 데로 끌려간다. 새 능력은 아이콘에만 붙인다.
        if let Some(p) = st.live_pane {
            let hit = egui::Rect::from_min_size(egui::pos2(rect.left() + 2.0, rect.top()), egui::vec2(14.0, rect.height()));
            let r = ui.interact(hit, ui.id().with(("jump", &s.name)), egui::Sense::click());
            if r.on_hover_text(tr(lang, "sessions.jumphere")).clicked() {
                jump = Some(p);
            }
        }
        // **연결됨을 색으로만 말하던 자리.** 이 파일의 `cues` 모듈이 문제의 예로 들고도
        // 정작 안 고쳤던 곳이다(2026-09-10). 켜면 아이콘 옆에 점을 함께 그린다.
        //
        // 자리는 아래 "일괄 확인 결과"와 같다. 겹치지 않는다 — 그쪽은 연결 중이 아닐
        // 때만 그리고, 이쪽은 연결 중일 때만 그린다.
        if st.live && st.cues {
            ui.painter().text(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                crate::cues::LIVE,
                egui::FontId::proportional(9.0),
                crate::theme_ui::OK,
            );
        }
        // 일괄 확인 결과 — 이름 앞에 작은 점. 연결돼 있으면 이미 초록 아이콘이 있으므로
        // 겹쳐 그리지 않는다.
        if let (Some(rc), false) = (st.reach, st.live) {
            ui.painter().text(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                rc.mark(),
                egui::FontId::proportional(9.0),
                rc.color(),
            );
        }
        // 마지막 연결 실패 — 이름 앞 경고 표시. 연결 중이면 이미 지워졌으므로 뜨지 않는다.
        let mut warn_rect = None;
        if let (Some(f), false) = (st.fail.as_ref(), st.live) {
            let pos = egui::pos2(rect.left() + 16.0, rect.center().y);
            let g = ui.painter().text(pos, egui::Align2::LEFT_CENTER, "\u{26a0}", egui::FontId::proportional(10.0), crate::theme_ui::ERR);
            warn_rect = Some((g, crate::lastfail::detail(lang, f)));
        }
        // 이름(+메모 📝) — 폭 넘치면 … 말줄임.
        let note = if notes.get(&s.name).is_some_and(|n| !n.is_empty()) { " \u{1f4dd}" } else { "" };
        let mut job = egui::text::LayoutJob::simple_singleline(format!("{}{note}", s.name), font, vis.text_color());
        job.wrap = egui::text::TextWrapping { max_width: (rect.width() - 26.0).max(16.0), max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, vis.text_color());
        // 왜 실패했는지는 **가리켰을 때** 보여 준다. 목록에 늘 펼쳐 두면 이름이 밀린다.
        if let Some((wr, tip)) = warn_rect {
            // 경고 글리프 자리에만 감지 영역을 둔다 — 행 전체에 붙이면 이름 위에서도 떠서
            // 목록을 훑는 내내 말풍선이 따라다닌다.
            let id = egui::Id::new(("failtip", &s.name));
            ui.interact(wr, id, egui::Sense::hover()).on_hover_text(tip);
        }
        // 드래그(길게 눌러 이동) 페이로드 + 고스트(커서 옆에 이름).
        if r.drag_started() { r.dnd_set_drag_payload(s.name.clone()); }
        if r.dragged() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                egui::Area::new(egui::Id::new(("sdghost", &s.name))).order(egui::Order::Tooltip).fixed_pos(p + egui::vec2(12.0, 4.0))
                    .show(ui.ctx(), |ui| { egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(format!("\u{1f5a5} {}", s.name))); });
            }
        }
        // 인라인 동작 아이콘(우측): ⋯더보기 · 🖧SFTP(SSH) · ✎편집 · ✕삭제.
        //
        // 간격이 1px이었다. 버튼의 둥근 호버 배경은 글자보다 패딩만큼 넓어서, 한 아이콘에
        // 마우스를 올리면 그 배경이 **옆 아이콘 자리까지 겹쳐** 보였다. 글자 간격이 아니라
        // 배경끼리 부딪히지 않을 만큼 띄워야 한다.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if !show_icons {
                return; // 가리키지 않은 행은 이름만 — 자리는 위에서 이미 비워 뒀다.
            }
            let icon = |ui: &mut egui::Ui, g: &str, key: &str| ui.small_button(g).on_hover_text(tr(lang, key)).clicked();
            if icon(ui, "\u{2715}", "sessions.delete") { action = Some(MenuAction::DeleteSession(s.name.clone())); }
            if icon(ui, "\u{270e}", "sessions.edit") { action = Some(MenuAction::EditSession(s.clone())); }
            if is_ssh && icon(ui, "\u{1f5a7}", "sessions.opensftp") { action = Some(MenuAction::OpenSftp(s.clone())); }
            ui.menu_button("\u{22ef}", |ui| {
                *menu_open_out = Some(s.name.clone()); // 열려 있는 동안 아이콘을 유지시킨다.
                if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
            });
        });
        r
    });
    // 호버: 연결 정보 + (있으면) 마지막 접속 상대시간 + 메모.
    let mut hint = crate::connectsave::conn_hint(s);
    if let Some(t) = last.filter(|&t| t > 0) { hint.push_str(&format!("\n\u{1f553} {}", crate::humanfmt::human_age(t as u64, now as u64))); }
    if let Some(n) = notes.get(&s.name).filter(|n| !n.is_empty()) { hint.push_str(&format!("\n\u{1f4dd} {n}")); }
    let resp = resp.inner.on_hover_text(hint);
    // 클릭=연결(SSH 열기). 드래그(이동)는 페이로드로 처리돼 단순 클릭과 구분된다.
    if resp.clicked() {
        // Ctrl/Shift는 '선택'이고 평클릭은 '연결'이다. 범위 선택은 보이는 순서를 아는
        // 호출부에서 판정하므로 여기서는 수정자만 실어 올려보낸다.
        let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
        *click_out = Some((s.name.clone(), ctrl, shift));
        if !ctrl && !shift {
            *new_sel = Some(s.name.clone());
        }
    }
    // 우클릭=더보기(라인 어디서나). 라인 전폭 영역이라 이름 어디서 눌러도 동일 메뉴.
    resp.context_menu(|ui| {
        if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
    });
    // 초록 아이콘을 눌렀으면 그것이 이긴다 — 행 클릭(연결)보다 뒤에 두어 덮어쓴다.
    // 아이콘은 행 안에 있으므로 두 판정이 같은 프레임에 함께 참일 수 있다.
    if let Some(p) = jump {
        action = Some(MenuAction::JumpToPane(p));
    }
    action
}

