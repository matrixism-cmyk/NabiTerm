//! 커맨드 팔레트(Ctrl+Shift+P): 메뉴 액션을 퍼지 검색으로 즉시 실행.
//!
//! 메뉴 액션을 라벨 목록으로 노출하고, 부분 문자열 필터 + Enter(첫 결과)/클릭으로 실행한다.

use crate::app::NabiApp;
use nabi_i18n::{tr, Lang};
use nabi_proto::ShellKind;
use std::path::PathBuf;

pub(crate) enum PaletteAction {
    /// git 워크트리 만들기/목록(B6).
    WorktreeCreate,
    WorktreeList,
    NewLocal(ShellKind),
    /// AI 터미널 프로필 i번으로 새 터미널(aiprof.rs).
    NewAiProfile(usize),
    /// AI 터미널 프로필 관리 독립창.
    AiProfiles,
    OpenRecentFile(PathBuf),
    ConnectSession(nabi_session::SavedSession),
    OpenSftp(nabi_session::SavedSession),
    DuplicateTab,
    ReopenClosed,
    CloseOthers,
    SelectAll,
    ZoomPane,
    PrevPrompt,
    NextPrompt,
    ResetTerm,
    QuickConnect,
    ToggleBroadcast,
    TearOff,
    DockFloat,
    ArrangeTile,
    ArrangeCascade,
    ToggleBrowser,
    OpenBrowserTab,
    /// 열린 모든 창의 스크롤백을 한 번에 검색.
    FindAll,
    ToggleSessionsPanel,
    ToggleStatusBar,
    OpenSettings,
    OpenTelegram,
    OpenVault,
    OpenKnownHosts,
    SaveOutput, EditScrollback,
    OpenForward,
    SaveWorkspace,
    RestoreWorkspace,
    OpenConfigDir,
    ToggleOnTop,
    ZoomIn,
    ZoomOut,
    SetLang(Lang),
    SendSnippet(String),
    AiDashboard, ToggleFloatOnTop, CopyLastOutput, JumpDir(String), QuickSelect,
    RunHistory(String), PasteClip(String), FocusPane(nabi_types::PaneId),
    DuplicateConnection, ToggleSessionLog, NewTabHere, ClearBuffer,
    SyncUpload, SyncDownload, CopyOutputMd, CompareFiles, FindDuplicates, FindLargeFiles,
    CopySshCmd, GenSshKey, InstallPubkey, SelToPad, CopyTabsMd, SaveAllDocs,
    NewPad, OpenFileDialog, ScrollBottom, ScrollTop, ReplaceInFiles, DirTree, DirStats,
    /// 설정 ▸ 스케줄 페이지 바로 열기 / 도움말 ▸ AI 제어 페이지 바로 열기(T3-1 도구 메뉴).
    OpenSchedule, OpenAiCli,
    /// 워크스페이스 스냅샷 저장/목록(T7-2).
    SnapshotSave, SnapshotList,
    /// 브로드캐스트 결과 집계 뷰(T7-3).
    BroadcastResults,
    /// nabiPad: 커서 심볼의 정의로 이동/심볼 정보/참조 찾기(T6-4 LSP).
    GotoDefinition, LspHover, LspRefs, LspFormat,
    /// SSH ed25519 키 생성 모달.
    OpenKeygen,
    /// 폴더 동기화 다이얼로그(S6-51).
    OpenSync,
    /// 마지막 명령 출력 AI 인계/마크다운 복사(터미널→AI 동선).
    HandoffLast, CopyLastMd,
    /// SFTP 전송 히스토리 창(S6-60).
    XferHistory,
}

