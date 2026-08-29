//! egui_dock TabViewer — 각 탭이 한 pane의 터미널을 렌더한다.
//!
//! 탭은 PaneId만 보유하고 상태는 오케스트레이터에서 읽는다(원칙: UI는 PaneId만).

use crate::selection::Sel;
use nabi_orchestrator::OrchestratorHandle;
use nabi_proto::Command;
use nabi_types::{GridSize, PaneId};
use nabi_i18n::tr;
use nabi_vt::Theme;
use std::collections::HashMap;

/// 도킹 탭 뷰어. pane별 리사이즈 추적용 last_grid를 참조로 공유한다.
pub struct TermTabViewer<'a> {
    pub orch: &'a OrchestratorHandle,
    pub theme: Theme,
    pub font_size: f32,
    pub last_grid: &'a mut HashMap<PaneId, GridSize>,
    /// MultiExec: 켜지면 입력을 모든 pane에 브로드캐스트한다.
    pub broadcast: bool,
    /// 탭 제목에 pane ID(#N) 배지 표시.
    pub show_pane_ids: bool,
    /// Find(Ctrl+F) 검색어(있으면 일치 셀 하이라이트).
    pub find: Option<String>,
    /// 키워드 하이라이트 규칙(설정). 출력에서 이 단어들을 일치 색으로 표시.
    pub highlights: &'a [String],
    /// 동기 스크롤이 켜져 있나 — 켜져 있으면 굴릴 때 같은 그룹을 함께 옮긴다.
    pub sync_scroll: bool,
    /// 고정된 탭들 — 닫기 단추를 감추고 제목에 압정을 붙인다.
    pub pinned: &'a mut std::collections::HashSet<PaneId>,
    /// 사용자 지정 탭 이름(PaneId별). 비어 있으면 기본 제목 사용.
    pub tab_names: &'a mut HashMap<PaneId, String>,
    pub lang: nabi_i18n::Lang,
    /// 브로드캐스트 대상 그룹(비어 있으면 이 창의 터미널 전체). context_menu에서 토글.
    pub broadcast_group: &'a mut std::collections::HashSet<PaneId>,
    /// 휠을 키로 보낼 pane 집합(탭 컨텍스트 메뉴에서 켠다).
    pub wheel_keys: &'a mut std::collections::HashSet<PaneId>,
    /// pane별 마지막 Ctrl+T(오버레이 열기) 전송 시각(재전송 방지 래치).
    pub tui_overlay: &'a mut HashMap<PaneId, std::time::Instant>,
    /// 휠 도우미를 명시적으로 끈 pane(자동 감지 무시).
    pub wheel_keys_off: &'a mut std::collections::HashSet<PaneId>,
    /// 이 창(dock)에 속한 터미널 pane 전체 — 그룹 미지정 브로드캐스트의 대상 범위.
    pub window_panes: &'a std::collections::HashSet<PaneId>,
    /// 마우스 텍스트 선택 상태(드래그→릴리스 자동 복사).
    pub selection: &'a mut Option<Sel>,
    /// 탭 색상 라벨(PaneId별).
    pub tab_colors: &'a mut std::collections::HashMap<PaneId, egui::Color32>,
    /// 터미널 `파일:줄` 더블클릭 결과(경로, 0기반 줄) — dock 표시 후 NabiApp이 에디터로 연다.
    pub pending_pathline: &'a mut Option<(String, usize)>,
    /// 커서 깜빡임 on 프레임 여부.
    pub blink_on: bool,
    /// 드래그 선택 종료 시 자동 복사 여부.
    pub copy_on_select: bool,
    /// 이번 프레임에 **위험 표식**이 붙은 pane들(guard.rs). 표식은 앱만 알고,
    /// 보내는 자리는 여기다 — 순서를 지키려면 판단이 보내는 자리에 있어야 한다.
    pub risky_panes: &'a std::collections::HashSet<PaneId>,
    /// 위험 명령 확인을 켤 것인가(`terminal.guard_dangerous`).
    pub guard_dangerous: bool,
    /// 위험 명령 확인 대기(붙잡힌 입력).
    pub pending_send: &'a mut Option<crate::guard::PendingSend>,
    /// 활동(비포커스 출력) 표시 대상 pane 집합.
    pub activity: &'a std::collections::HashSet<PaneId>,
    /// pane 커스텀 상태(AI 도구 발행) — 있으면 탭에 🤖 배지(P5).
    pub pane_status: &'a HashMap<PaneId, std::collections::BTreeMap<String, String>>,
    /// pane에서 실행 중인 명령(셸 통합) — claude 등 AI면 탭에 🤖 자동 배지.
    pub run_cmd: &'a HashMap<PaneId, String>,
    /// AI 명령 바 표시 여부(설정 terminal.ai_cmd_bar — aicmdbar.rs).
    pub ai_cmd_bar: bool,
    /// trzsz 전송 진행률(읽기 전용 — 취소는 orch로 바로 보낸다).
    pub trzsz: &'a crate::trzszui::TrzszUi,
    /// pane별 모델·노력 선택 기억(명령 바 버튼에 현재 상태 표시).
    pub ai_picks: &'a mut HashMap<PaneId, crate::aicmdbar::AiPicks>,
    /// pane별 AI 화면 판독 캐시(모드·모델·노력·제목).
    pub ai_screen: &'a mut HashMap<PaneId, crate::aimode::AiScreen>,
    /// 설정에 저장된 마지막 선택(재시작 후 초기 표시값).
    pub ai_last_model: &'a str,
    pub ai_last_effort: &'a str,
    /// 이번 프레임에 고른 값 신호 — app이 설정에 저장한다.
    pub ai_pick_out: &'a mut Option<(String, String)>,
    /// 영문 팁 한글 오버레이 설정 + 캐시(tipoverlay.rs).
    pub tip_overlay: bool,
    pub tip_ai_on: bool,
    pub tip_cache: &'a mut HashMap<PaneId, crate::tipoverlay::TipHit>,
    pub tip_ai: &'a mut crate::tipai::TipAi,
    /// 명령 실행 중(OSC 133;C~D) pane — 탭에 ⚙ 상태 배지(CP-6).
    pub running: &'a std::collections::HashMap<PaneId, std::time::Instant>,
    /// 포커스 pane의 IME 조합 중 텍스트(커서에 오버레이 — 한글 조합 표시).
    pub ime_preedit: &'a mut String,
    /// 탭 바 "+" 버튼 클릭 신호(on_add에서 설정).
    pub add_requested: &'a mut bool,
    /// "+"가 눌린 (surface, node) — 새 탭을 그 분할에 두기 위해(엉뚱한 탭 생성 방지).
    pub add_target: &'a mut Option<(egui_dock::SurfaceIndex, egui_dock::NodeIndex)>,
    pub ssh_click: &'a mut Option<String>,
    pub link_click: &'a mut Option<(PaneId, String)>,
    pub focused: Option<PaneId>,
    /// 비활성 pane을 우클릭하면 그 pane으로 포커스 이동 요청(첫 클릭=활성화만, dock.show 뒤 적용).
    pub focus_req: &'a mut Option<PaneId>,
    /// 마우스 붙여넣기 요청 (pane, 원문). 확인 여부는 app이 한곳에서 정한다.
    pub paste_req: &'a mut Option<(PaneId, String)>,
    /// 이번 프레임 탭 컨텍스트 메뉴가 열려 있는지(빈 탭바 우클릭 메뉴 중복 표시 방지).
    pub tab_ctx_tab: &'a mut Option<PaneId>,
    pub pane_font: &'a std::collections::HashMap<PaneId, f32>,
    pub cwds: &'a std::collections::HashMap<PaneId, String>,
    /// 활성 원격 패널(현재 포커스된 SFTP/FTP 탭)과 그 PaneId.
    pub sftp: &'a mut crate::sftppanel::SftpPanel,
    pub sftp_pane: Option<PaneId>,
    /// 배경 원격 패널들(다른 SFTP/FTP 탭) — 제목/식별·분할보기 렌더용.
    pub sftp_bg: &'a HashMap<PaneId, crate::sftppanel::SftpPanel>,
    pub sftp_act: &'a mut crate::sftptab::SftpAct,
    /// SFTP 원격 경로 북마크(FileZilla식).
    pub sftp_bookmarks: &'a [String],
    /// 스스로 쌓인 최근 원격 경로(북마크와 같은 메뉴에 붙는다).
    pub sftp_recent: &'a [String],
    /// 최근 로컬 폴더(원격과 같은 규칙 — `recentpaths`).
    pub local_recent: &'a [String],
    /// 현재 정렬 기준·방향(원격 목록 헤더 표시용) — 로컬 브라우저와 공유하는 상태.
    pub sort: (crate::browserfs::Sort, bool),
    /// 닫힌 원격 탭의 PaneId(있으면 central에서 정리).
    pub sftp_closed: &'a mut Option<PaneId>,
    /// Ctrl+휠 확대/축소 요청 (포인터가 올라간 pane, 휠 부호). central에서 적용·포커스.
    pub zoom_req: &'a mut Option<(PaneId, f32)>,
    /// 포커스 pane이 이번 프레임 리사이즈됐으면 새 그리드 크기(central이 배지로 표시).
    pub resized: &'a mut Option<GridSize>,
    /// 탭을 창 바깥으로 끌어다 놓아 분리(tear-off)할 pane(central이 floating으로 이동).
    pub tear_off: &'a mut Option<PaneId>,
    /// 탭 우클릭 ▸ 탭 복제 요청(central에서 duplicate_pane 호출).
    pub dup_tab: &'a mut Option<PaneId>,
    /// 이 프레임에 각 탭이 차지한 자리(화면 좌표, 논리 픽셀).
    ///
    /// 화면 캡처와 내장 웹 브라우저가 **똑같이** 이것을 필요로 한다. 두 곳이 따로
    /// 재려고 하면 언젠가 서로 다른 값을 갖게 되므로 그릴 때 한 번만 적어 둔다.
    pub pane_rects: &'a mut HashMap<PaneId, egui::Rect>,
    /// "창 안에 띄우기" 신호 — 메인 창 안 오버레이(docked_float)로 이동할 pane(P3).
    pub dock_float: &'a mut Option<PaneId>,
    /// 탭으로 열린 브라우저들(독립 상태) + 수집 액션/닫힘 신호 + 비교맵/업로드 가능 여부.
    pub browser_tabs: &'a mut HashMap<PaneId, crate::browserpanel::BrowserPanel>,
    pub browser_act: &'a mut Vec<(PaneId, crate::browser::BrowserAct)>,
    pub browser_closed: &'a mut Option<PaneId>,
    pub remote_map: &'a std::collections::HashMap<String, (bool, u64)>,
    pub can_upload: bool,
    /// 내장 에디터 탭들 + 수집 액션(저장 등) + 닫힘 신호.
    pub editors: &'a mut HashMap<PaneId, crate::editor::EditorDoc>,
    /// nabiPad 최근 파일(파일 메뉴 ▸ 최근 파일).
    pub recent_files: &'a [String],
    pub editor_act: &'a mut Vec<(PaneId, crate::editor::EditorAct)>,
    pub editor_closed: &'a mut Option<PaneId>,
    /// 링크 길게 누름 메뉴 신호((URL, 위치)).
    pub link_menu: &'a mut Option<(String, egui::Pos2)>,
    /// 인라인 이미지(Sixel) 텍스처 캐시(이미지 id별 GPU 업로드 1회).
    pub img_textures: &'a mut HashMap<u64, egui::TextureHandle>,
    /// pane 출처(SSH 여부 판정 — 탭 우클릭 'SFTP 열기' 노출용) + SFTP 열기 신호.
    pub pane_origins: &'a HashMap<PaneId, nabi_session::SessionKind>,
    pub sftp_open: &'a mut Option<PaneId>,
    /// 탭 메뉴 'AI에 넘기기/마크다운 복사' 신호: (pane, copy_only).
    pub ai_handoff: &'a mut Option<(PaneId, bool)>,
}

