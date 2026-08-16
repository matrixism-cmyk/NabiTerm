//! 파일 브라우저 본문 렌더 — 사이드패널/도크 탭 공용(액션 수집, 적용은 browserapply.rs).

use crate::browserfs::{human, read_entries, Sort};
use crate::browserpanel::BrowserPanel;
use crate::sftpview::RemoteName;
use crate::viewmode::ViewMode;
use nabi_i18n::Lang;
use std::collections::HashMap;
use std::path::PathBuf;

/// 사용자 홈 디렉터리(파일 브라우저 시작 위치).
pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}

/// 브라우저 렌더 중 수집된 사용자 액션(적용은 NabiApp::apply_browser_act).
#[derive(Default)]
pub(crate) struct BrowserAct {
    pub nav: Option<PathBuf>,
    pub cd_here: bool,
    pub cycle_sort: bool,
    pub set_sort: Option<Sort>,
    /// 단일 클릭 선택: (이름, ctrl, shift) — 토글/범위 처리는 적용측.
    pub select: Option<(String, bool, bool)>,
    pub upload: Option<String>,
    pub dl_into: Option<(String, RemoteName)>,
    /// 빈 영역에 원격 항목 드롭(현재 폴더로 다운로드).
    pub drop_remote: Option<RemoteName>,
    pub calc_size: Option<String>,
    pub duplicate: Option<String>,
    pub new_folder: bool,
    pub new_file: bool,
    pub term_here: bool,
    pub toggle_hidden: bool,
    pub mkdir_ok: bool,
    pub mkdir_cancel: bool,
    pub rename_start: Option<String>,
    pub rename_ok: bool,
    pub rename_cancel: bool,
    pub delete: Option<String>,
    pub over: bool,
    pub view: ViewMode,
    /// 클립보드 항목을 현재 폴더로 붙여넣기(CF_HDROP).
    pub paste: bool,
    /// 클립보드로 복사할 항목.
    pub copy: Option<String>,
    /// OS 드래그-아웃 시작 항목.
    pub os_drag: Option<String>,
    /// 편집을 요청한 파일(내장/외부 에디터).
    pub edit: Option<String>,
    /// HEX로 강제로 열 파일.
    pub edit_hex: Option<String>,
    /// 빠른 미리보기 요청 파일(E9).
    pub preview: Option<String>,
    /// 파일 내용 검색 패턴(Find in Files).
    pub content_search: Option<String>,
    /// 빈 공간 메뉴 ▸ 전체 선택 / 선택 반전 / 폴더 트리 / 확장자 통계 / 선택 경로 복사.
    pub select_all: bool, pub invert_sel: bool, pub dir_tree: bool, pub dir_stats: bool, pub copy_paths: bool,
    /// 이 브라우저 패널의 화면 rect(OS 파일 드롭 위치 판정용).
    pub rect: Option<egui::Rect>,
}

/// 브라우저 본문을 ui에 그리고 액션을 모은다(사이드패널·탭 양쪽에서 호출).
/// `scope`: 인스턴스 구분자(탭=PaneId, 사이드패널=0) — 스크롤 등 위젯 상태 분리.
pub(crate) fn render_browser_tab(
    ui: &mut egui::Ui,
    b: &mut BrowserPanel,
    remote_map: &HashMap<String, (bool, u64)>,
    can_upload: bool,
    lang: Lang,
    scope: u64,
) -> BrowserAct {
    ui.push_id(("browser_scope", scope), |ui| {
        render_inner(ui, b, remote_map, can_upload, lang)
    })
    .inner
}