impl NabiApp {
    pub(crate) fn show_command_palette(&mut self, ctx: &egui::Context) {
        // Ctrl+Shift+P 토글, Esc 닫기.
        let toggle = ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::P));
        if toggle {
            self.palette_open = !self.palette_open;
            self.palette_query.clear();
        }
        if !self.palette_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.palette_open = false;
            return;
        }

        let lang = self.lang;
        let t = &self.config.terminal;
        let mut cmds = crate::palettecmds::palette_commands(
            lang,
            &self.sessions,
            &self.editor_config.recent_files,
            &t.snippets,
            &t.dir_visits,
            &t.cmd_history,
            &self.clip_history,
        );
        // 새 AI 터미널 프로필(세션 메뉴와 동일 동선 — 드리프트 금지).
        for (i, p) in t.ai_profiles.iter().enumerate() {
            cmds.push((format!("{}: {} — {}", tr(lang, "menu.newai"), p.name, p.cmd), PaletteAction::NewAiProfile(i)));
        }
        if let Ok(ps) = self.orch.panes.read() {
            // F2 열린 pane 전환 — SFTP 전용 pane은 탭이 아니라 패널이라 제외한다.
            for (p, v) in ps.iter() {
                if Some(*p) != self.sftp_pane && !self.sftp_bg.contains_key(p) {
                    cmds.push((format!("\u{2b1c} {}", v.title), PaletteAction::FocusPane(*p)));
                }
            }
        }
        let fcwd = self.focused_pane().and_then(|p| self.cwds.get(&p)).map(|c| crate::workspace::strip_uri_slash(c)).unwrap_or_default(); // F3 컨텍스트 명령(cwd 매칭)
        if !fcwd.is_empty() {
            // 같은 디렉터리에서 쓴 명령을 📍로 앞에 세운다(F3).
            for cmd in crate::cmdhist::recent_in_cwd(&self.config.terminal.cmd_history, &fcwd, 15) {
                let s: String = cmd.chars().take(50).collect();
                cmds.push((format!("\u{1f4cd} {s}"), PaletteAction::RunHistory(cmd)));
            }
        }
        let q = self.palette_query.to_lowercase();
        let mut chosen: Option<usize> = None;
        let mut enter = false;
        let mut open = true;

        egui::Window::new(tr(lang, "palette.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .default_width(440.0)
            .show(ctx, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text(tr(lang, "palette.hint"))
                        .desired_width(f32::INFINITY),
                );
                nabi_editor::uiutil::focus_once(&resp); // 매 프레임 request_focus는 IME 조합 파괴(egui 0.36).
                enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.separator();
                egui::ScrollArea::vertical().id_salt("palette_scroll").max_height(320.0).show(ui, |ui| {
                    let mut first: Option<usize> = None;
                    for (i, (label, _)) in cmds.iter().enumerate() {
                        if !q.is_empty() && !fuzzy_match(&label.to_lowercase(), &q) {
                            continue;
                        }
                        if first.is_none() {
                            first = Some(i);
                        }
                        if ui.selectable_label(false, label).clicked() {
                            chosen = Some(i);
                        }
                    }
                    if enter {
                        if let Some(f) = first {
                            chosen = Some(f);
                        }
                    }
                });
            });

        if let Some(i) = chosen {
            if let Some((_, act)) = cmds.into_iter().nth(i) {
                self.run_palette(ctx, act);
            }
            self.palette_open = false;
            self.palette_query.clear();
        } else if !open {
            self.palette_open = false;
        }
    }
}

/// 부분순서(서브시퀀스) 매치: needle의 글자가 순서대로 hay에 나타나면 true.
fn fuzzy_match(hay: &str, needle: &str) -> bool {
    let mut it = hay.chars();
    needle.chars().all(|nc| it.any(|hc| hc == nc))
}

/// 팔레트에 노출할 (라벨, 액션) 목록을 현재 언어로 만든다(저장 세션 포함).
#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn fuzzy_subsequence() {
        assert!(fuzzy_match("new tab", "nt"));
        assert!(fuzzy_match("new tab", "newtab"));
        assert!(!fuzzy_match("new tab", "tn"));
        assert!(fuzzy_match("anything", ""));
    }
}
