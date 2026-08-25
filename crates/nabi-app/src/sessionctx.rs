//! 저장 세션 공용 컨텍스트 메뉴 — 사이드바와 "세션 관리" 메뉴가 같은 항목/동작을 쓰도록 한 곳에 모은다(DRY·일관성).
//! 연결/SFTP/고정/메모/명령복사/호스트복사/연결테스트/편집/복제/그룹이동/삭제.

use crate::menu::MenuAction;
use nabi_session::{SavedSession, SessionKind};
use nabi_i18n::{tr, Lang};

/// 한 세션의 컨텍스트 메뉴 항목을 그리고, 선택된 동작을 돌려준다. `folders`는 "그룹 이동" 하위 목록.
pub(crate) fn session_menu_items(ui: &mut egui::Ui, s: &SavedSession, lang: Lang, folders: &[String]) -> Option<MenuAction> {
    let mut action = None;
    if ui.button(tr(lang, "qc.connect")).clicked() { action = Some(MenuAction::ConnectSaved(s.clone())); ui.close(); }
    if ui.button(tr(lang, "sessions.togglepin")).clicked() { action = Some(MenuAction::TogglePin(s.name.clone())); ui.close(); }
    if ui.button(tr(lang, "sessions.editnote")).clicked() { action = Some(MenuAction::EditNote(s.name.clone())); ui.close(); }
    if matches!(s.kind, SessionKind::Ssh { .. }) && !s.is_ftp && ui.button(tr(lang, "sessions.opensftp")).clicked() {
        action = Some(MenuAction::OpenSftp(s.clone()));
        ui.close();
    }
    // SSH 세션이면 ssh CLI 명령·호스트 복사(스크립트·공유용, 비밀 미포함) + 연결 테스트.
    if let SessionKind::Ssh { host, port, user, key_path, jump, .. } = &s.kind {
        if !s.is_ftp && ui.button(tr(lang, "ssh.copycmd")).clicked() {
            ui.ctx().copy_text(crate::sshcmd::ssh_command(host, *port, user, key_path.as_deref(), jump.as_deref()));
            ui.close();
        }
        if ui.button(tr(lang, "ssh.copyhost")).clicked() {
            ui.ctx().copy_text(if user.is_empty() { format!("{host}:{port}") } else { format!("{user}@{host}:{port}") });
            ui.close();
        }
        if ui.button(tr(lang, "sessions.testconn")).clicked() { action = Some(MenuAction::TestConnection(host.clone(), *port)); ui.close(); }
        if ui.button(tr(lang, "sessions.copyurl")).clicked() {
            if let Some(url) = crate::sshconfig::to_ssh_url(s) { ui.ctx().copy_text(url); }
            ui.close();
        }
    }
    if matches!(s.kind, SessionKind::Ssh { .. }) && ui.button(tr(lang, "fwd.auto")).clicked() {
        action = Some(MenuAction::EditAutoForwards(s.name.clone()));
        ui.close();
    }
    if ui.button(tr(lang, "sessions.edit")).clicked() { action = Some(MenuAction::EditSession(s.clone())); ui.close(); }
    if ui.button(tr(lang, "sessions.duplicate")).clicked() { action = Some(MenuAction::DuplicateSession(s.clone())); ui.close(); }
    // 분할로 연결(MobaXterm식) — 현재 pane 옆/아래에 이 세션을 새 분할로 연다.
    ui.menu_button(tr(lang, "sessions.splitconnect"), |ui| {
        if ui.button(tr(lang, "menu.splitright")).clicked() { action = Some(MenuAction::SplitConnect(s.clone(), true)); ui.close(); }
        if ui.button(tr(lang, "menu.splitdown")).clicked() { action = Some(MenuAction::SplitConnect(s.clone(), false)); ui.close(); }
    });
    // 표식(운영/스테이징/개발…) — 연결 전에 어떤 서버인지 알아보게 하는 안전장치다.
    ui.menu_button(tr(lang, "sessions.settag"), |ui| {
        for t in nabi_session::SessionTag::ALL {
            let (r, g, b) = t.rgb();
            let dot = egui::RichText::new("\u{25cf}").color(egui::Color32::from_rgb(r, g, b));
            let picked = ui
                .horizontal(|ui| {
                    ui.label(dot);
                    // 색만으로 구분하지 않는다 — 라벨을 늘 함께 둔다(색각 이상 배려).
                    ui.selectable_label(s.tag == t, tr(lang, t.key())).clicked()
                })
                .inner;
            if picked {
                action = Some(MenuAction::SetSessionTag(s.name.clone(), t));
                ui.close();
            }
        }
    });
    // 그룹 이동(DnD 대안) — 기존 그룹/그룹 없음으로.
    ui.menu_button(tr(lang, "sessions.movegroup"), |ui| {
        if s.folder.is_some() && ui.button(tr(lang, "sessions.nogroup")).clicked() {
            action = Some(MenuAction::MoveSessionToGroup(s.name.clone(), None));
            ui.close();
        }
        for f in folders.iter().filter(|f| s.folder.as_deref() != Some(f.as_str())) {
            if ui.button(format!("\u{1f4c1} {f}")).clicked() {
                action = Some(MenuAction::MoveSessionToGroup(s.name.clone(), Some(f.clone())));
                ui.close();
            }
        }
        // 새 그룹 만들기(백로그): 이름 입력 → Enter/+ 로 그 그룹으로 이동(그룹은 라벨이라 이동=생성).
        ui.separator();
        if let Some(new) = crate::sessionctx::inline_name_input(ui, egui::Id::new(("newgroup", &s.name)), "sessions.newgroup", lang) {
            action = Some(MenuAction::MoveSessionToGroup(s.name.clone(), Some(new)));
            ui.close();
        }
    });
    if ui.button(tr(lang, "sessions.delete")).clicked() { action = Some(MenuAction::DeleteSession(s.name.clone())); ui.close(); }
    action
}

/// 메뉴 안 한 줄 이름 입력(임시 상태는 egui 메모리) — Enter 또는 + 클릭 시 값 반환.
pub(crate) fn inline_name_input(ui: &mut egui::Ui, id: egui::Id, hint_key: &str, lang: nabi_i18n::Lang) -> Option<String> {
    let mut txt: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let mut out = None;
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut txt).hint_text(tr(lang, hint_key)).desired_width(130.0));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (enter || ui.small_button("+").clicked()) && !txt.trim().is_empty() {
            out = Some(txt.trim().to_string());
            txt.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, txt));
    out
}
