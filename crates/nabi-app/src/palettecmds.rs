//! 명령 팔레트 항목 목록 빌더 — palette.rs에서 분리(라인 한도·관심사 분리).
//! 고정 명령 + 셸/언어/최근파일/디렉터리점프/히스토리/클립/스니펫/세션을 한 Vec로 모은다.

use crate::palette::PaletteAction;
use nabi_i18n::{tr, Lang};
use std::path::PathBuf;

pub(crate) fn palette_commands(
    lang: Lang,
    sessions: &nabi_session::SessionTree,
    recent: &[String],
    snippets: &[String],
    dirs: &std::collections::BTreeMap<String, (u32, i64)>,
    hist: &[(String, String, i32, i64)],
    clips: &[String],
) -> Vec<(String, PaletteAction)> {
    let mut v = vec![
        (tr(lang, "wt.create").to_string(), PaletteAction::WorktreeCreate),
        (tr(lang, "wt.list").to_string(), PaletteAction::WorktreeList),
        (tr(lang, "tab.duplicate").to_string(), PaletteAction::DuplicateTab),
        (tr(lang, "tab.reopen").to_string(), PaletteAction::ReopenClosed),
        (tr(lang, "tab.closeothers").to_string(), PaletteAction::CloseOthers),
        (tr(lang, "tab.selectall").to_string(), PaletteAction::SelectAll),
        (tr(lang, "menu.zoompane").to_string(), PaletteAction::ZoomPane),
        (tr(lang, "prompt.prev").to_string(), PaletteAction::PrevPrompt),
        (tr(lang, "prompt.next").to_string(), PaletteAction::NextPrompt),
        (tr(lang, "prompt.prevfail").to_string(), PaletteAction::PrevFailed),
        (tr(lang, "prompt.nextfail").to_string(), PaletteAction::NextFailed),
        (tr(lang, "blocks.title").to_string(), PaletteAction::BlockList),
        (tr(lang, "mark.toggle").to_string(), PaletteAction::ToggleMark),
        (tr(lang, "mark.prev").to_string(), PaletteAction::PrevMark),
        (tr(lang, "mark.next").to_string(), PaletteAction::NextMark),
        (tr(lang, "mark.clear").to_string(), PaletteAction::ClearMarks),
        (tr(lang, "sftp.find.title").to_string(), PaletteAction::SftpFind),
        (tr(lang, "connhist.title").to_string(), PaletteAction::ConnHistory),
        (tr(lang, "term.reset").to_string(), PaletteAction::ResetTerm),
        (tr(lang, "menu.quickconnect").to_string(), PaletteAction::QuickConnect),
        (tr(lang, "menu.browser").to_string(), PaletteAction::ToggleBrowser),
        (tr(lang, "menu.browsertab").to_string(), PaletteAction::OpenBrowserTab),
        (tr(lang, "hist.open").to_string(), PaletteAction::OpenPaneHistory),
        (tr(lang, "findall.title").to_string(), PaletteAction::FindAll),
        (tr(lang, "menu.sessionspanel").to_string(), PaletteAction::ToggleSessionsPanel),
        (tr(lang, "settings.statusbar").to_string(), PaletteAction::ToggleStatusBar),
        // 메뉴에만 있던 전역 동작(§메뉴/IA 정리) — 라벨은 메뉴와 **같은 키**를 쓴다.
        // 다른 낱말로 부르면 같은 기능이 두 이름을 갖게 되고, 그것이 곧 드리프트다.
        (tr(lang, "menu.fullscreen").to_string(), PaletteAction::Fullscreen),
        (tr(lang, "tab.tile").to_string(), PaletteAction::TileTabs),
        (tr(lang, "tab.merge").to_string(), PaletteAction::MergeTabs),
        (tr(lang, "menu.qcbar").to_string(), PaletteAction::ToggleQcBar),
        (tr(lang, "menu.aicmdbar").to_string(), PaletteAction::ToggleAiCmdBar),
        (tr(lang, "menu.help").to_string(), PaletteAction::About),
        (tr(lang, "menu.tearoff").to_string(), PaletteAction::TearOff),
        (tr(lang, "tab.dockfloat").to_string(), PaletteAction::DockFloat),
        (tr(lang, "arrange.tile").to_string(), PaletteAction::ArrangeTile),
        (tr(lang, "arrange.cascade").to_string(), PaletteAction::ArrangeCascade),
        (tr(lang, "menu.broadcast").to_string(), PaletteAction::ToggleBroadcast),
        (tr(lang, "menu.syncscroll").to_string(), PaletteAction::ToggleSyncScroll),
        (tr(lang, "menu.settings").to_string(), PaletteAction::OpenSettings),
        (tr(lang, "settings.sec.telegram").to_string(), PaletteAction::OpenTelegram),
        (tr(lang, "menu.vault").to_string(), PaletteAction::OpenVault),
        (tr(lang, "menu.dupconn").to_string(), PaletteAction::DuplicateConnection),
        (tr(lang, "menu.sessionlog").to_string(), PaletteAction::ToggleSessionLog),
        (tr(lang, "menu.newtabhere").to_string(), PaletteAction::NewTabHere),
        (tr(lang, "term.clearbuf").to_string(), PaletteAction::ClearBuffer),
        (tr(lang, "sftp.syncup").to_string(), PaletteAction::SyncUpload),
        (tr(lang, "sftp.syncdown").to_string(), PaletteAction::SyncDownload),
        (tr(lang, "knownhosts.title").to_string(), PaletteAction::OpenKnownHosts),
        (tr(lang, "menu.saveoutput").to_string(), PaletteAction::SaveOutput),
        (tr(lang, "cmd.editscrollback").to_string(), PaletteAction::EditScrollback),
        (tr(lang, "menu.localforward").to_string(), PaletteAction::OpenForward),
        (tr(lang, "menu.saveworkspace").to_string(), PaletteAction::SaveWorkspace),
        (tr(lang, "menu.restoreworkspace").to_string(), PaletteAction::RestoreWorkspace),
        (tr(lang, "menu.configdir").to_string(), PaletteAction::OpenConfigDir),
        (tr(lang, "menu.ontop").to_string(), PaletteAction::ToggleOnTop),
        (tr(lang, "menu.zoomin").to_string(), PaletteAction::ZoomIn),
        (tr(lang, "menu.zoomout").to_string(), PaletteAction::ZoomOut),
        (tr(lang, "ai.dashboard").to_string(), PaletteAction::AiDashboard),
        (tr(lang, "float.ontop").to_string(), PaletteAction::ToggleFloatOnTop),
        (tr(lang, "cmd.copyoutput").to_string(), PaletteAction::CopyLastOutput),
        (tr(lang, "cmd.copyoutputmd").to_string(), PaletteAction::CopyOutputMd),
        (tr(lang, "diff.compare").to_string(), PaletteAction::CompareFiles),
        (tr(lang, "dup.title").to_string(), PaletteAction::FindDuplicates),
        (tr(lang, "large.title").to_string(), PaletteAction::FindLargeFiles),
        (tr(lang, "ssh.copycmd").to_string(), PaletteAction::CopySshCmd),
        (tr(lang, "ssh.genkey").to_string(), PaletteAction::GenSshKey),
        (tr(lang, "ssh.installkey").to_string(), PaletteAction::InstallPubkey),
        (tr(lang, "cmd.seltopad").to_string(), PaletteAction::SelToPad),
        (tr(lang, "cmd.copytabsmd").to_string(), PaletteAction::CopyTabsMd),
        (tr(lang, "cmd.saveall").to_string(), PaletteAction::SaveAllDocs),
        (tr(lang, "menu.newpad").to_string(), PaletteAction::NewPad),
        (tr(lang, "nabipad.openinwindow").to_string(), PaletteAction::PadInWindow),
        (tr(lang, "nabipad.openfile").to_string(), PaletteAction::OpenFileDialog),
        (tr(lang, "term.scrollbottom").to_string(), PaletteAction::ScrollBottom),
        (tr(lang, "term.scrolltop").to_string(), PaletteAction::ScrollTop),
        (tr(lang, "replace.title").to_string(), PaletteAction::ReplaceInFiles),
        (tr(lang, "dir.tree").to_string(), PaletteAction::DirTree),
        (tr(lang, "dir.stats").to_string(), PaletteAction::DirStats),
        (tr(lang, "qsel.title").to_string(), PaletteAction::QuickSelect),
        (tr(lang, "snap.save").to_string(), PaletteAction::SnapshotSave),
        (tr(lang, "snap.list").to_string(), PaletteAction::SnapshotList),
        (tr(lang, "bcast.results").to_string(), PaletteAction::BroadcastResults),
        (tr(lang, "lsp.gotodef").to_string(), PaletteAction::GotoDefinition),
        (tr(lang, "lsp.hover.pal").to_string(), PaletteAction::LspHover),
        (tr(lang, "lsp.refs.pal").to_string(), PaletteAction::LspRefs),
        (tr(lang, "lsp.format.pal").to_string(), PaletteAction::LspFormat),
        (tr(lang, "keygen.title").to_string(), PaletteAction::OpenKeygen),
        (tr(lang, "menu.importscreen").to_string(), PaletteAction::OpenImportScreen),
        (tr(lang, "copyid.title").to_string(), PaletteAction::CopyId),
        (tr(lang, "env.title").to_string(), PaletteAction::OpenEnvMgr),
        (tr(lang, "web.title").to_string(), PaletteAction::OpenWeb),
        (tr(lang, "cmdhist.title").to_string(), PaletteAction::OpenCmdHistory),
        (tr(lang, "bundle.title").to_string(), PaletteAction::OpenSupportBundle),
        (tr(lang, "block.copy").to_string(), PaletteAction::CopyCommandBlock),
        (tr(lang, "reach.all").to_string(), PaletteAction::CheckAllReachable),
        (tr(lang, "editor.reopenclosed").to_string(), PaletteAction::ReopenClosedDoc),
        (tr(lang, "sync.title").to_string(), PaletteAction::OpenSync),
        (tr(lang, "handoff.last").to_string(), PaletteAction::HandoffLast),
        (tr(lang, "handoff.copymd").to_string(), PaletteAction::CopyLastMd),
        (tr(lang, "sftp.history").to_string(), PaletteAction::XferHistory),
        (tr(lang, "settings.sec.schedule").to_string(), PaletteAction::OpenSchedule),
        // 도구 메뉴에는 있는데 팔레트에서는 못 찾던 둘 — 아래 대조 시험이 이제 지킨다.
        (tr(lang, "replay.title").to_string(), PaletteAction::OpenReplay),
        (tr(lang, "trail.title").to_string(), PaletteAction::OpenAgentTrail),
        (tr(lang, "help.agent.title").to_string(), PaletteAction::OpenAiCli),
        (tr(lang, "aiprof.manage").to_string(), PaletteAction::AiProfiles),
    ];
    for (label, shell) in crate::menu::installed_shells() {
        v.push((format!("{}: {label}", tr(lang, "menu.newlocal")), PaletteAction::NewLocal(shell)));
    }
    for l in Lang::all() {
        v.push((format!("{}: {}", tr(lang, "menu.language"), l.label()), PaletteAction::SetLang(l)));
    }
    for p in recent {
        let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
        v.push((format!("{}: {name}", tr(lang, "menu.recentfile")), PaletteAction::OpenRecentFile(PathBuf::from(p))));
    }
    for d in crate::dirjump::top(dirs, chrono::Local::now().timestamp(), 12) { v.push((format!("\u{1f4c1} {d}"), PaletteAction::JumpDir(d.clone()))); } // E1 디렉터리 점프
    for (cmd, _cwd, _e) in crate::cmdhist::recent(hist, 30) {
        let s: String = cmd.chars().take(50).collect(); // E5 명령 히스토리.
        v.push((format!("\u{2318} {s}"), PaletteAction::RunHistory(cmd)));
    }
    for c in clips.iter().take(30) {
        let s: String = c.chars().take(50).collect(); // F1 클립보드 히스토리.
        v.push((format!("\u{1f4cb} {}", s.replace('\n', " ")), PaletteAction::PasteClip(c.clone())));
    }
    for cmd in snippets { let preview: String = cmd.chars().take(40).collect(); v.push((format!("{}: {preview}", tr(lang, "menu.snippets")), PaletteAction::SendSnippet(cmd.clone()))); }
    for s in &sessions.sessions {
        if !s.name.is_empty() {
            v.push((format!("{}: {}", tr(lang, "menu.sessions"), s.name), PaletteAction::ConnectSession(s.clone())));
            if matches!(s.kind, nabi_session::SessionKind::Ssh { .. }) {
                v.push((format!("SFTP: {}", s.name), PaletteAction::OpenSftp(s.clone())));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    /// 메뉴에서 부르는 팔레트 동작은 **팔레트에서도 찾을 수 있어야 한다.**
    ///
    /// 메뉴는 마우스로 훑는 사람의 길이고 팔레트는 손을 안 떼는 사람의 길이다. 같은
    /// 기능이 한쪽에만 있으면, 그쪽을 쓰지 않는 사람에게는 없는 기능이 된다.
    ///
    /// 실제로 둘이 그랬다 — 재생과 에이전트 기록은 도구 메뉴에만 있었다. 손으로 두
    /// 목록을 맞춰 두면 언젠가 또 달라지므로 **소스를 훑어 대조한다.**
    #[test]
    fn 메뉴에_있는_것은_팔레트에서도_찾을_수_있다() {
        let menus = concat!(
            include_str!("toolsmenu.rs"),
            include_str!("viewmenu.rs"),
            include_str!("sessionsmenu.rs"),
        );
        let listed = include_str!("palettecmds.rs");
        let mut missing = Vec::new();
        for (i, _) in menus.match_indices("PaletteAction::") {
            let name: String = menus[i + "PaletteAction::".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();
            // 인자를 받는 것은 그때그때 만들어 넣으므로 이 대조에서 뺀다.
            let takes_arg = menus[i..].chars().nth("PaletteAction::".len() + name.len()) == Some('(');
            if name.is_empty() || takes_arg {
                continue;
            }
            let used = format!("PaletteAction::{name}),");
            if !listed.contains(&used) && !missing.contains(&name) {
                missing.push(name);
            }
        }
        assert!(missing.is_empty(), "메뉴에는 있는데 팔레트 목록에 없다: {missing:?}");
    }
}
