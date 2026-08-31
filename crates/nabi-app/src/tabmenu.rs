//! 탭 우클릭 컨텍스트 메뉴(이름 변경 · 브로드캐스트 그룹 · 색상 라벨).

use nabi_i18n::{tr, Lang};
use nabi_orchestrator::OrchestratorHandle;
use nabi_types::PaneId;
use std::collections::{HashMap, HashSet};

/// 탭 색상 지정 시 탭 텍스트 색을 적용한 스타일을 돌려준다(없으면 None).
pub(crate) fn tab_style(
    colors: &HashMap<PaneId, egui::Color32>,
    tab: &PaneId,
    global: &egui_dock::TabStyle,
) -> Option<egui_dock::TabStyle> {
    let mut s = global.clone();
    match colors.get(tab) {
        // 사용자 지정 색은 탭 배경에 적용(시안성↑) + 명도에 따라 대비 텍스트색 자동 선택.
        Some(&color) => {
            let [r, g, b, _] = color.to_array();
            let txt = if 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 140.0 { egui::Color32::BLACK } else { egui::Color32::WHITE };
            for st in [&mut s.active, &mut s.inactive, &mut s.focused, &mut s.hovered] {
                st.bg_fill = color;
                st.text_color = txt;
            }
        }
        // 없으면 활성/포커스 탭만 강조색(시안)으로 부각.
        None => {
            s.active.text_color = crate::theme_ui::ACCENT;
            s.focused.text_color = crate::theme_ui::ACCENT;
        }
    }
    Some(s)
}

/// 탭 표시 제목: 활동 마커 + (사용자 지정 이름 > OSC 제목 > 오케스트레이터 제목).
pub(crate) fn tab_title(
    orch: &OrchestratorHandle,
    tab_names: &HashMap<PaneId, String>,
    activity: &HashSet<PaneId>,
    cwds: &HashMap<PaneId, String>,
    tab: &PaneId,
) -> String {
    let mark = if activity.contains(tab) {
        "\u{25cf} "
    } else {
        ""
    };
    let nonempty = |s: String| (!s.is_empty()).then_some(s);
    let base = tab_names
        .get(tab)
        .filter(|n| !n.is_empty())
        .cloned()
        .or_else(|| {
            orch.panes.read().ok().and_then(|m| m.get(tab).cloned()).and_then(|v| {
                let t = v.model.lock().ok().map(|md| md.title().to_string());
                t.filter(|s| !s.is_empty()).or_else(|| nonempty(v.title.clone()))
            })
        })
        // 제목이 없으면 현재 작업 디렉터리 이름으로(로컬 셸 탭이 "pane N" 대신).
        .or_else(|| cwds.get(tab).and_then(|c| nonempty(basename(c))))
        .unwrap_or_else(|| format!("pane {}", tab.get()));
    // 제목 위생화(제어문자 제거)+말줄임은 공유 nabi_render::clean_title로 일원화(중복 truncate 제거).
    format!("{mark}{}", nabi_render::clean_title(&strip_exe_path(base), 24))
}

/// 기본 제목이 셸 실행파일 전체 경로("C:\...\pwsh.exe")면 이름만 남긴다 — 어떤
/// 셸인지만 알면 충분. 일반 제목(경로 아님)은 건드리지 않는다. (탭·OS 창 제목 공용.)
pub(crate) fn strip_exe_path(s: String) -> String {
    if !s.to_ascii_lowercase().ends_with(".exe") {
        return s;
    }
    let b = basename(&s);
    let name = b[..b.len() - 4].to_string();
    if name.is_empty() {
        s
    } else {
        name
    }
}

/// 경로의 마지막 구성요소(폴더명). 슬래시/역슬래시 모두 처리.
fn basename(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_string()
}

/// 탭 색상 라벨 프리셋(빨강/초록/파랑/노랑/보라).
const TAB_COLORS: [egui::Color32; 5] = [
    egui::Color32::from_rgb(0xe0, 0x5a, 0x5a),
    egui::Color32::from_rgb(0x5a, 0xc0, 0x6a),
    egui::Color32::from_rgb(0x5a, 0x8a, 0xe0),
    egui::Color32::from_rgb(0xe0, 0xc0, 0x4a),
    egui::Color32::from_rgb(0xb0, 0x6a, 0xd0),
];

