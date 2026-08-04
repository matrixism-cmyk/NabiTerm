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

#[allow(clippy::too_many_arguments)]
pub(crate) fn tab_context_menu(
    ui: &mut egui::Ui,
    tab: &PaneId,
    orch: &OrchestratorHandle,
    lang: Lang,
    tab_names: &mut HashMap<PaneId, String>,
    broadcast_group: &mut HashSet<PaneId>,
    tab_colors: &mut HashMap<PaneId, egui::Color32>,
    is_ssh: bool,
    tear_off: &mut Option<PaneId>,
    sftp_open: &mut Option<PaneId>,
    dock_float: Option<&mut Option<PaneId>>,
) {
    ui.label(tr(lang, "tab.rename"));
    let name = tab_names.entry(*tab).or_default();
    // 포커스를 확실히 잡아 키 입력이 터미널로 새지 않게 한다.
    ui.text_edit_singleline(name).request_focus();
    if ui.button(tr(lang, "tab.reset")).clicked() {
        tab_names.remove(tab);
        ui.close_menu();
    }
    if ui.button(tr(lang, "term.reset")).clicked() {
        if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) {
            if let Ok(mut md) = v.model.lock() {
                md.reset();
            }
        }
        ui.close_menu();
    }
    // 스크롤백(히스토리)만 비우기 — 화면 유지(Clear Buffer).
    if ui.button(tr(lang, "term.clearbuf")).clicked() {
        if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) {
            if let Ok(mut md) = v.model.lock() { md.clear_scrollback(); }
        }
        ui.close_menu();
    }
    // 출력(히스토리+화면)을 파일로 저장(세션 로그).
    let dump = || orch.panes.read().ok().and_then(|m| m.get(tab).cloned()).and_then(|v| v.model.lock().ok().map(|md| md.dump_text(1_000_000)));
    if ui.button(tr(lang, "menu.saveoutput")).clicked() {
        if let Some(text) = dump() {
            if let Some(path) = rfd::FileDialog::new().set_file_name("terminal-output.txt").save_file() {
                let _ = std::fs::write(path, text);
            }
        }
        ui.close_menu();
    }
    // 출력을 클립보드로 복사(파일 저장 없이) + AI용 마크다운 코드블록 복사(바이브코딩).
    if ui.button(tr(lang, "term.copyoutput")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(t); }
        ui.close_menu();
    }
    if ui.button(tr(lang, "term.copyoutputmd")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(format!("```\n{}\n```", t.trim_end())); }
        ui.close_menu();
    }
    // 출력에서 URL/IP만 추출해 클립보드로(로그에서 링크·주소 일괄 수집).
    if ui.button(tr(lang, "term.exturls")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(crate::editorextract::extract_urls(&t)); }
        ui.close_menu();
    }
    if ui.button(tr(lang, "term.extips")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(crate::editorextract::extract_ips(&t)); }
        ui.close_menu();
    }
    if ui.button(tr(lang, "term.extemails")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(crate::editorextract::extract_emails(&t)); }
        ui.close_menu();
    }
    if ui.button(tr(lang, "term.extnumbers")).clicked() {
        if let Some(t) = dump() { ui.ctx().copy_text(crate::editorextract::extract_numbers(&t)); }
        ui.close_menu();
    }
    // 스크롤백 맨 위/아래로 이동(긴 출력 빠른 탐색).
    let scroll = |top: bool| { if let Some(v) = orch.panes.read().ok().and_then(|m| m.get(tab).cloned()) { if let Ok(mut md) = v.model.lock() { if top { md.scroll_to_top(); } else { md.scroll_to_bottom(); } } } };
    if ui.button(tr(lang, "term.scrolltop")).clicked() { scroll(true); ui.close_menu(); }
    if ui.button(tr(lang, "term.scrollbottom")).clicked() { scroll(false); ui.close_menu(); }
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
        ui.close_menu();
    }
    if let Some(dock_float) = dock_float {
        if ui
            .button(tr(lang, "tab.dockfloat"))
            .on_hover_text(tr(lang, "tab.dockfloat.hint"))
            .clicked()
        {
            *dock_float = Some(*tab);
            ui.close_menu();
        }
    }
    // SSH 탭이면 같은 연결로 SFTP 파일 브라우저 열기(자격증명 재사용).
    if is_ssh && ui.button(tr(lang, "tab.opensftp")).clicked() {
        *sftp_open = Some(*tab);
        ui.close_menu();
    }
    ui.separator();
    if ui.button(tr(lang, "tab.close")).clicked() {
        orch.send(nabi_proto::Command::ClosePane { pane: *tab });
        ui.close_menu();
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
