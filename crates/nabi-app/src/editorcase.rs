//! nabiPad 식별자 케이스 변환(VS Code "Change Case" 벤치마킹) — 줄 단위로 적용해
//! 식별자 목록(한 줄 하나)을 일괄 변환한다. 단어 분리는 구분자 + camelCase 경계 기준.

use nabi_i18n::{tr, Lang};

/// 문자열을 소문자 단어들로 분리(공백/기호 + 소문자→대문자 경계에서 끊음).
fn words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut prev) = (Vec::new(), String::new(), None::<char>);
    for c in s.chars() {
        if c.is_alphanumeric() {
            if matches!(prev, Some(p) if p.is_lowercase() && c.is_uppercase()) && !cur.is_empty() {
                out.push(std::mem::take(&mut cur)); // camelCase 경계.
            }
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev = Some(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.iter().map(|w| w.to_lowercase()).collect()
}

/// 각 줄을 단어로 분리해 변환기를 적용한다(빈 줄 보존).
fn per_line(t: &str, f: impl Fn(Vec<String>) -> String) -> String {
    t.split('\n').map(|l| f(words(l))).collect::<Vec<_>>().join("\n")
}

/// 첫 글자만 대문자.
fn cap(w: &str) -> String {
    let mut ch = w.chars();
    match ch.next() {
        Some(c) => c.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

pub(crate) fn to_snake(t: &str) -> String {
    per_line(t, |w| w.join("_"))
}
pub(crate) fn to_kebab(t: &str) -> String {
    per_line(t, |w| w.join("-"))
}
pub(crate) fn to_constant(t: &str) -> String {
    per_line(t, |w| w.iter().map(|x| x.to_uppercase()).collect::<Vec<_>>().join("_"))
}
pub(crate) fn to_pascal(t: &str) -> String {
    per_line(t, |w| w.iter().map(|x| cap(x)).collect())
}
pub(crate) fn to_camel(t: &str) -> String {
    per_line(t, |w| {
        w.iter().enumerate().map(|(i, x)| if i == 0 { x.clone() } else { cap(x) }).collect()
    })
}

/// 각 글자의 대소문자를 반전(Vim g~ / CyberChef Swap case). 단어 분리 무관.
pub(crate) fn swap_case(t: &str) -> String {
    t.chars()
        .flat_map(|c| {
            if c.is_uppercase() {
                c.to_lowercase().collect::<Vec<_>>()
            } else if c.is_lowercase() {
                c.to_uppercase().collect()
            } else {
                vec![c]
            }
        })
        .collect()
}

/// 교차 케이스(aLtErNaTiNg) — 알파벳을 짝/홀로 소·대문자 교대(비알파벳은 카운트 제외). 밈/강조용.
pub(crate) fn alternating_case(t: &str) -> String {
    let mut n = 0usize;
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        if c.is_alphabetic() {
            if n.is_multiple_of(2) {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            n += 1;
        } else {
            out.push(c);
        }
    }
    out
}

/// 문장 케이스: 각 문장(`. ! ?`/개행 뒤)의 첫 글자만 대문자, 나머지 소문자.
pub(crate) fn sentence_case(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut new_sentence = true;
    for c in t.chars() {
        if c.is_alphabetic() {
            if new_sentence {
                out.extend(c.to_uppercase());
                new_sentence = false;
            } else {
                out.extend(c.to_lowercase());
            }
        } else {
            out.push(c);
            if matches!(c, '.' | '!' | '?' | '\n') {
                new_sentence = true;
            }
        }
    }
    out
}

/// "케이스" 서브메뉴 — 텍스트/프로그래밍 케이스 하위 그룹으로 분류(2단계 계층).
pub(crate) fn case_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editmenugroups::pick;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.casetext"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.upper", |t: &str| t.to_uppercase()),
            ("editor.lower", |t: &str| t.to_lowercase()),
            ("editor.titlecase", crate::editorlines::to_title_case),
            ("editor.sentence", sentence_case),
            ("editor.swapcase", swap_case),
            ("editor.altcase", alternating_case),
        ]));
    });
    ui.menu_button(tr(lang, "editor.caseprog"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.snake", to_snake), ("editor.kebab", to_kebab),
            ("editor.camel", to_camel), ("editor.pascal", to_pascal),
            ("editor.constant", to_constant),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_conversions() {
        assert_eq!(to_snake("myVariable Name"), "my_variable_name");
        assert_eq!(to_kebab("Hello World"), "hello-world");
        assert_eq!(to_camel("hello_world foo"), "helloWorldFoo");
        assert_eq!(to_pascal("hello world"), "HelloWorld");
        assert_eq!(to_constant("fooBar"), "FOO_BAR");
    }

    #[test]
    fn per_line_and_swap() {
        assert_eq!(to_snake("FooBar\nbazQux"), "foo_bar\nbaz_qux"); // 줄 단위.
        assert_eq!(swap_case("Hello, World"), "hELLO, wORLD");
        assert_eq!(to_snake(""), ""); // 빈 입력.
        assert_eq!(alternating_case("hello world"), "hElLo WoRlD"); // 비알파벳 카운트 제외.
    }

    #[test]
    fn sentence_case_boundaries() {
        assert_eq!(sentence_case("hello WORLD. bye now"), "Hello world. Bye now"); // .뒤 새 문장.
        assert_eq!(sentence_case("what? yes!"), "What? Yes!"); // ?!도 경계.
        assert_eq!(sentence_case("a\nb"), "A\nB"); // 개행도 새 문장.
    }
}
