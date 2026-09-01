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
    /// **아무 일도 안 일어났을 때 할 말**(i18n 키).
    ///
    /// 이 메뉴의 명령 여럿은 "할 것이 없으면 아무것도 안 한다"를 `false` 로 알려 주는데,
    /// 부르는 쪽이 그 답을 버리고 있었다(2026-09-01, `#[must_use]` 를 붙여 열 곳을 찾았다).
    /// 누른 사람에게는 그것이 **고장**으로 보인다 — 눌렀는데 아무 일도 없으니까.
    pub note: Option<&'static str>,
}

/// AI 에게 넘길 글의 상한(문자). 이보다 크면 어느 AI 의 문맥에도 안 들어가고,
/// 클립보드에 담는 것만으로도 화면이 멈춘다. 상한을 넘으면 **왜 안 되는지 말해 준다.**
const MAX_AI_COPY: usize = 200_000;

/// 출처 경로 머리글 — AI 가 어느 파일 이야기인지 알게 한다. 경로가 없으면 빈 글.
fn path_header(path: &std::path::Path) -> String {
    match path.as_os_str().is_empty() {
        true => String::new(),
        false => format!("`{}`\n", path.display()),
    }
}

/// 우클릭 메뉴를 그린다. 즉시 적용 가능한 편집은 여기서 처리하고, 나머지는 act로 돌려준다.
///
/// `path`·`hint` 는 AI 복사에 쓴다(출처 경로 머리글과 코드펜스 언어).
#[allow(clippy::too_many_arguments)]
pub fn context_menu(
    ui: &mut egui::Ui,
    eb: &mut EditBuf,
    lang: Lang,
    readonly: bool,
    path: &std::path::Path,
    hint: &str,
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
    // **AI 복사** — 작은 문서(`editorctx`)에만 있던 것을 여기에도 붙인다. 큰 로그야말로
    // AI 에게 넘길 일이 잦은데 정작 그 창에는 길이 없었다(2026-09-01 쌍둥이 비대칭).
    ui.menu_button(tr(lang, "ctx.aicopy"), |ui| {
        // **"파일 전체"는 일부러 없다.** 이 편집기는 파일이 2MB 를 넘어서 열린 창이다 —
        // 그만한 글은 어느 AI 의 문맥에도 안 들어간다. 대신 아래 둘이 그 자리를 대신한다:
        // 필요한 데만 골라 보내거나, **어디를 보라고 알려 준다**(경로:줄).
        let sel_len = eb.selection().map(|(a, b)| b - a).unwrap_or(0);
        let ok = (1..=MAX_AI_COPY).contains(&sel_len);
        if ui.add_enabled(ok, egui::Button::new(tr(lang, "ctx.copymd"))).clicked() {
            let body = eb.selected_text();
            act.copy = Some(format!("{}```{hint}\n{}\n```", path_header(path), body.trim_end()));
            ui.close();
        }
        // 골라 놓은 것이 너무 크면 왜 안 되는지 말해 준다(눌러도 아무 일 없는 것보다 낫다).
        if sel_len > MAX_AI_COPY {
            ui.label(tr(lang, "editor.xform.toobig"));
        }
        let has_path = !path.as_os_str().is_empty();
        if ui.add_enabled(has_path, egui::Button::new(tr(lang, "ctx.copyloc"))).clicked() {
            // 줄 번호는 rope 에 직접 묻는다 — 글로 펼치면 이 편집기를 쓰는 뜻이 사라진다.
            let line = |c: usize| eb.rope.char_to_line(c.min(eb.rope.len_chars())) + 1;
            let spec = match eb.selection() {
                Some((a, b)) if a != b => crate::editorloc::linespec(line(a), line(b)),
                _ => line(eb.cursor()).to_string(),
            };
            act.copy = Some(format!("{}:{spec}", path.display()));
            ui.close();
        }
    });
    // 마크다운 강조 — 선택이 없으면 커서 밑 낱말에 건다(문서 전체로 번지지 않는다).
    ui.add_enabled_ui(!readonly, |ui| {
        ui.menu_button(tr(lang, "editor.mdmenu"), |ui| {
            if let Some(m) = crate::editormd::emphasis_menu(ui, lang) {
                if !eb.toggle_emphasis(m) { act.note = Some("edit.nochange"); }
            }
        });
    });
    // 다중 커서 — 팔레트·메뉴에도 있어야 단축키를 모르는 사람이 만난다.
    if ui.button(tr(lang, "edit.addnextmatch")).clicked() {
        if !eb.add_next_match() { act.note = Some("edit.nomorematch"); }
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
        if !eb.add_cursor_vertical(-1) { act.note = Some("edit.nomoreline"); }
        ui.close();
    }
    if ui.button(tr(lang, "edit.addcursordown")).clicked() {
        if !eb.add_cursor_vertical(1) { act.note = Some("edit.nomoreline"); }
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
        let rng = crate::editbuffold::fold_range_at(cl, total, |i| eb.fold_indent(i));
        if let Some((s, e)) = rng {
            if ui.button(tr(lang, "fold.close")).clicked() {
                eb.folds.toggle(s, e);
                ui.close();
            }
        }
    }
    // 전체 접기 — 처음 보는 파일의 뼈대만 훑을 때 쓴다. 한 자리씩 접는 것으로는
    // 그 목적을 이룰 수 없다(함수 스무 개를 하나씩 접게 된다).
    if ui.button(tr(lang, "fold.closeall")).clicked() {
        let total = eb.rope.len_lines();
        // 깊이를 먼저 다 구해 둔다 — `eb`를 빌려 주면서 동시에 `eb.folds`를 고칠 수 없다.
        // 전체 접기는 어차피 문서를 한 번 훑어야 하므로 이 한 번이 추가 비용이 아니다.
        let ind: Vec<Option<usize>> = (0..total).map(|i| eb.fold_indent(i)).collect();
        crate::editfoldall::fold_all(&mut eb.folds, total, |i| ind.get(i).copied().flatten());
        ui.close();
    }
    if !eb.folds.is_empty() && ui.button(tr(lang, "fold.openall")).clicked() {
        crate::editfoldall::unfold_all(&mut eb.folds);
        ui.close();
    }
    // 변환(정렬·대소문자·인코딩 등)은 editbufxform 덕에 rope 문서에서도 돈다.
    // 선택이 있으면 그 구간만, 없으면 문서 전체(크기 한도 안에서).
    let blocked = crate::editbufxform::target_range(eb.selection(), eb.rope.len_chars()).is_none();
    ui.add_enabled_ui(!readonly && !blocked, |ui| {
        ui.menu_button(tr(lang, "ctx.transform"), |ui| {
            if let Some(f) = crate::editorxform::transform_menu(ui, lang) {
                if !eb.apply_transform(f) { act.note = Some("edit.nochange"); }
                ui.close();
            }
        })
        .response
        .on_disabled_hover_text(tr(lang, "editor.xform.toobig"));
    });
    act
}