/// 탭 메뉴에서 **나오는 것들**.
///
/// 예전에는 이 자루의 내용이 전부 따로따로 인자였다. 열여섯 개까지 늘어나서, 하나를 더
/// 붙일 때마다 부르는 자리와 받는 자리를 함께 고쳐야 했다 — 그러다 한쪽을 빠뜨리면
/// 컴파일은 되는데 엉뚱한 값이 들어간다.
///
/// 묶어 두면 새 항목을 더할 때 **기본값이 저절로 따라온다**(`Default`).
#[derive(Default)]
pub(crate) struct TabMenuOut {
    /// 이 탭을 분리 창으로 뗀다.
    pub tear_off: Option<PaneId>,
    /// 이 탭의 원격(SFTP) 브라우저를 연다.
    pub sftp_open: Option<PaneId>,
    /// AI 로 넘긴다(pane, 클립보드만).
    pub ai_handoff: Option<(PaneId, bool)>,
    /// 이 pane 의 전체 기록을 그 자리에서 연다.
    pub open_history: Option<PaneId>,
    /// 같은 출처로 탭을 하나 더 연다.
    pub duplicate: bool,
    /// 사용자에게 보일 한 줄(저장 성공·실패 등). 있으면 중앙이 알림으로 띄운다.
    pub notice: Option<String>,
}

/// 탭 메뉴가 **읽고 고치는 것들**.
///
/// pane 마다 딸린 표들(이름·색·브로드캐스트 묶음·휠 설정)과 이 탭에 대한 사실 두 가지다.
/// 인자로 늘어놓으면 열셋이 되어 clippy 가 막고, 막는 것을 `#[allow]` 로 가리면 다음에
/// 하나 더 붙일 때 또 늘어난다 — 가리지 않고 묶었다.
pub(crate) struct TabMenuState<'a> {
    pub tab_names: &'a mut HashMap<PaneId, String>,
    pub broadcast_group: &'a mut HashSet<PaneId>,
    pub wheel_keys: &'a mut HashSet<PaneId>,
    pub wheel_keys_off: &'a mut HashSet<PaneId>,
    pub tab_colors: &'a mut HashMap<PaneId, egui::Color32>,
    /// 이 pane 에서 도는 프로그램이 휠을 직접 받는가(자동 감지).
    pub wheel_auto: bool,
    pub is_ssh: bool,
}