fn render_inner(
    ui: &mut egui::Ui,
    b: &mut BrowserPanel,
    remote_map: &HashMap<String, (bool, u64)>,
    can_upload: bool,
    lang: Lang,
) -> BrowserAct {
    let mut a = BrowserAct {
        // 시작 시점 min_rect는 비어 있어 ui_contains_pointer가 항상 false —
        // 할당 영역(max_rect) 기준으로 판정해야 엄지 버튼/OS 드롭이 동작한다.
        over: ui.rect_contains_pointer(ui.max_rect()),
        view: b.view,
        ..Default::default()
    };
    // Ctrl+휠로 이 파일 브라우저만 확대/축소(글꼴 크기 — 전역 영향 없음).
    let dz = crate::renameui::ctrl_wheel_zoom(ui, a.over);
    if dz != 0.0 {
        b.font_size = (b.font_size + dz).clamp(9.0, 28.0);
    }
    let path = b.path.clone();
    // 빈 경로 = "내 컴퓨터"(드라이브 목록). 그 외엔 일반 디렉터리.
    let is_drives = path.as_os_str().is_empty();
    let entries = if is_drives {
        Vec::new()
    } else {
        read_entries(&path, b.sort, b.sort_desc, b.show_hidden)
    };
    ui.horizontal_wrapped(|ui| {
        ui.label("\u{1f4c1}");
        let full = path.to_string_lossy().into_owned();
        for (label, target) in crate::browserfs::local_crumbs(&path) {
            if ui.small_button(label).clicked() {
                a.nav = Some(target);
            }
        }
        let (nd, nf, sz) = crate::browserfs::summarize(entries.iter().map(|r| (r.is_dir, r.size)));
        if !is_drives {
            ui.label(format!("{nd}\u{1f4c1} {nf}\u{1f4c4} {}", human(sz)));
            // 다중 선택 합계(개수·크기) — SFTP와 일관(FileZilla식).
            if !b.multi.is_empty() {
                let ssz: u64 = entries.iter().filter(|r| b.multi.contains(&r.name)).map(|r| r.size).sum();
                ui.label(format!("\u{00b7} {}\u{2713} {}", b.multi.len(), human(ssz)));
            }
            if let Some(u) = crate::drives::drive_usage_label(&path, lang) {
                ui.separator();
                ui.label(u); // 현재 드라이브 사용/여유 용량.
            }
        }
        if ui
            .small_button("\u{1f4cb}")
            .on_hover_text(nabi_i18n::tr(lang, "browser.copypath"))
            .clicked()
        {
            ui.ctx().copy_text(full);
        }
    });
    ui.horizontal(|ui| {
        if ui.button("\u{1f3e0}").on_hover_text(nabi_i18n::tr(lang, "sftp.home")).clicked() {
            a.nav = Some(home_dir());
        }
        // 내 컴퓨터(드라이브 목록) — 빈 경로로 이동.
        if ui.button("\u{1f5a5}").on_hover_text(nabi_i18n::tr(lang, "browser.mycomputer")).clicked() {
            a.nav = Some(std::path::PathBuf::new());
        }
        // 바로가기: 바탕화면/문서/다운로드/네트워크(특수 폴더로 즉시 이동).
        ui.menu_button("\u{2b50}", |ui| {
            let home = home_dir();
            for (key, sub) in [("browser.desktop", "Desktop"), ("browser.documents", "Documents"), ("browser.downloads", "Downloads")] {
                if ui.button(nabi_i18n::tr(lang, key)).clicked() {
                    a.nav = Some(home.join(sub));
                    ui.close();
                }
            }
            if ui.button(nabi_i18n::tr(lang, "browser.network")).clicked() {
                // 네트워크는 인앱 SMB 열거가 없어 OS 네트워크 폴더로 연다.
                let _ = std::process::Command::new("explorer").arg("shell:NetworkPlacesFolder").spawn();
                ui.close();
            }
        })
        .response
        .on_hover_text(nabi_i18n::tr(lang, "browser.places"));
        // 아이콘 + 번역된 툴팁 — 옆 버튼들·SFTP 툴바와 같은 방식(한국어를 라벨에 박아 두면
        // 일본어·영어 사용자에게 그대로 보인다).
        if ui.button("\u{2b06}").on_hover_text(nabi_i18n::tr(lang, "browser.up")).clicked() {
            // 드라이브 루트에서 위로 가면 부모가 없으니 "내 컴퓨터"로.
            a.nav = Some(path.parent().map(|p| p.to_path_buf()).unwrap_or_default());
        }
        // 편집 가능한 경로 주소창(SFTP와 일관 — 입력/붙여넣기 후 Enter로 이동). 비편집 시 현재 경로 표시.
        let r = ui.add(egui::TextEdit::singleline(&mut b.addr).desired_width(180.0).hint_text(nabi_i18n::tr(lang, "sftp.gotopath")));
        // Enter를 누른 프레임엔 포커스를 잃으므로 확정 여부를 **먼저** 본다(sftptoolbar와 같은 이유).
        let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if entered && !b.addr.trim().is_empty() {
            a.nav = Some(std::path::PathBuf::from(b.addr.trim()));
        } else if !r.has_focus() {
            b.addr = path.to_string_lossy().into_owned();
        }
        // 터미널에서 열기(스마트): 활성 탭이 셸이면 그 터미널을 이 폴더로 cd, 아니면 새 터미널을 연다.
        if ui.button(format!("\u{1f4bb} {}", nabi_i18n::tr(lang, "browser.cdhere"))).clicked() {
            a.cd_here = true;
        }
        if ui.button("\u{1f4c1}+").on_hover_text(nabi_i18n::tr(lang, "sftp.newfolder")).clicked() { a.new_folder = true; }
        if ui.button("\u{1f4c4}+").on_hover_text(nabi_i18n::tr(lang, "sftp.newfile")).clicked() { a.new_file = true; }
        // 클립보드(탐색기에서 복사한 파일)를 현재 폴더로 붙여넣기.
        if ui.button("\u{1f4cb}\u{2193}").on_hover_text(nabi_i18n::tr(lang, "browser.paste")).clicked() { a.paste = true; }
        // 정렬 키 + 현재 방향(▲오름/▼내림) 표시. 클릭 시 정렬 키 순환.
        let arrow = if b.sort_desc { "\u{25bc}" } else { "\u{25b2}" };
        if ui.button(format!("{arrow} {}", nabi_i18n::tr(lang, b.sort.key()))).on_hover_text(nabi_i18n::tr(lang, "sftp.sort")).clicked() { a.cycle_sort = true; }
        crate::browserinput::view_combo(ui, lang, &mut a.view);
        if ui.button("\u{1f441}").on_hover_text(nabi_i18n::tr(lang, "sftp.hidden")).clicked() { a.toggle_hidden = true; }
    });
    // 내 컴퓨터: 드라이브 목록(용량 막대)으로 대체하고 일반 파일 목록은 건너뛴다.
    if is_drives {
        if let Some(d) = crate::drives::drives_view(ui, lang) {
            a.nav = Some(d);
        }
        let panel = ui.min_rect().union(ui.max_rect());
        a.over = a.over || ui.rect_contains_pointer(panel);
        a.rect = Some(panel);
        return a;
    }
    let new_is_file = b.new_is_file;
    let details = a.view == crate::sftpview::ViewMode::Details;
    // 자세히 보기는 셀에서 인라인 편집 — 상단 이름변경 행은 숨긴다(그 외 보기는 유지).
    let (mk_ok, mk_cancel, rn_ok, rn_cancel) =
        crate::browserinput::input_rows(ui, &mut b.mkdir, &mut b.rename, new_is_file, lang, !details);
    a.mkdir_ok = mk_ok;
    a.mkdir_cancel = mk_cancel;
    a.rename_ok = rn_ok;
    a.rename_cancel = rn_cancel;
    ui.horizontal(|ui| {
        // 파일 내용 검색(Find in Files) — 필터 텍스트를 패턴으로 현재 폴더를 재귀 검색.
        if ui.small_button("\u{1f50d}").on_hover_text(nabi_i18n::tr(lang, "browser.contentsearch")).clicked() && !b.filter.trim().is_empty() {
            a.content_search = Some(b.filter.clone());
        }
        ui.add(
            egui::TextEdit::singleline(&mut b.filter)
                .hint_text(nabi_i18n::tr(lang, "browser.filter"))
                .desired_width(f32::INFINITY),
        );
    });
    let filt = b.filter.to_lowercase();
    ui.separator();
    crate::browsercols::apply_list_font(ui, b.font_size); // 이 브라우저 목록만 글꼴 적용.
    // 목록 전체를 드롭 존으로 — 원격 항목을 빈 영역에 놓으면 현재 폴더로 다운로드.
    // 인라인 이름변경 편집기: b.rename 버퍼를 셀 위치에서 직접 편집(자세히·아이콘/타일 보기 모두).
    let rn_target = b.rename.as_ref().map(|(o, _)| o.clone());
    let rn_buf = rn_target.as_ref().and(b.rename.as_mut().map(|(_, n)| n));
    let mut ren = crate::renameui::RenameUi {
        target: rn_target,
        buf: rn_buf,
        focus: &mut b.rename_focus,
        commit: false,
        cancel: false,
    };
    // 빈 공간 우클릭 메뉴: 행보다 "먼저" 배경 클릭영역을 등록한다(나중 등록되는 행이 히트테스트에서
    // 위에 있어 좌/우클릭을 가져가고, 빈 칸 우클릭만 이 배경 메뉴가 받는다 — 행 클릭 회귀 방지).
    let bg = ui.interact(ui.available_rect_before_wrap(), ui.id().with("empty_bg"), egui::Sense::click());
    bg.context_menu(|ui| crate::browsermenu::empty_space_menu(ui, &mut a, lang, &path));
    let (_, payload) = ui.dnd_drop_zone::<RemoteName, _>(egui::Frame::NONE, |ui| {
        let acts = crate::browserrows::browser_rows(
            ui, &entries, &path, &filt, remote_map, can_upload, lang, b.sort, b.sort_desc,
            a.view, b.selected.as_deref(), &b.multi, b.scroll, &mut ren,
        );
        if acts.nav.is_some() {
            a.nav = acts.nav; // 더블클릭 진입(툴바 nav를 덮지 않음).
        }
        a.delete = acts.delete;
        a.rename_start = acts.rename;
        a.upload = acts.upload;
        a.dl_into = acts.dl_into;
        a.calc_size = acts.calc_size;
        a.duplicate = acts.duplicate;
        a.set_sort = acts.set_sort;
        a.select = acts.select;
        a.copy = acts.copy;
        a.os_drag = acts.os_drag;
        a.edit = acts.edit;
        a.edit_hex = acts.edit_hex;
        a.preview = acts.preview;
    });
    a.rename_ok |= ren.commit; // 인라인 편집 확정/취소.
    a.rename_cancel |= ren.cancel;
    if let Some(p) = payload {
        a.drop_remote = Some((*p).clone());
    }
    if a.select_all { b.multi = entries.iter().map(|r| r.name.clone()).collect(); } // 빈 공간 메뉴 ▸ 전체 선택.
    if a.invert_sel { let cur = b.multi.clone(); b.multi = entries.iter().map(|r| r.name.clone()).filter(|n| !cur.contains(n)).collect(); } // 선택 반전.
    if a.copy_paths {
        // 선택이 없으면 보이는 항목 전체의 경로를 복사한다.
        let names: Vec<String> = if b.multi.is_empty() {
            entries.iter().map(|r| r.name.clone()).collect()
        } else {
            b.multi.iter().cloned().collect()
        };
        let joined = names.iter().map(|n| path.join(n).to_string_lossy().into_owned());
        ui.ctx().copy_text(joined.collect::<Vec<_>>().join("\n"));
    }
    // 종료 시점에 실제 사용 영역∪할당 영역으로 over 재확정(아래 빈 공간 포함).
    let panel = ui.min_rect().union(ui.max_rect());
    a.over = a.over || ui.rect_contains_pointer(panel);
    a.rect = Some(panel);
    a
}

