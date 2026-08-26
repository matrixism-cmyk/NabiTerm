//! SFTP 패널 본문 렌더 — ui에 그리며 발생 액션을 SftpAct로 모은다(처리는 sftpact.rs).

use crate::sftppanel::SftpPanel;
use crate::sftpqueue::{show_queue, QueueAct};
use nabi_i18n::{tr, Lang};

/// SFTP 패널 렌더 중 수집된 사용자 액션.
#[derive(Default)]
pub(crate) struct SftpAct {
    pub go: Option<String>,
    pub dl: Option<(String, u64)>,
    /// 원격 명령: (이름, 고른 명령).
    pub run_cmd: Option<(String, crate::remotecmd::RemoteOp)>,
    /// 서버 안에서 사본 만들기: (이름, 크기).
    pub copy_here: Option<(String, u64, bool)>,
    pub del: Option<String>,
    pub dldir: Option<String>,
    /// 폴더 크기 재귀 계산.
    pub dirsize: Option<String>,
    /// 이 폴더에서 SSH 터미널 열기(빠른연결 프리필).
    pub open_term: Option<String>,
    /// SFTP 세션 재연결(빠른연결 프리필로 비밀번호 재입력).
    pub reconnect: bool,
    pub dl_cur: bool,
    pub edit: Option<String>,
    /// 원격 파일 앞부분 미리보기.
    pub preview: Option<String>,
    pub chmod: Option<(String, u32)>,
    /// 권한 재귀 적용(하위 전부).
    pub chmod_rec: Option<(String, u32)>,
    pub rename: Option<String>,
    pub do_rename: bool,
    pub do_mkdir: bool,
    /// 새 폴더/파일을 기본 이름으로 즉시 생성 후 인라인 이름변경(상단 입력박스 대신).
    pub new_folder: bool,
    pub new_file: bool,
    /// OS 클립보드(CF_HDROP) 파일을 현재 SFTP 폴더로 붙여넣기 업로드(FileZilla식).
    pub paste: bool,
    pub do_del: bool,
    pub cancel_del: bool,
    /// 전송 큐에서 모은 동작(일시정지·순서변경·제거·재시도).
    pub queue: QueueAct,
    /// 디렉터리 동기화 버튼(업로드/다운로드 — 차이 파일만).
    pub sync_up: bool,
    pub sync_down: bool,
    /// 동기화 미리보기 확인: 적용 / 취소.
    pub sync_apply: bool,
    /// 서버에서 파일 찾기 창 열기.
    pub open_find: bool,
    pub sync_cancel: bool,
    pub close: bool,
    pub over: bool,
    pub cycle_sort: bool,
    pub toggle_compare: bool,
    pub toggle_sync: bool,
    pub search: bool,
    /// 고른 원격 파일 둘을 내려받아 비교.
    pub compare: bool,
    pub batch_toggle: bool,
    pub batch_apply: bool,
    /// 앱 내 DnD로 떨어진 로컬 경로(업로드).
    pub upload_path: Option<String>,
    /// 폴더 행에 드롭한 (폴더명, 로컬경로) — 그 폴더로 업로드.
    pub upload_into: Option<(String, String)>,
    /// 컬럼 헤더 클릭으로 선택한 정렬 기준.
    pub set_sort: Option<crate::browserfs::Sort>,
    /// 단일 클릭 선택: (이름, ctrl, shift).
    pub select: Option<(String, bool, bool)>,
    /// 마우스 엄지 버튼: 뒤로/앞으로 히스토리 이동.
    pub back: bool,
    pub fwd: bool,
    /// 북마크: 현재 경로 추가 / 선택 경로로 이동 / 삭제.
    pub bookmark_add: bool, pub bookmark_go: Option<String>, pub bookmark_del: Option<String>,
    pub rect: Option<egui::Rect>, // 패널 화면 rect(OS 파일 드롭 위치 판정).
    pub os_drag: Option<(String, u64, bool)>, // 탐색기로 드래그-아웃(이름, 크기, 폴더 여부).
}