impl egui_dock::TabViewer for TermTabViewer<'_> {
    type Tab = PaneId;

    /// egui_dock 내장 Eject(같은 창 안 떠있는 패널) 비활성 — 입력 누수·닫기 버그가 있어
    /// 자체 오버레이("창 안에 띄우기", docked_float)로 대체한다. false면 eject 버튼·드래그-창 모두 사라진다.
    fn allowed_in_windows(&self, _tab: &mut PaneId) -> bool {
        false
    }

    /// 탭을 끌어 창 밖에서 놓으면 별도 OS 창으로 분리(브라우저식 탭 떼어내기).
    /// 드래그 중에는 마우스 캡처로 창 밖 좌표도 전달되므로 릴리스 위치로 판별한다.
    fn on_tab_button(&mut self, tab: &mut PaneId, response: &egui::Response) {
        if response.drag_stopped() {
            let outside = response
                .ctx
                .pointer_latest_pos()
                .map(|p| !response.ctx.content_rect().contains(p))
                .unwrap_or(false);
            if outside {
                *self.tear_off = Some(*tab);
            }
        }
    }

    /// 탭 고유 ID — egui_dock 기본값은 "제목 텍스트"라 같은 제목(같은 폴더 브라우저,
    /// 같은 셸 이름 등)끼리 스크롤/인터랙션 상태가 공유돼 버린다. PaneId로 고정.
    fn id(&mut self, tab: &mut PaneId) -> egui::Id {
        egui::Id::new(("nabi_tab", tab.get()))
    }

    fn title(&mut self, tab: &mut PaneId) -> egui::WidgetText {
        // 압정은 맨 앞에 — 닫기 단추가 없는 까닭을 한눈에 알려 준다.
        let pin = if self.pinned.contains(tab) { "\u{1f4cc} " } else { "" };
        // pane ID 배지(설정 시) — 제어 평면 `nabi cli --pane <N>` 타깃 참조용.
        // 브라우저 탭은 UI 전용이라 제외(오케스트레이터 pane이 아님).
        let mut id = if self.show_pane_ids { format!("#{} ", tab.get()) } else { String::new() };
        if self.running.contains_key(tab) {
            id = format!("\u{2699} {id}"); // 명령 실행 중 상태 배지(CP-6).
        }
        // AI 배지: 상태를 발행하는 에이전트는 입력 대기(blocked)면 🔔(주의 필요), 아니면 🤖. 명령만 감지되면 🤖.
        if let Some(st) = self.pane_status.get(tab).filter(|m| !m.is_empty()) {
            let b = if crate::aistatus::agent_state(st, self.running.contains_key(tab)) == 2 { "\u{1f514}" } else { "\u{1f916}" };
            id = format!("{b} {id}");
        } else if self.run_cmd.get(tab).is_some_and(|c| crate::aistatus::is_ai_command(c)) {
            id = format!("\u{1f916} {id}");
        }
        if let Some(b) = self.browser_tabs.get(tab) {
            let name = b
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| b.path.display().to_string());
            return format!("{pin}\u{1f4c1} {name}").into();
        }
        if let Some(e) = self.editors.get(tab) {
            let star = if e.dirty { " \u{25cf}" } else { "" }; // 미저장 ● — 본문 헤더·VS Code와 통일.
            return format!("{pin}\u{270e} {}{star}", e.title).into();
        }
        if let Some(host) = self.remote_host(*tab) {
            // 원격 패널도 브라우저 탭처럼 UI 전용이라 pane ID 배지를 붙이지 않는다.
            // 붙이면 내부 채번(u64::MAX-n)이 그대로 새어 "#18446744073709551614"가 보인다.
            let h = if host.is_empty() { "SFTP".to_string() } else { host };
            return format!("{pin}\u{1f5a7} {h}").into();
        }
        let base =
            crate::tabmenu::tab_title(self.orch, self.tab_names, self.activity, self.cwds, tab);
        if self.broadcast && self.broadcast_group.contains(tab) {
            format!("{pin}{id}\u{21c9} {base}").into()
        } else {
            format!("{pin}{id}{base}").into()
        }
    }

    /// **고정한 탭에는 닫기 단추가 없다.** 고정의 뜻이 여기에 있다 — 실수로 누르는
    /// 일을 막는 것이지, 못 닫게 하는 것이 아니다(우클릭에서 고정을 풀면 닫힌다).
    fn closeable(&mut self, tab: &mut PaneId) -> bool {
        !self.pinned.contains(tab)
    }

    fn on_add(&mut self, path: egui_dock::NodePath) {
        *self.add_requested = true;
        *self.add_target = Some((path.surface, path.node)); // 클릭된 탭 바 위치를 기억(거기에 새 탭 생성).
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut PaneId, _path: egui_dock::NodePath) {
        *self.tab_ctx_tab = Some(*tab); // 어느 탭의 메뉴가 열려 있는지 기록(#3 중복 방지 판정용).
        // 고정은 맨 위에 둔다 — 메뉴가 길어 아래로 내려가면 못 찾는다.
        let pinned = self.pinned.contains(tab);
        let label = if pinned { "tab.unpin" } else { "tab.pin" };
        if ui.button(tr(self.lang, label)).clicked() {
            match pinned {
                true => self.pinned.remove(tab),
                false => self.pinned.insert(*tab),
            };
            ui.close();
        }
        ui.separator();
        let is_ssh = matches!(
            self.pane_origins.get(tab),
            Some(nabi_session::SessionKind::Ssh { .. })
        );
        // "창 안에 띄우기"는 오버레이가 터미널만 렌더하므로 터미널/SSH pane에서만 제공
        // (브라우저/에디터/SFTP는 "새 OS 창으로 분리"를 쓴다).
        let is_term = self.orch.panes.read().ok().is_some_and(|m| m.contains_key(tab))
            && !self.browser_tabs.contains_key(tab)
            && !self.editors.contains_key(tab)
            && self.remote_host(*tab).is_none();
        let mut dup = false;
        crate::tabmenu::tab_context_menu(
            ui, tab, self.orch, self.lang,
            self.tab_names, self.broadcast_group, self.wheel_keys, self.wheel_keys_off,
            self.run_cmd.get(tab).is_some_and(|c| crate::panewheel::is_tui_history_app(c)),
            self.tab_colors,
            is_ssh, self.tear_off, self.sftp_open, self.ai_handoff,
            if is_term { Some(&mut *self.dock_float) } else { None },
            &mut dup,
        );
        if dup {
            *self.dup_tab = Some(*tab);
        }
    }

    fn tab_style_override(
        &self,
        tab: &PaneId,
        global: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        crate::tabmenu::tab_style(self.tab_colors, tab, global)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut PaneId) {
        let pane = *tab;
        // 무엇을 그리든 자리는 같으니 맨 앞에서 적는다 — 아래 갈래마다 적으면 하나를 빠뜨린다.
        self.pane_rects.insert(pane, ui.max_rect());
        if let Some(b) = self.browser_tabs.get_mut(&pane) {
            // 로컬 파일 브라우저 탭 — 액션은 central이 적용.
            self.browser_act.push((
                pane,
                crate::browser::render_browser_tab(
                    ui, b, self.remote_map, self.can_upload, self.lang, pane.get(), self.local_recent,
                ),
            ));
            return;
        }
        if Some(pane) == self.sftp_pane {
            *self.sftp_act = crate::sftptab::render_sftp_tab(ui, self.sftp, self.lang, self.sftp_bookmarks, self.sftp_recent, self.sort);
            return;
        }
        if let Some(p) = self.sftp_bg.get(&pane) {
            // 배경 원격 탭(분할 보기 등) — 호스트만 표시(탭을 누르면 활성화됨).
            ui.label(format!("\u{1f5a7} {}", p.host));
            return;
        }
        if let Some(e) = self.editors.get_mut(&pane) {
            self.editor_act.push((pane, crate::editortab::render_editor_tab(ui, e, self.lang, self.recent_files)));
            return;
        }
        self.paint_term(ui, pane);
    }

    fn on_close(&mut self, tab: &mut PaneId) -> egui_dock::tab_viewer::OnCloseResponse {
        use egui_dock::tab_viewer::OnCloseResponse;
        if self.browser_tabs.contains_key(tab) {
            *self.browser_closed = Some(*tab); // UI 전용 탭 — 오케스트레이터 명령 없음.
            return OnCloseResponse::Close;
        }
        if self.editors.contains_key(tab) {
            *self.editor_closed = Some(*tab); // 정리는 central에서.
            return OnCloseResponse::Close;
        }
        if Some(*tab) == self.sftp_pane || self.sftp_bg.contains_key(tab) {
            *self.sftp_closed = Some(*tab); // 정리는 central에서(닫힘 자체도 central이 수행).
            return OnCloseResponse::Ignore;
        }
        self.orch.send(Command::ClosePane { pane: *tab });
        self.last_grid.remove(tab);
        OnCloseResponse::Close
    }
}

impl TermTabViewer<'_> {
    /// pane이 원격(SFTP/FTP) 탭이면 그 호스트를, 아니면 None.
    fn remote_host(&self, pane: PaneId) -> Option<String> {
        if Some(pane) == self.sftp_pane {
            Some(self.sftp.host.clone())
        } else {
            self.sftp_bg.get(&pane).map(|p| p.host.clone())
        }
    }
}
