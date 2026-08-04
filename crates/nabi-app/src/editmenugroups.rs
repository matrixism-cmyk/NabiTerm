//! nabiPad Edit 메뉴 그룹 헬퍼 — 비대했던 평면 변환 목록을 주제별 서브메뉴로 묶는다.
//! 각 헬퍼는 클릭된 순수 변환 함수를 Option으로 돌려주고, editormenu가 doc에 적용한다.
//! (VS Code/Sublime/Notepad++ 메뉴 그룹핑 벤치마킹 — 2단계 이하 계층 + 발견성.)

use nabi_i18n::{tr, Lang};

pub(crate) type Xf = fn(&str) -> String;

/// (i18n 키, 변환 함수) 목록을 버튼으로 그려 클릭된 함수를 돌려준다. 메뉴 그룹 공용.
pub(crate) fn pick(ui: &mut egui::Ui, lang: Lang, items: &[(&str, Xf)]) -> Option<Xf> {
    for (k, f) in items {
        if ui.button(tr(lang, k)).clicked() {
            ui.close_menu();
            return Some(*f);
        }
    }
    None
}

/// "정렬" — 사전/자연/대소무시/내림/길이/숫자/고유.
pub(crate) fn sort_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    use crate::editorsort as s;
    pick(ui, lang, &[
        ("editor.sortlines", sort_lines),
        ("editor.sortnat", s::sort_natural),
        ("editor.sortci", s::sort_ci),
        ("editor.sortdesc", s::sort_desc),
        ("editor.sortlen", s::sort_by_length),
        ("editor.sortnum", s::sort_numeric),
        ("editor.sortuniq", s::sort_unique),
    ])
}

/// "들여쓰기/줄번호" — 탭↔공백·들여/내어쓰기·줄 번호.
pub(crate) fn indent_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    use crate::editorindent as i;
    use crate::editorlines as l;
    pick(ui, lang, &[
        ("editor.tabs2spaces", i::tabs_to_spaces4),
        ("editor.spaces2tabs", i::spaces_to_tabs4),
        ("editor.indent", i::indent2),
        ("editor.outdent", i::outdent2),
        ("editor.numon", l::add_line_numbers),
        ("editor.numoff", l::remove_line_numbers),
    ])
}

/// "공백/빈 줄" — 후행/선두/양쪽 trim·앞뒤 빈 줄·빈 줄 제거·빈 줄/공백 축약.
pub(crate) fn whitespace_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    use crate::editorlines as l;
    pick(ui, lang, &[
        ("editor.trimws", trim_trailing),
        ("editor.trimlead", l::trim_leading),
        ("editor.trimboth", l::trim_both),
        ("editor.trimblanks", l::trim_blank_edges),
        ("editor.rmempty", remove_empty_lines),
        ("editor.squeeze", l::squeeze_blanks),
        ("editor.collapsespaces", l::collapse_spaces),
        ("editor.tolf", l::to_lf),
        ("editor.tocrlf", l::to_crlf),
    ])
}

/// "줄 작업" — 중복 제거 3종·줄 순서/문자 뒤집기·줄 합치기.
pub(crate) fn lineops_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    use crate::editorlines as l;
    pick(ui, lang, &[
        ("editor.dedupe", dedupe_lines),
        ("editor.dedupeci", l::dedupe_ci),
        ("editor.uniqadj", l::uniq_adjacent),
        ("editor.keepuniq", l::keep_unique),
        ("editor.reverselines", reverse_lines),
        ("editor.joinlines", l::join_lines),
        ("editor.revchars", l::reverse_line_chars),
    ])
}

/// "암호" — ROT13/ROT47·Atbash·모스·A1Z26(CyberChef식 분류로 변환·텍스트도구에서 통합).
pub(crate) fn cipher_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    pick(ui, lang, &[
        ("editor.rot13", crate::editorcodec::rot13),
        ("editor.rot47", crate::editorcodec2::rot47),
        ("editor.rot5", crate::editorcodec2::rot5),
        ("editor.atbash", crate::editortext::atbash),
        ("editor.morseenc", crate::editorcodec3::morse_encode),
        ("editor.morsedec", crate::editorcodec3::morse_decode),
        ("editor.a1z26enc", crate::editorcodec3::a1z26_encode),
        ("editor.a1z26dec", crate::editorcodec3::a1z26_decode),
    ])
}

/// 줄을 사전순으로 정렬한다(전체 문서).
fn sort_lines(text: &str) -> String {
    let mut v: Vec<&str> = text.split('\n').collect();
    v.sort_unstable();
    v.join("\n")
}

/// 연속 여부와 무관하게 중복 줄을 제거한다(처음 나온 것만 유지, 순서 보존).
fn dedupe_lines(text: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    text.split('\n').filter(|l| seen.insert(*l)).collect::<Vec<_>>().join("\n")
}

/// 줄 순서를 뒤집는다.
fn reverse_lines(text: &str) -> String {
    text.split('\n').rev().collect::<Vec<_>>().join("\n")
}

/// 빈 줄(공백만 있는 줄 포함)을 제거한다.
fn remove_empty_lines(text: &str) -> String {
    text.split('\n').filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

/// 각 줄 끝의 공백·탭을 제거한다(CR은 보존해 CRLF가 유지되게).
fn trim_trailing(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.split('\n').peekable();
    while let Some(line) = it.next() {
        out.push_str(line.trim_end_matches([' ', '\t']));
        if it.peek().is_some() {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_transforms() {
        assert_eq!(sort_lines("b\na\nc"), "a\nb\nc");
        assert_eq!(dedupe_lines("a\nb\na"), "a\nb");
        assert_eq!(reverse_lines("a\nb\nc"), "c\nb\na");
        assert_eq!(remove_empty_lines("a\n \nb"), "a\nb");
        assert_eq!(trim_trailing("a  \nb\t"), "a\nb");
    }
}