/// SFTP 패널 본문을 ui에 그리고 액션을 모은다(사이드 패널/탭 공용).
pub(crate) fn render_sftp_tab(ui: &mut egui::Ui, sftp: &mut SftpPanel, lang: Lang, bookmarks: &[String], recent: &[String], sort: (crate::browserfs::Sort, bool)) -> SftpAct {
    let mut a = SftpAct {
        // max_rect 기준(시작 시점 min_rect는 비어 있음) — 엄지 버튼/드롭 판정용.
        over: ui.rect_contains_pointer(ui.max_rect()),
        ..Default::default()
    };
    // 키보드 탐색(F5/Backspace/↑↓/Enter) + 마우스 엄지 버튼 — 로컬 브라우저와 통일.
    crate::sftptable::keyboard_nav(ui, sftp, &mut a);
    if sftp.font_size < 8.0 {
        sftp.font_size = 13.0; // 지연 초기화(Default 파생이라 0).
    }
    // Ctrl+휠로 이 SFTP 패널만 확대/축소(글꼴 크기 — 전역 영향 없음).
    let dz = crate::renameui::ctrl_wheel_zoom(ui, a.over);
    if dz != 0.0 {
        sftp.font_size = (sftp.font_size + dz).clamp(9.0, 28.0);
    }
    crate::sftptoolbar::render_toolbar(ui, sftp, lang, bookmarks, recent, sort.1, &mut a);
    if !sftp.status.is_empty() {
        ui.horizontal(|ui| {
            // 진행 중(연결/전송/검색은 '…'로 끝남)이면 스피너 표시.
            if sftp.status.ends_with('\u{2026}') {
                ui.spinner();
            }
            ui.colored_label(crate::theme_ui::ACCENT, &sftp.status);
        });
    }
    // 동기화 미리보기: 적용 전 대상 파일 수·목록을 보여주고 확인(WinSCP식 preview).
    if let Some((up, files)) = &sftp.sync_pending {
        ui.horizontal(|ui| {
            let dir = if *up { "\u{2191}" } else { "\u{2193}" };
            ui.colored_label(crate::theme_ui::ACCENT, format!("{dir} {} {}", tr(lang, "sftp.syncpreview"), files.len()));
            if ui.small_button(tr(lang, "sftp.syncapply")).clicked() { a.sync_apply = true; }
            if ui.small_button(tr(lang, "qc.cancel")).clicked() { a.sync_cancel = true; }
        });
        ui.weak(files.join(", ").chars().take(200).collect::<String>()); // 목록 미리보기(축약).
    }
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("/").clicked() {
            a.go = Some("/".to_string());
        }
        for (label, full) in crate::sftppath::crumbs(&sftp.path) {
            if ui.small_button(label).clicked() {
                a.go = Some(full);
            }
        }
        let (nd, nf, sz) =
            crate::browserfs::summarize(sftp.entries.iter().map(|e| (e.is_dir, e.size)));
        ui.label(format!("{nd}\u{1f4c1} {nf}\u{1f4c4} {}", crate::browserfs::human(sz)));
        // 다중 선택 합계(개수·크기) — FileZilla식 상태.
        if !sftp.multi.is_empty() {
            let ssz: u64 = sftp.entries.iter().filter(|e| sftp.multi.contains(&e.name)).map(|e| e.size).sum();
            ui.label(format!("\u{00b7} {}\u{2713} {}", sftp.multi.len(), crate::browserfs::human(ssz)));
        }
        // 보기 모드 선택(탐색기식: 자세히/목록/큰·작은 아이콘/타일).
        egui::ComboBox::from_id_salt("sftp_view")
            .selected_text(tr(lang, sftp.view_mode.key()))
            .show_ui(ui, |ui| {
                for m in crate::sftpview::ViewMode::all() {
                    ui.selectable_value(&mut sftp.view_mode, m, tr(lang, m.key()));
                }
            });
    });
    use crate::sftpentries::RowAction;
    let details = matches!(sftp.view_mode, crate::sftpview::ViewMode::Details);
    match crate::sftpentries::input_rows(ui, sftp, lang, details) {
        RowAction::Rename => a.do_rename = true,
        RowAction::Mkdir => a.do_mkdir = true,
        RowAction::DelOk => a.do_del = true,
        RowAction::DelCancel => a.cancel_del = true,
        RowAction::None => {}
    }
    a.queue = show_queue(ui, &sftp.transfers, lang);
    ui.add(
        egui::TextEdit::singleline(&mut sftp.filter)
            .hint_text(tr(lang, "browser.filter"))
            .desired_width(f32::INFINITY),
    );
    if sftp.batch_mode {
        // 적용 전 미리보기: 바뀔 항목 수.
        let n = sftp
            .entries
            .iter()
            .filter(|e| {
                crate::sftpentries::batch_new_name(&e.name, &sftp.batch_find, &sftp.batch_replace, 1)
                    .is_some()
            })
            .count();
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut sftp.batch_find);
            ui.label("\u{2192}");
            ui.add(egui::TextEdit::singleline(&mut sftp.batch_replace).hint_text("{n} {nn} {name} {ext}"))
                .on_hover_text(tr(lang, "sftp.batchtokens"));
            if ui.add_enabled(n > 0, egui::Button::new(tr(lang, "sftp.batchapply"))).clicked() {
                a.batch_apply = true;
            }
            ui.weak(format!("({n})"));
        });
    }
    ui.separator();
    if !sftp.search_results.is_empty() {
        // 검색 결과 목록(클릭 시 해당 파일의 상위 폴더로 이동).
        ui.label(format!("\u{1f50d} {}", sftp.search_results.len()));
        egui::ScrollArea::vertical().id_salt("sftp_search").show(ui, |ui| {
            for r in &sftp.search_results {
                if ui.button(r).clicked() {
                    a.go = Some(crate::sftppath::parent_dir(r));
                }
            }
        });
        return a;
    }
    crate::sftplist::list_zone(ui, sftp, lang, details, sort, &mut a);
    a.rect = Some(ui.min_rect().union(ui.max_rect())); // OS 드롭 위치 판정용.
    a
}

