//! SFTP 패널 상단 툴바. render_sftp_tab에서 분리(파일 크기 규율). 액션은 SftpAct에 모은다.
//!
//! 아이콘이 열 몇 개가 한 줄에 늘어서면 무엇이 무엇인지 알 수 없다. 자주 쓰는 것만 툴바에
//! 남기고(이동·새로고침·주소·새로 만들기·붙여넣기) 나머지는 성격별로 묶었다:
//! **비교·동기화**, **보기**(정렬·숨김), **도구**(검색·일괄이름·폴더받기).
//! 그룹 사이에는 구분선을 둬 눈으로 끊어 읽히게 한다.

use crate::sftppanel::SftpPanel;
use crate::sftptab::SftpAct;
use nabi_i18n::{tr, Lang};

/// 아이콘 버튼 + 툴팁(설명 없는 글리프를 남기지 않는다).
fn icon(ui: &mut egui::Ui, glyph: &str, lang: Lang, key: &str) -> bool {
    ui.small_button(glyph).on_hover_text(tr(lang, key)).clicked()
}

/// SFTP 툴바를 그리고 클릭 액션을 a에 채운다.
pub(crate) fn render_toolbar(
    ui: &mut egui::Ui,
    sftp: &mut SftpPanel,
    lang: Lang,
    bookmarks: &[String],
    sort_desc: bool,
    a: &mut SftpAct,
) {
    ui.horizontal_wrapped(|ui| {
        let t = if sftp.host.is_empty() { tr(lang, "sftp.title").to_string() } else { sftp.host.clone() };
        ui.label(format!("\u{1f5a7} {t}"));
        // 남은 자리 — 올리기 전에 알아야 뜻이 있다. 모르는 서버(statvfs 미지원)에서는
        // 아무것도 적지 않는다(0으로 적으면 "가득 찼다"는 거짓말이 된다).
        if let Some(l) = crate::freespace::label(sftp.free_space) {
            ui.weak(l).on_hover_text(tr(lang, "sftp.freespace"));
        }
        // ── 이동 ──
        if icon(ui, "\u{1f3e0}", lang, "sftp.home") {
            a.go = Some(".".to_string());
        }
        if icon(ui, "\u{2191}", lang, "browser.up") {
            a.go = Some(crate::sftppath::parent_dir(&sftp.path));
        }
        if icon(ui, "\u{27f3}", lang, "sftp.refresh") {
            a.go = Some(sftp.path.clone());
        }
        // 편집 가능한 경로 주소창(FileZilla식 — 직접 입력/붙여넣기로 이동).
        let r = ui.add(
            egui::TextEdit::singleline(&mut sftp.addr)
                .desired_width(200.0)
                .hint_text(tr(lang, "sftp.gotopath")),
        );
        // ⚠️ Enter를 누른 프레임에는 TextEdit가 포커스를 넘겨준다 — 그래서 `has_focus()`로 먼저
        // 갈라 버리면 되돌리기 갈래로 빠져 입력이 지워지고 이동 갈래는 **도달조차 못 한다**.
        // 확정 여부를 먼저 읽고, 그 다음에만 현재 경로로 되돌린다.
        let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if entered && !sftp.addr.trim().is_empty() {
            a.go = Some(sftp.addr.trim().to_string());
        } else if !r.has_focus() {
            sftp.addr = sftp.path.clone();
        }
        bookmark_menu(ui, lang, bookmarks, a);
        ui.separator();
        // ── 만들기·붙여넣기 ──
        if icon(ui, "\u{1f4c1}+", lang, "sftp.newfolder") {
            a.new_folder = true;
        }
        if icon(ui, "\u{1f4c4}+", lang, "sftp.newfile") {
            a.new_file = true;
        }
        if icon(ui, "\u{1f4cb}\u{2193}", lang, "browser.paste") {
            a.paste = true;
        }
        ui.separator();
        sync_menu(ui, lang, a);
        find_button(ui, lang, a);
        view_menu(ui, sftp, lang, sort_desc, a);
        tools_menu(ui, lang, a);
        ui.separator();
        // ── 연결 ──
        if icon(ui, "\u{1f50c}", lang, "sftp.reconnect") {
            a.reconnect = true;
        }
        if icon(ui, "\u{2715}", lang, "sftp.close") {
            a.close = true;
        }
    });
}

