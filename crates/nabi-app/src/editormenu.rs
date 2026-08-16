//! nabiPad 메뉴바 — 일반 텍스트 에디터식 메뉴(파일/편집/보기/검색/인코딩/도움말).
//! 기본 숨김(EditorConfig.show_menu_bar=false). 표시 여부 토글은 nabipad.toml에 저장된다.

use crate::app::NabiApp;
use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const EOLS: [&str; 3] = ["LF", "CRLF", "CR"];

/// 메뉴바를 그린다. doc-로컬 토글(강조·줄바꿈·공백·읽기전용·찾기)은 doc에 직접,
/// 앱 처리가 필요한 동작(저장·인코딩·EOL·메뉴 토글)은 act로 모은다.
pub(crate) fn menu_bar(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang, act: &mut EditorAct, recent: &[String]) {
    // 텍스트 서식 항목(강조·줄바꿈·공백·인코딩·EOL·후행공백)은 일반 텍스트 문서에서만,
    // 찾기는 HEX를 제외한 모든 편집기에서, HEX 전환 라벨은 모드에 맞춰 보인다.
    let is_hex = doc.hex.is_some();
    let plain = doc.hex.is_none() && doc.edit.is_none() && doc.big.is_none();
    // 변환 명령은 String 문서든 rope(대용량) 문서든 똑같이 쓸 수 있다 — 순수 fn(&str)->String이라
    // 버퍼 종류를 가릴 이유가 없었는데, 2MB를 넘는 순간 40여 개가 통째로 사라졌었다.
    let can_transform = plain || doc.edit.is_some();
    egui::menu::bar(ui, |ui| {
        ui.menu_button(tr(lang, "nabipad.menu.file"), |ui| {
            if ui.button(tr(lang, "menu.newpad")).clicked() { act.new_doc = true; ui.close(); }
            if ui.button(tr(lang, "nabipad.openfile")).clicked() { act.open_file = true; ui.close(); }
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button(tr(lang, "nabipad.recentfiles"), |ui| {
                    for p in recent {
                        if ui.button(p).clicked() { act.open_recent = Some(p.clone()); ui.close(); }
                    }
                });
            });
            ui.separator();
            if ui.button(tr(lang, "editor.save")).clicked() { act.save = true; ui.close(); }
            if ui.button(tr(lang, "cmd.saveall")).clicked() { act.save_all = true; ui.close(); }
            if ui.button(tr(lang, "editor.saveas")).clicked() { act.save_as = true; ui.close(); }
            if plain && ui.button(tr(lang, "editor.reload")).clicked() { act.reload = true; ui.close(); }
            if plain && ui.button(tr(lang, "editor.diffdisk")).clicked() { act.diff_disk = true; ui.close(); }
            ui.separator();
            let path = doc.path.to_string_lossy().into_owned();
            if ui.button(tr(lang, "nabipad.reveal")).clicked() {
                let _ = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn();
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.copypath")).clicked() { ui.ctx().copy_text(path.clone()); ui.close(); }
            if ui.button(tr(lang, "nabipad.openexternal")).clicked() {
                // OS 기본 프로그램으로 파일 열기(start "" <path>).
                let _ = std::process::Command::new("cmd").args(["/C", "start", "", &path]).spawn();
                ui.close();
            }
            ui.separator();
            if ui.button(tr(lang, "nabipad.close")).clicked() { act.close = true; ui.close(); }
        });
        ui.menu_button(tr(lang, "nabipad.menu.edit"), |ui| {
            if ui.checkbox(&mut doc.readonly, tr(lang, "editor.readonly")).clicked() { ui.close(); }
            // 변환 명령(일반 텍스트 문서 전용)은 전부 주제별 서브메뉴로 묶어 평면 비대화를 막는다.
            if can_transform {
                use crate::editmenugroups as g;
                // 문서가 너무 커서 전체 적용을 막아야 하면, 눌러도 아무 일이 없게 두지 말고
                // 아예 비활성으로 보여 준다("선택하고 다시 하라"는 이유까지 붙여서).
                let blocked = doc.edit.as_ref().is_some_and(|e| {
                    crate::editbufxform::target_range(e.selection(), e.rope.len_chars()).is_none()
                });
                let apply = |ui: &mut egui::Ui, f: Option<fn(&str) -> String>, doc: &mut EditorDoc| {
                    let Some(f) = f else { return };
                    match doc.edit.as_mut() {
                        // rope 문서: 선택이 있으면 그 구간만, 없으면 문서 전체(크기 한도 안에서).
                        Some(e) => {
                            e.apply_transform(f);
                            doc.dirty = e.dirty;
                        }
                        None => {
                            doc.text = f(&doc.text);
                            doc.dirty = true;
                        }
                    }
                    ui.close();
                };
                if blocked {
                    ui.label(tr(lang, "editor.xform.toobig"));
                    ui.disable();
                }
                // 줄/공백 정리 그룹.
                ui.menu_button(tr(lang, "editor.sortmenu"), |ui| { let f = g::sort_menu(ui, lang); apply(ui, f, doc); });
                ui.menu_button(tr(lang, "editor.wsmenu"), |ui| { let f = g::whitespace_menu(ui, lang); apply(ui, f, doc); });
                ui.menu_button(tr(lang, "editor.lineopsmenu"), |ui| { let f = g::lineops_menu(ui, lang); apply(ui, f, doc); });
                ui.menu_button(tr(lang, "editor.indentmenu"), |ui| { let f = g::indent_menu(ui, lang); apply(ui, f, doc); });
                ui.separator();
                // 개발자 변환 도구는 "도구" 부모 아래로 묶어 편집 메뉴 최상위를 단순화한다.
                ui.menu_button(tr(lang, "editor.toolsmenu"), |ui| {
                    ui.menu_button(tr(lang, "editor.convert"), |ui| { let f = crate::editorcodec::convert_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.casemenu"), |ui| { let f = crate::editorcase::case_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.ciphermenu"), |ui| { let f = g::cipher_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.textmenu"), |ui| { let f = crate::editortext::texttools_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.listmenu"), |ui| { let f = crate::editorlist::list_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.devmenu"), |ui| { let f = crate::editordev::dev_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.datamenu"), |ui| { let f = crate::editorcsv::data_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.extractmenu"), |ui| { let f = crate::editorextract::extract_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.hashmenu"), |ui| { let f = crate::editorhash::hash_menu(ui, lang); apply(ui, f, doc); });
                    ui.menu_button(tr(lang, "editor.nummenu"), |ui| { let f = crate::editornum::num_menu(ui, lang); apply(ui, f, doc); });
                });
            }
        });
        ui.menu_button(tr(lang, "nabipad.menu.view"), |ui| {
            if plain {
                ui.checkbox(&mut doc.show_lineno, tr(lang, "nabipad.lineno"));
                ui.checkbox(&mut doc.highlight, tr(lang, "editor.highlight"));
                ui.checkbox(&mut doc.wrap, tr(lang, "editor.wrap"));
                ui.checkbox(&mut doc.show_ws, tr(lang, "editor.showws"));
                ui.checkbox(&mut doc.minimap, tr(lang, "nabipad.minimap"));
                ui.checkbox(&mut doc.outline, tr(lang, "nabipad.outline"));
                // 구문 강조 언어 모드 강제(무제목/오탐 문서용) — 상태바와 공용 picker(DRY).
                ui.menu_button(tr(lang, "nabipad.syntaxlang"), |ui| crate::editorstatus::syntax_lang_picker(ui, doc, lang));
                ui.menu_button(tr(lang, "nabipad.bookmarks"), |ui| {
                    doc.bookmarks.retain(|&b| b < doc.text.lines().count().max(1)); // 편집으로 사라진 줄의 stale 북마크 self-heal.
                    if ui.button(tr(lang, "nabipad.bmtoggle")).clicked() {
                        let l = doc.cur_line;
                        match doc.bookmarks.iter().position(|&b| b == l) {
                            Some(i) => drop(doc.bookmarks.remove(i)),
                            None => {
                                doc.bookmarks.push(l);
                                doc.bookmarks.sort_unstable(); // 다음/이전 점프가 순서대로.
                            }
                        }
                        ui.close();
                    }
                    if ui.button(tr(lang, "nabipad.bmnext")).clicked() {
                        // 아래로 가장 가까운 북마크(없으면 처음으로 감싸기).
                        let next = doc.bookmarks.iter().find(|&&b| b > doc.cur_line);
                        if let Some(n) = next.or_else(|| doc.bookmarks.first()).copied() {
                            doc.jump_to_line(n);
                        }
                        ui.close();
                    }
                    if ui.button(tr(lang, "nabipad.bmprev")).clicked() {
                        // 위로 가장 가까운 북마크(없으면 마지막으로 감싸기).
                        let prev = doc.bookmarks.iter().rev().find(|&&b| b < doc.cur_line);
                        if let Some(n) = prev.or_else(|| doc.bookmarks.last()).copied() {
                            doc.jump_to_line(n);
                        }
                        ui.close();
                    }
                    if ui.button(tr(lang, "nabipad.bmclear")).clicked() { doc.bookmarks.clear(); ui.close(); }
                });
                ui.menu_button(tr(lang, "editor.docinfo"), |ui| {
                    let s = crate::editorstats::document_stats(&doc.text);
                    ui.label(format!("{}: {}", tr(lang, "stat.lines"), s.lines));
                    ui.label(format!("{}: {}", tr(lang, "stat.words"), s.words));
                    ui.label(format!("{}: {}", tr(lang, "stat.chars"), s.chars));
                    ui.label(format!("{}: {}", tr(lang, "stat.charsnows"), s.chars_no_ws));
                    ui.label(format!("{}: {}", tr(lang, "stat.bytes"), s.bytes));
                    ui.separator();
                    ui.label(format!("{}: {}", tr(lang, "stat.paras"), s.paragraphs));
                    ui.label(format!("{}: {}", tr(lang, "stat.maxline"), s.max_line));
                    ui.label(format!("{}: {}", tr(lang, "stat.avgline"), s.avg_line));
                });
                ui.separator();
            }
            // 체크 해제 시 메뉴바 숨김(토글). 표시 상태에선 항상 체크되어 보인다.
            let mut shown = true;
            if ui.checkbox(&mut shown, tr(lang, "nabipad.menu.show")).clicked() {
                act.toggle_menu_bar = true;
                ui.close();
            }
        });
        if !is_hex {
            ui.menu_button(tr(lang, "nabipad.menu.search"), |ui| {
                if ui.button(tr(lang, "find.placeholder")).clicked() { doc.find.open = true; ui.close(); }
            });
        }
        if plain {
            ui.menu_button(tr(lang, "nabipad.menu.encoding"), |ui| {
                ui.menu_button(tr(lang, "nabipad.reopenenc"), |ui| {
                    if let Some(e) = crate::encodings::encoding_menu(ui, &doc.encoding) { act.set_encoding = Some(e); }
                });
                ui.menu_button(tr(lang, "nabipad.saveenc"), |ui| {
                    if let Some(e) = crate::encodings::encoding_menu(ui, &doc.encoding) { act.save_encoding = Some(e); }
                });
                ui.separator();
                ui.label(egui::RichText::new(tr(lang, "editor.eol")).weak().small());
                for eol in EOLS {
                    if ui.selectable_label(doc.eol == eol, eol).clicked() { act.set_eol = Some(eol); ui.close(); }
                }
            });
        }
        // HEX↔텍스트 전환은 일반 텍스트/HEX 문서에서만(대용량 rope/뷰어는 전환 불가).
        if plain || is_hex {
            ui.menu_button(tr(lang, "nabipad.menu.tools"), |ui| {
                let label = if is_hex { "nabipad.textmode" } else { "nabipad.hexmode" };
                if ui.button(tr(lang, label)).clicked() { act.toggle_hex = true; ui.close(); }
            });
        }
        if ui.button(tr(lang, "menu.settings")).clicked() { act.open_settings = true; ui.close(); }
        ui.menu_button(tr(lang, "nabipad.menu.help"), |ui| {
            ui.label(format!("{} v{}", tr(lang, "nabipad.name"), env!("CARGO_PKG_VERSION")));
        });
    });
}

impl NabiApp {
    /// 메뉴바 표시를 토글하고 nabipad.toml에 저장 + 열린 모든 에디터에 즉시 반영.
    pub(crate) fn toggle_editor_menu_bar(&mut self) {
        let v = !self.editor_config.show_menu_bar;
        self.editor_config.show_menu_bar = v;
        for d in self.editors.values_mut() {
            d.show_menu = v;
        }
        let _ = nabi_config::save(&self.editor_config_path, &self.editor_config);
    }
}
