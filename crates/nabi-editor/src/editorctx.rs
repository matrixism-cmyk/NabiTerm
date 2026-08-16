//! nabiPad 편집창 우클릭 컨텍스트 메뉴 — 일반 에디터 표준(잘라내기/복사/붙여넣기/삭제/전체 선택).
//! 빈 공간·선택 블럭 위 어디서 눌러도 동작. egui TextEdit 커서/상태를 직접 조작한다.
//! 실행취소/다시실행은 egui 기본 단축키(Ctrl+Z / Ctrl+Y)로 동작한다.

use crate::editor::EditorDoc;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::{TextEditOutput, TextEditState};
use nabi_i18n::{tr, Lang};

/// char 인덱스 → 바이트 오프셋(범위 끝은 text 길이로 클램프).
fn byte_at(text: &str, ch: usize) -> usize {
    text.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(text.len())
}

/// TextEditState의 커서를 char 범위로 지정해 저장한다(선택/이동 반영).
fn set_range(ctx: &egui::Context, id: egui::Id, a: usize, b: usize) {
    if let Some(mut st) = TextEditState::load(ctx, id) {
        st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
        st.store(ctx, id);
    }
}

/// 편집창 응답에 우클릭 메뉴를 붙인다. 선택 유무에 따라 항목 활성화.
pub fn editor_context_menu(out: &TextEditOutput, doc: &mut EditorDoc, lang: Lang, readonly: bool, act: &mut crate::editor::EditorAct) {
    let id = out.response.id;
    let range = out.cursor_range.map(|cr| {
        let (a, b) = (cr.primary.index.0, cr.secondary.index.0);
        (a.min(b), a.max(b))
    });
    let has_sel = range.is_some_and(|(a, b)| b > a);
    out.response.clone().context_menu(|ui| {
        let ctx = ui.ctx().clone();
        let sel_text = |doc: &EditorDoc| range.map(|(a, b)| doc.text[byte_at(&doc.text, a)..byte_at(&doc.text, b)].to_string());
        if ui.add_enabled(has_sel && !readonly, egui::Button::new(tr(lang, "ctx.cut"))).clicked() {
            if let (Some((a, b)), Some(t)) = (range, sel_text(doc)) {
                ctx.copy_text(t);
                doc.text.replace_range(byte_at(&doc.text, a)..byte_at(&doc.text, b), "");
                doc.dirty = true;
                set_range(&ctx, id, a, a);
            }
            ui.close();
        }
        if ui.add_enabled(has_sel, egui::Button::new(tr(lang, "ctx.copy"))).clicked() {
            if let Some(t) = sel_text(doc) {
                ctx.copy_text(t);
            }
            ui.close();
        }
        // AI 복사(바이브코딩): 선택/파일 전체를 마크다운 코드블록으로, 또는 현재 위치를 경로:줄로 — 한 서브메뉴로 정리.
        ui.menu_button(tr(lang, "ctx.aicopy"), |ui| {
            let hdr = if doc.path.as_os_str().is_empty() { String::new() } else { format!("`{}`\n", doc.path.display()) }; // 출처 경로 헤더(AI 식별).
            let hint = doc.lang_ext(); // 코드펜스 언어 힌트 — 구문 언어 모드(syntax_ext) 우선.
            if ui.add_enabled(has_sel, egui::Button::new(tr(lang, "ctx.copymd"))).clicked() {
                if let Some(t) = sel_text(doc) { ctx.copy_text(format!("{hdr}```{hint}\n{}\n```", t.trim_end())); }
                ui.close();
            }
            if ui.add_enabled(!doc.text.is_empty(), egui::Button::new(tr(lang, "ctx.copyfilemd"))).clicked() { ctx.copy_text(format!("{hdr}```{hint}\n{}\n```", doc.text.trim_end())); ui.close(); }
            let has_path = !doc.path.as_os_str().is_empty();
            if ui.add_enabled(has_path, egui::Button::new(tr(lang, "ctx.copyloc"))).clicked() {
                // 파일:줄[-줄] — 선택이 있으면 그 줄 범위까지 붙는다.
                let spec = crate::editorloc::loc_linespec(&doc.text, range, doc.cur_line);
                ctx.copy_text(format!("{}:{}", doc.path.display(), spec));
                ui.close();
            }
        });
        // 선택을 터미널에서 실행(에디터에 명령 작성→첫 터미널 pane에 전송+Enter, AI 바이브코딩).
        if ui.add_enabled(has_sel, egui::Button::new(tr(lang, "ctx.runterm"))).clicked() {
            if let Some(t) = sel_text(doc) {
                act.run_in_term = Some(t);
            }
            ui.close();
        }
        // 선택 영역을 새 파일로 저장(EmEditor식 발췌).
        if ui.add_enabled(has_sel, egui::Button::new(tr(lang, "ctx.savesel"))).clicked() {
            if let Some(t) = sel_text(doc) {
                if let Some(p) = rfd::FileDialog::new().save_file() {
                    let _ = std::fs::write(p, t);
                }
            }
            ui.close();
        }
        // 선택 변환은 "변환" 서브메뉴 → 카테고리별 하위 그룹(editorxform)으로 정리. 선택 함수만 받아 일괄 적용.
        let mut chosen: Option<fn(&str) -> String> = None;
        ui.add_enabled_ui(has_sel && !readonly, |ui| {
            ui.menu_button(tr(lang, "ctx.transform"), |ui| {
                chosen = crate::editorxform::transform_menu(ui, lang);
                ui.separator();
                // 주석 토글: 구문 언어 모드(syntax_ext) 우선. html/xml/css류는 블록 주석, 그 외는 줄 주석.
                if ui.button(tr(lang, "ctx.comment")).clicked() {
                    if let Some((a, b)) = range {
                        let (ba, bb) = (byte_at(&doc.text, a), byte_at(&doc.text, b));
                        let ext = doc.lang_ext();
                        let conv = match crate::editorcomment::block_comment_markers(&ext) {
                            Some((o, c)) => crate::editorcomment::toggle_block_comment(&doc.text[ba..bb], o, c),
                            None => crate::editorcomment::toggle_line_comment(&doc.text[ba..bb], crate::editorcomment::line_comment_marker(&ext)),
                        };
                        let end = a + conv.chars().count();
                        doc.text.replace_range(ba..bb, &conv);
                        doc.dirty = true;
                        set_range(&ctx, id, a, end);
                    }
                    ui.close();
                }
                // 선택을 괄호/따옴표 쌍으로 감싸기(코드·마크다운 편집). 라벨이 기호라 i18n 불필요.
                ui.menu_button(tr(lang, "ctx.surround"), |ui| {
                    for (open, close) in [("(", ")"), ("[", "]"), ("{", "}"), ("<", ">"), ("\"", "\""), ("'", "'"), ("`", "`")] {
                        if ui.button(format!("{open}{close}")).clicked() {
                            if let Some((a, b)) = range {
                                let (ba, bb) = (byte_at(&doc.text, a), byte_at(&doc.text, b));
                                let conv = format!("{open}{}{close}", &doc.text[ba..bb]);
                                let end = a + conv.chars().count();
                                doc.text.replace_range(ba..bb, &conv);
                                doc.dirty = true;
                                set_range(&ctx, id, a, end);
                            }
                            ui.close();
                        }
                    }
                });
            });
        });
        // 그룹 메뉴에서 고른 변환을 선택 영역에 적용(렌더 경로와 분리해 차용 충돌 회피).
        if let (Some(f), Some((a, b))) = (chosen, range) {
            let (ba, bb) = (byte_at(&doc.text, a), byte_at(&doc.text, b));
            let conv = f(&doc.text[ba..bb]);
            let end = a + conv.chars().count();
            doc.text.replace_range(ba..bb, &conv);
            doc.dirty = true;
            set_range(&ctx, id, a, end);
        }
        // 삽입(개발자 도구): UUID·현재 날짜시각·임의 비밀번호를 커서 위치(선택 시 대체)에 넣는다.
        ui.add_enabled_ui(!readonly, |ui| {
            ui.menu_button(tr(lang, "ctx.insert"), |ui| {
                let mut ins: Option<String> = None;
                if ui.button(tr(lang, "ctx.uuid")).clicked() { ins = Some(crate::editoruuid::gen_uuid_v4()); ui.close(); }
                if ui.button(tr(lang, "ctx.datetime")).clicked() { ins = Some(crate::editoruuid::now_datetime()); ui.close(); }
                if ui.button(tr(lang, "ctx.unixts")).clicked() { ins = Some(crate::editoruuid::now_unix()); ui.close(); }
                if ui.button(tr(lang, "ctx.password")).clicked() { ins = Some(crate::editoruuid::gen_password(16)); ui.close(); }
                if ui.button(tr(lang, "ctx.lorem")).clicked() { ins = Some(crate::editoruuid::lorem()); ui.close(); }
                if let (Some(s), Some((a, b))) = (ins, range) {
                    let (ba, bb) = (byte_at(&doc.text, a), byte_at(&doc.text, b));
                    let end = a + s.chars().count();
                    doc.text.replace_range(ba..bb, &s);
                    doc.dirty = true;
                    set_range(&ctx, id, end, end);
                }
            });
        });
        // 코드(LSP — rs 문서만): 정의로 이동/심볼 정보/참조 찾기(+진단 목록). 앱 허브가 처리.
        if doc.lang_ext() == "rs" {
            ui.menu_button(tr(lang, "ctx.code"), |ui| {
                if ui.button(tr(lang, "lsp.gotodef.short")).clicked() { act.lsp_goto_def = true; ui.close(); }
                if ui.button(tr(lang, "lsp.hover")).clicked() { act.lsp_hover = true; ui.close(); }
                if ui.button(tr(lang, "lsp.refs")).clicked() { act.lsp_refs = true; ui.close(); }
                if ui.button(tr(lang, "lsp.rename")).clicked() { doc.rename_open = true; ui.close(); }
                if ui.button(tr(lang, "lsp.format")).clicked() { act.lsp_format = true; ui.close(); }
                if ui.add_enabled(!doc.diags.is_empty(), egui::Button::new(tr(lang, "lsp.diags"))).clicked() { doc.diag_popup = true; ui.close(); }
            });
        }
        // 괄호 짝으로 커서 이동(VS Code). 커서 위치 괄호의 짝 char 인덱스로 점프.
        if ui.button(tr(lang, "ctx.gotobracket")).clicked() {
            if let Some(cidx) = out.cursor_range.map(|cr| cr.primary.index.0) {
                if let Some((_, j)) = crate::editorextra::match_bracket(&doc.text, cidx) {
                    set_range(&ctx, id, j, j);
                }
            }
            ui.close();
        }
        // 선택 복제: 선택 바로 뒤에 사본을 삽입하고 사본을 선택(VS Code Ctrl+D 상당).
        if ui.add_enabled(has_sel && !readonly, egui::Button::new(tr(lang, "ctx.dupsel"))).clicked() {
            if let (Some((_, b)), Some(t)) = (range, sel_text(doc)) {
                let bb = byte_at(&doc.text, b);
                doc.text.insert_str(bb, &t);
                doc.dirty = true;
                set_range(&ctx, id, b, b + t.chars().count());
            }
            ui.close();
        }
        if ui.add_enabled(!readonly, egui::Button::new(tr(lang, "ctx.paste"))).clicked() {
            if let Some(t) = crate::uiutil::clipboard_text() {
                let (a, b) = range.unwrap_or((0, 0));
                doc.text.replace_range(byte_at(&doc.text, a)..byte_at(&doc.text, b), &t);
                doc.dirty = true;
                let end = a + t.chars().count();
                set_range(&ctx, id, end, end);
            }
            ui.close();
        }
        if ui.add_enabled(has_sel && !readonly, egui::Button::new(tr(lang, "ctx.delete"))).clicked() {
            if let Some((a, b)) = range {
                doc.text.replace_range(byte_at(&doc.text, a)..byte_at(&doc.text, b), "");
                doc.dirty = true;
                set_range(&ctx, id, a, a);
            }
            ui.close();
        }
        ui.separator();
        // 줄 편집(복제/삭제/이동)은 "줄" 서브메뉴로 묶는다(메뉴 정리). 커서가 속한 줄 대상.
        let cur = range.map(|(a, _)| a).unwrap_or(0);
        ui.add_enabled_ui(!readonly, |ui| {
            ui.menu_button(tr(lang, "ctx.lines"), |ui| {
                use crate::editorlineops as l;
                // 미리 계산(불변 차용 후 해제) → 클릭 시 적용(가변).
                let ops: [(&str, (String, usize)); 4] = [
                    ("ctx.dupline", l::dup_line(&doc.text, cur)),
                    ("ctx.delline", l::del_line(&doc.text, cur)),
                    ("ctx.moveup", l::move_line(&doc.text, cur, true)),
                    ("ctx.movedown", l::move_line(&doc.text, cur, false)),
                ];
                for (key, out) in ops {
                    if ui.button(tr(lang, key)).clicked() {
                        doc.text = out.0;
                        doc.dirty = true;
                        set_range(&ctx, id, out.1, out.1);
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        if ui.button(tr(lang, "ctx.selectall")).clicked() {
            set_range(&ctx, id, 0, doc.text.chars().count());
            ui.close();
        }
        if ui.button(tr(lang, "menu.find")).clicked() {
            doc.find.open = true; // 찾기 바 열기(Ctrl+F와 동일).
            ui.close();
        }
        ui.separator();
        // 보기 토글(빠른 전환) — 줄바꿈·공백 표시·구문 강조.
        ui.checkbox(&mut doc.wrap, tr(lang, "editor.wrap"));
        ui.checkbox(&mut doc.show_ws, tr(lang, "editor.showws"));
        ui.checkbox(&mut doc.highlight, tr(lang, "editor.highlight"));
    });
}