/// 즐겨찾기(FileZilla식): 현재 경로 추가 + 목록에서 이동/삭제.
fn bookmark_menu(ui: &mut egui::Ui, lang: Lang, bookmarks: &[String], a: &mut SftpAct) {
    ui.menu_button("\u{2b50}", |ui| {
        if ui.button(tr(lang, "sftp.addbookmark")).clicked() {
            a.bookmark_add = true;
            ui.close();
        }
        for b in bookmarks {
            ui.horizontal(|ui| {
                if ui.button(b).clicked() {
                    a.bookmark_go = Some(b.clone());
                    ui.close();
                }
                if ui.small_button("\u{2715}").clicked() {
                    a.bookmark_del = Some(b.clone());
                    ui.close();
                }
            });
        }
    })
    .response
    .on_hover_text(tr(lang, "sftp.bookmarks"));
}

/// 서버에서 파일 찾기 — 동기화 묶음에 넣지 않는다. 견주는 일이 아니라 찾는 일이다.
fn find_button(ui: &mut egui::Ui, lang: Lang, a: &mut SftpAct) {
    if ui
        .button(format!("\u{1f50d} {}", tr(lang, "sftp.find.title")))
        .on_hover_text(tr(lang, "sftp.find.hint"))
        .clicked()
    {
        a.open_find = true;
    }
}

/// 로컬↔원격을 견주는 기능 묶음(비교 색칠·동기 이동·차이만 전송).
fn sync_menu(ui: &mut egui::Ui, lang: Lang, a: &mut SftpAct) {
    ui.menu_button(format!("\u{21c5} {}", tr(lang, "sftp.syncgroup")), |ui| {
        if ui.button(format!("\u{21c6} {}", tr(lang, "sftp.compare"))).clicked() {
            a.toggle_compare = true;
            ui.close();
        }
        if ui.button(format!("\u{1f517} {}", tr(lang, "sftp.sync"))).clicked() {
            a.toggle_sync = true;
            ui.close();
        }
        ui.separator();
        if ui.button(format!("\u{2191}\u{21c5} {}", tr(lang, "sftp.syncup"))).clicked() {
            a.sync_up = true;
            ui.close();
        }
        if ui.button(format!("\u{2193}\u{21c5} {}", tr(lang, "sftp.syncdown"))).clicked() {
            a.sync_down = true;
            ui.close();
        }
    });
}

/// 목록 표시 방식(정렬·숨김). 보기 모드 드롭다운은 로컬 브라우저와 같은 자리에 둔다.
fn view_menu(ui: &mut egui::Ui, sftp: &mut SftpPanel, lang: Lang, sort_desc: bool, a: &mut SftpAct) {
    ui.menu_button(format!("\u{1f441} {}", tr(lang, "menu.view")), |ui| {
        let arrow = if sort_desc { "\u{25bc}" } else { "\u{25b2}" };
        if ui.button(format!("{arrow} {}", tr(lang, "sftp.sort"))).clicked() {
            a.cycle_sort = true;
            ui.close();
        }
        let hidden = crate::viewmenu::check(sftp.show_hidden, tr(lang, "sftp.hidden"));
        if ui.selectable_label(sftp.show_hidden, hidden).clicked() {
            sftp.show_hidden = !sftp.show_hidden;
            ui.close();
        }
    });
}

/// 검색·일괄 이름변경·폴더 통째 받기.
fn tools_menu(ui: &mut egui::Ui, lang: Lang, a: &mut SftpAct) {
    ui.menu_button(format!("\u{1f527} {}", tr(lang, "sftp.tools")), |ui| {
        if ui.button(format!("\u{1f50d} {}", tr(lang, "sftp.search"))).clicked() {
            a.search = true;
            ui.close();
        }
        if ui.button(format!("\u{1f524} {}", tr(lang, "sftp.batchrename"))).clicked() {
            a.batch_toggle = true;
            ui.close();
        }
        if ui.button(format!("\u{2b07} {}", tr(lang, "sftp.downloaddir"))).clicked() {
            a.dl_cur = true;
            ui.close();
        }
    });
}
