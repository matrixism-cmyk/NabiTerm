//! rope 편집기(대용량 문서) 우클릭 메뉴.
//!
//! 지금까지 우클릭 메뉴는 작은 문서(String 경로, `editorctx.rs`)에만 있었다. 파일이 2MB를
//! 넘어 rope 편집기로 열리는 순간 오른쪽 버튼이 아무 반응도 하지 않았다 — 잘라내기조차
//! 단축키로만 가능했다. 표준 편집 명령과 변환 메뉴를 같은 자리에 붙인다.

use crate::editbuf::EditBuf;
use nabi_i18n::{tr, Lang};

/// 메뉴에서 고른 동작 — 실행은 호출부(빌림 충돌 회피).
#[derive(Default)]
pub struct BufMenuAct {
    /// 클립보드로 보낼 텍스트(복사·잘라내기).
    pub copy: Option<String>,
    /// 클립보드에서 붙여넣기.
    pub paste: bool,
    /// 찾기 막대 열기.
    pub find: bool,
}

/// 우클릭 메뉴를 그린다. 즉시 적용 가능한 편집은 여기서 처리하고, 나머지는 act로 돌려준다.
pub fn context_menu(
    ui: &mut egui::Ui,
    eb: &mut EditBuf,
    lang: Lang,
    readonly: bool,
) -> BufMenuAct {
    let mut act = BufMenuAct::default();
    let has_sel = eb.selection().is_some();
    // 선택이 없을 때 잘라내기·복사를 누르게 두면 아무 일도 안 일어난다 — 비활성으로 보여 준다.
    ui.add_enabled_ui(has_sel, |ui| {
        if ui.button(tr(lang, "menu.copy")).clicked() {
            act.copy = Some(eb.selected_text());
            ui.close();
        }
        ui.add_enabled_ui(!readonly, |ui| {
            if ui.button(tr(lang, "ctx.cut")).clicked() {
                act.copy = Some(eb.selected_text());
                eb.delete();
                ui.close();
            }
            if ui.button(tr(lang, "ctx.delete")).clicked() {
                eb.delete();
                ui.close();
            }
        });
    });
    ui.add_enabled_ui(!readonly, |ui| {
        if ui.button(tr(lang, "menu.paste")).clicked() {
            act.paste = true;
            ui.close();
        }
    });
    // 다중 커서 — 팔레트·메뉴에도 있어야 단축키를 모르는 사람이 만난다.
    if ui.button(tr(lang, "edit.addnextmatch")).clicked() {
        eb.add_next_match();
        ui.close();
    }
    if ui.button(tr(lang, "edit.selectallmatches")).clicked() {
        eb.select_all_matches();
        ui.close();
    }
    if eb.sel.len() > 1 && ui.button(tr(lang, "edit.clearcursors")).clicked() {
        eb.sel.collapse_to_primary();
        ui.close();
    }
    if ui.button(tr(lang, "edit.addcursorup")).clicked() {
        eb.add_cursor_vertical(-1);
        ui.close();
    }
    if ui.button(tr(lang, "edit.addcursordown")).clicked() {
        eb.add_cursor_vertical(1);
        ui.close();
    }
    if ui.button(tr(lang, "menu.selectall")).clicked() {
        eb.select_all();
        ui.close();
    }
    ui.separator();
    ui.add_enabled_ui(!readonly, |ui| {
        if ui.button(tr(lang, "editor.undo")).clicked() {
            eb.undo();
            ui.close();
        }
        if ui.button(tr(lang, "editor.redo")).clicked() {
            eb.redo();
            ui.close();
        }
    });
    ui.separator();
    if ui.button(tr(lang, "menu.find")).clicked() {
        act.find = true;
        ui.close();
    }
    // 코드 폴딩(T6-3): 커서 위치의 들여쓰기 블록 접기/펼치기 + 전체 펼치기.
    let (cl, _) = eb.cursor_line_col();
    if eb.folds.header_at(cl).is_some() {
        if ui.button(tr(lang, "fold.open")).clicked() {
            eb.folds.unfold_containing(cl);
            ui.close();
        }
    } else {
        let total = eb.rope.len_lines();
        let rng = crate::editbuffold::fold_range_at(cl, total, |i| {
            let s = eb.line_string(i);
            let t = s.trim_end();
            if t.trim_start().is_empty() {
                return None;
            }
            Some(t.chars().take_while(|c| *c == ' ' || *c == '\t').map(|c| if c == '\t' { eb.tab } else { 1 }).sum())
        });
        if let Some((s, e)) = rng {
            if ui.button(tr(lang, "fold.close")).clicked() {
                eb.folds.toggle(s, e);
                ui.close();
            }
        }
    }
    if !eb.folds.is_empty() && ui.button(tr(lang, "fold.openall")).clicked() {
        eb.folds.clear();
        ui.close();
    }
    // 변환(정렬·대소문자·인코딩 등)은 editbufxform 덕에 rope 문서에서도 돈다.
    // 선택이 있으면 그 구간만, 없으면 문서 전체(크기 한도 안에서).
    let blocked = crate::editbufxform::target_range(eb.selection(), eb.rope.len_chars()).is_none();
    ui.add_enabled_ui(!readonly && !blocked, |ui| {
        ui.menu_button(tr(lang, "ctx.transform"), |ui| {
            if let Some(f) = crate::editorxform::transform_menu(ui, lang) {
                eb.apply_transform(f);
                ui.close();
            }
        })
        .response
        .on_disabled_hover_text(tr(lang, "editor.xform.toobig"));
    });
    act
}