pub(crate) fn tab_context_menu(
    ui: &mut egui::Ui,
    tab: &PaneId,
    orch: &OrchestratorHandle,
    lang: Lang,
    st: &mut TabMenuState<'_>,
    out: &mut TabMenuOut,
    dock_float: Option<&mut Option<PaneId>>,
) {
    // 자루 안의 것들을 옛 이름으로 받아 둔다 — 아래 본문을 그대로 두려는 것이다.
    let TabMenuOut { tear_off, sftp_open, ai_handoff, open_history, duplicate, notice } = out;
    let save_msg = notice;
    let TabMenuState {
        tab_names,
        broadcast_group,
        wheel_keys,
        wheel_keys_off,
        tab_colors,
        wheel_auto,
        is_ssh,
    } = st;
    let (wheel_auto, is_ssh) = (*wheel_auto, *is_ssh);
    ui.label(tr(lang, "tab.rename"));
    let name = tab_names.entry(*tab).or_default();
    // 포커스를 확실히 잡아 키 입력이 터미널로 새지 않게 한다.
    nabi_editor::uiutil::focus_once(&ui.text_edit_singleline(name)); // 매 프레임 재요청 금지(IME).
    if ui.button(tr(lang, "tab.reset")).clicked() {
        tab_names.remove(tab);
        ui.close();
    }
    if ui.button(tr(lang, "term.reset")).clicked() {
        if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) {
            if let Ok(mut md) = v.model.lock() {
                md.reset();
            }
        }
        ui.close();
    }
    // 전체 기록 열기 — 스크롤백에 안 남는 프로그램(클로드 코드 등) 때문에 필요하다.
    // 지우는 항목 바로 위에 둔다: 지우기 전에 남길 길이 눈에 먼저 들어와야 한다.
    if ui.button(tr(lang, "hist.open")).clicked() {
        *open_history = Some(*tab);
        ui.close();
    }
    // 스크롤백(히스토리)만 비우기 — 화면 유지(Clear Buffer).
    if ui.button(tr(lang, "term.clearbuf")).clicked() {
        if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) {
            if let Ok(mut md) = v.model.lock() { md.clear_scrollback(); }
        }
        ui.close();
    }
    // 출력을 어디론가 보내는 여섯 가지는 한 묶음이다(taboutputmenu) — 평평하게 늘어놓으면
    // 다른 항목 열셋 사이에 흩어져 찾는 사람이 열아홉 개를 다 읽어야 한다.
    crate::taboutputmenu::output_menu(ui, tab, orch, lang, ai_handoff, save_msg);
    // 스크롤백 맨 위/아래로 이동(긴 출력 빠른 탐색).
    let scroll = |top: bool| { if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) { if let Ok(mut md) = v.model.lock() { if top { md.scroll_to_top(); } else { md.scroll_to_bottom(); } } } };
    if ui.button(tr(lang, "term.scrolltop")).clicked() { scroll(true); ui.close(); }
    if ui.button(tr(lang, "term.scrollbottom")).clicked() { scroll(false); ui.close(); }
    // 스크롤백을 남기지 않는 TUI(codex CLI 등)는 휠로 볼 과거가 터미널에 없다.
    // 그런 pane에서만 사용자가 켜서 휠을 PageUp/PageDown으로 바꿔 보낸다.
    let mut keys = wheel_keys.contains(tab) || (wheel_auto && !wheel_keys_off.contains(tab));
    if ui.checkbox(&mut keys, tr(lang, "tab.wheelkeys")).on_hover_text(tr(lang, "tab.wheelkeys.hint")).clicked() {
        // 끄기는 명시 기록으로 남긴다 — 자동 감지(codex)가 다시 켜 버리지 않게.
        if keys { wheel_keys.insert(*tab); wheel_keys_off.remove(tab); }
        else { wheel_keys.remove(tab); wheel_keys_off.insert(*tab); }
    }
    ui.separator();
    let mut member = broadcast_group.contains(tab);
    if ui
        .checkbox(&mut member, tr(lang, "tab.bcastgroup"))
        .clicked()
    {
        if member {
            broadcast_group.insert(*tab);
        } else {
            broadcast_group.remove(tab);
        }
    }
    ui.horizontal(|ui| {
        ui.label(tr(lang, "tab.color"));
        for col in TAB_COLORS {
            if ui.button(egui::RichText::new("\u{25cf}").color(col)).clicked() {
                tab_colors.insert(*tab, col);
            }
        }
        if ui.button("\u{2715}").clicked() {
            tab_colors.remove(tab);
        }
    });
    ui.separator();
    // 분리: '새 OS 창으로 분리'(별도 OS 창)와 '창 안에 띄우기'(메인 창 안 오버레이) — 둘 다 노출.
    if ui
        .button(tr(lang, "tab.tearoff"))
        .on_hover_text(tr(lang, "tab.tearoff.hint"))
        .clicked()
    {
        *tear_off = Some(*tab);
        ui.close();
    }
    if let Some(dock_float) = dock_float {
        if ui
            .button(tr(lang, "tab.dockfloat"))
            .on_hover_text(tr(lang, "tab.dockfloat.hint"))
            .clicked()
        {
            *dock_float = Some(*tab);
            ui.close();
        }
    }
    // 같은 셸/SSH 세션을 하나 더 열기 — 팔레트(DuplicateTab)와 같은 동작을 한다(SSOT).
    if ui.button(tr(lang, "tab.duplicate")).clicked() {
        *duplicate = true;
        ui.close();
    }
    // SSH 탭이면 같은 연결로 SFTP 파일 브라우저 열기(자격증명 재사용).
    if is_ssh && ui.button(tr(lang, "tab.opensftp")).clicked() {
        *sftp_open = Some(*tab);
        ui.close();
    }
    ui.separator();
    if ui.button(tr(lang, "tab.close")).clicked() {
        orch.send(nabi_proto::Command::ClosePane { pane: *tab });
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::basename;

    #[test]
    fn shared_clean_title_shortens() {
        // 탭 제목 말줄임은 공유 clean_title로 일원화됨(중복 truncate_label 제거 회귀 가드).
        assert_eq!(nabi_render::clean_title("short", 24), "short");
        let out = nabi_render::clean_title(&"a".repeat(40), 24);
        assert_eq!(out.chars().count(), 24);
        assert!(out.ends_with('\u{2026}'));
    }

    #[test]
    fn basename_extracts_last_segment() {
        assert_eq!(basename("C:/a/b"), "b");
        assert_eq!(basename("/home/u/"), "u");
        assert_eq!(basename("solo"), "solo");
    }
}
