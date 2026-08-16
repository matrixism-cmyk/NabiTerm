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
    if ui.button(tr(lang, "sessions.edit")).clicked() { action = Some(MenuAction::EditSession(s.clone())); ui.close(); }
    if ui.button(tr(lang, "sessions.duplicate")).clicked() { action = Some(MenuAction::DuplicateSession(s.clone())); ui.close(); }
    // 분할로 연결(MobaXterm식) — 현재 pane 옆/아래에 이 세션을 새 분할로 연다.
    ui.menu_button(tr(lang, "sessions.splitconnect"), |ui| {
        if ui.button(tr(lang, "menu.splitright")).clicked() { action = Some(MenuAction::SplitConnect(s.clone(), true)); ui.close(); }
        if ui.button(tr(lang, "menu.splitdown")).clicked() { action = Some(MenuAction::SplitConnect(s.clone(), false)); ui.close(); }
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
    });
    if ui.button(tr(lang, "sessions.delete")).clicked() { action = Some(MenuAction::DeleteSession(s.name.clone())); ui.close(); }
    action
}
