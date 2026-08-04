//! nabiPad 선택 변환 메뉴 — 카테고리별 하위 그룹(대소문자/들여쓰기/정렬/줄/인코딩).
//! editorctx의 "변환" 서브메뉴에서 호출하며, 선택된 순수 변환 함수를 돌려준다(메뉴 정리·DRY).

use nabi_i18n::{tr, Lang};

type Xf = fn(&str) -> String;

/// 변환 항목을 그룹별 하위 메뉴로 렌더하고, 클릭된 변환 함수를 돌려준다(없으면 None).
pub(crate) fn transform_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    let case: [(&str, Xf); 2] = [("ctx.upper", |s| s.to_uppercase()), ("ctx.lower", |s| s.to_lowercase())];
    let indent: [(&str, Xf); 4] = [
        ("ctx.indentsel", crate::editorindent::indent2),
        ("ctx.outdentsel", crate::editorindent::outdent2),
        ("ctx.tab2sp", crate::editorindent::tabs_to_spaces4),
        ("ctx.sp2tab", crate::editorindent::spaces_to_tabs4),
    ];
    let sort: [(&str, Xf); 6] = [
        ("ctx.sortsel", crate::editorsort::sort_natural),
        ("ctx.sortcisel", crate::editorsort::sort_ci),
        ("ctx.sortdescsel", crate::editorsort::sort_desc),
        ("ctx.sortnumsel", crate::editorsort::sort_numeric),
        ("ctx.sortlensel", crate::editorsort::sort_by_length),
        ("ctx.dedupsel", crate::editorlines::keep_unique),
    ];
    let lines: [(&str, Xf); 4] = [
        ("ctx.revsel", |s| s.lines().rev().collect::<Vec<_>>().join("\n")),
        ("ctx.revchars", crate::editortext::reverse_all),
        ("ctx.joinsel", crate::editorlines::join_lines),
        ("ctx.trimsel", crate::editorlines::trim_both),
    ];
    let encode: [(&str, Xf); 13] = [
        ("ctx.b64enc", crate::editorconvert::base64_encode),
        ("ctx.b64dec", crate::editorconvert::base64_decode),
        ("ctx.urlenc", crate::editorconvert::url_encode),
        ("ctx.urldec", crate::editorconvert::url_decode),
        ("ctx.hexenc", crate::editorcodec::hex_text_encode),
        ("ctx.hexdec", crate::editorcodec::hex_text_decode),
        ("ctx.htmlenc", crate::editorconvert::html_encode),
        ("ctx.htmldec", crate::editorconvert::html_decode),
        ("ctx.bsesc", crate::editorconvert::backslash_escape),
        ("ctx.bsunesc", crate::editorconvert::backslash_unescape),
        ("ctx.jsonpretty", crate::editorconvert::json_pretty),
        ("ctx.jsonmin", crate::editorconvert::json_minify),
        ("ctx.rot13", crate::editorcodec::rot13),
    ];
    let groups: [(&str, &[(&str, Xf)]); 5] = [
        ("ctx.grp.case", &case),
        ("ctx.grp.indent", &indent),
        ("ctx.grp.sort", &sort),
        ("ctx.grp.lines", &lines),
        ("ctx.grp.encode", &encode),
    ];
    let mut chosen = None;
    for (gkey, items) in groups {
        ui.menu_button(tr(lang, gkey), |ui| {
            for (key, f) in items {
                if ui.button(tr(lang, key)).clicked() {
                    chosen = Some(*f);
                    ui.close_menu();
                }
            }
        });
    }
    chosen
}
